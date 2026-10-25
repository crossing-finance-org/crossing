#![no_std]
use crossing_common::{
    self as cc, AuctionClient, AuctionMeta, Clearing, Config, Error, Execution, FactoryClient,
    Observation, Stage,
};
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
enum Key {
    Factory,
    FactoryHash,
    Template,
    Lock,
    Clearing(Address),
    Settlement(Address),
    Count(Address, Address),
    History(Address, Address, u64),
    Latest(Address, Address),
    LastSettled(Address, Address),
}
#[contract]
pub struct PriceRegistry;
fn read<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(env: &Env, k: &Key) -> T {
    env.storage().instance().get(k).unwrap()
}
fn store<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, key: &Key, value: &T) {
    env.storage().persistent().set(key, value);
    let ttl = cc::RETENTION.min(env.storage().max_ttl());
    env.storage().persistent().extend_ttl(key, ttl / 2, ttl);
}
fn enter(env: &Env) -> Result<(), Error> {
    if env.storage().instance().get(&Key::Lock).unwrap_or(false) {
        return Err(Error::Reentry);
    }
    env.storage().instance().set(&Key::Lock, &true);
    Ok(())
}
fn leave(env: &Env) {
    env.storage().instance().set(&Key::Lock, &false);
    cc::touch(env);
}
fn source(env: &Env, auction: &Address) -> Result<(AuctionMeta, Config, Clearing), Error> {
    let factory: Address = read(env, &Key::Factory);
    cc::wasm(&factory, &read(env, &Key::FactoryHash))?;
    let template: BytesN<32> = read(env, &Key::Template);
    cc::wasm(auction, &template)?;
    let meta = FactoryClient::new(env, &factory)
        .get_auction(auction)
        .ok_or(Error::Provenance)?;
    let a = AuctionClient::new(env, auction);
    let c = a.config();
    let p = a.provenance();
    if p.factory != factory
        || p.registry != env.current_contract_address()
        || p.template != template
        || meta.template != template
        || meta.config_hash != p.config_hash
        || cc::config_hash(env, &c) != meta.config_hash
        || c.issuer != meta.issuer
        || c.rwa_token != meta.rwa
        || c.quote_token != meta.quote
        || c.mode != meta.mode
        || c.version != 1
    {
        return Err(Error::Provenance);
    }
    if !matches!(a.state().stage, Stage::Cleared | Stage::Settled) {
        return Err(Error::Phase);
    }
    let k = a.clearing().ok_or(Error::Phase)?;
    if k.filled <= 0 || k.quote <= 0 {
        return Err(Error::Amount);
    }
    Ok((meta, c, k))
}
// Same-ledger ties break on raw contract-ID bytes. An authorized issuer can
// influence these addresses through deployment salts; provenance does not
// prevent that. This arbitrary ordering carries no economic meaning.
fn newer(env: &Env, auction: &Address, k: &Clearing, key: &Key) -> Result<bool, Error> {
    if let Some(old) = env.storage().persistent().get::<_, Address>(key) {
        let observation: Observation = env
            .storage()
            .persistent()
            .get(&Key::Clearing(old.clone()))
            .ok_or(Error::Missing)?;
        Ok(k.ledger > observation.clearing.ledger
            || (k.ledger == observation.clearing.ledger && auction > &old))
    } else {
        Ok(true)
    }
}
fn record(env: &Env, auction: &Address) -> Result<Observation, Error> {
    if let Some(record) = env
        .storage()
        .persistent()
        .get(&Key::Clearing(auction.clone()))
    {
        return Ok(record);
    }
    let (meta, c, k) = source(env, auction)?;
    let record = Observation {
        auction: auction.clone(),
        meta: meta.clone(),
        clearing: k.clone(),
        delivery_deadline: c.delivery_deadline,
        published_at: env.ledger().timestamp(),
    };
    let count_key = Key::Count(meta.rwa.clone(), meta.quote.clone());
    let count: u64 = env.storage().persistent().get(&count_key).unwrap_or(0);
    let next = count.checked_add(1).ok_or(Error::Arithmetic)?;
    store(env, &Key::Clearing(auction.clone()), &record);
    store(
        env,
        &Key::History(meta.rwa.clone(), meta.quote.clone(), count),
        auction,
    );
    store(env, &count_key, &next);
    let latest = Key::Latest(meta.rwa, meta.quote);
    if newer(env, auction, &k, &latest)? {
        store(env, &latest, auction);
    }
    #[allow(deprecated)]
    env.events().publish(
        (
            Symbol::new(env, "crossing_v1"),
            Symbol::new(env, "clearing_recorded"),
        ),
        auction.clone(),
    );
    Ok(record)
}
#[contractimpl]
impl PriceRegistry {
    pub fn __constructor(
        env: Env,
        factory: Address,
        factory_wasm_hash: BytesN<32>,
        auction_wasm_hash: BytesN<32>,
    ) {
        env.storage().instance().set(&Key::Factory, &factory);
        env.storage()
            .instance()
            .set(&Key::FactoryHash, &factory_wasm_hash);
        env.storage()
            .instance()
            .set(&Key::Template, &auction_wasm_hash);
        cc::touch(&env);
    }
    pub fn bindings(env: Env) -> (Address, BytesN<32>, BytesN<32>) {
        (
            read(&env, &Key::Factory),
            read(&env, &Key::FactoryHash),
            read(&env, &Key::Template),
        )
    }
    pub fn record_clearing(env: Env, auction: Address) -> Result<Address, Error> {
        enter(&env)?;
        record(&env, &auction)?;
        leave(&env);
        Ok(auction)
    }
    pub fn record_settlement(env: Env, auction: Address) -> Result<Address, Error> {
        enter(&env)?;
        if env
            .storage()
            .persistent()
            .has(&Key::Settlement(auction.clone()))
        {
            leave(&env);
            return Ok(auction);
        }
        let observation = record(&env, &auction)?;
        // Always verify the source even if the clearing observation already existed.
        let (_, _, k) = source(&env, &auction)?;
        let a = AuctionClient::new(&env, &auction);
        if a.state().stage != Stage::Settled {
            return Err(Error::Phase);
        }
        let settlement = a.settlement().ok_or(Error::Phase)?;
        let execution = Execution {
            auction: auction.clone(),
            clearing_ledger: k.ledger,
            settlement: settlement.clone(),
            published_at: env.ledger().timestamp(),
        };
        store(&env, &Key::Settlement(auction.clone()), &execution);
        let key = Key::LastSettled(observation.meta.rwa, observation.meta.quote);
        if settlement.executed_rwa > 0 && newer(&env, &auction, &k, &key)? {
            store(&env, &key, &auction);
        }
        #[allow(deprecated)]
        env.events().publish(
            (
                Symbol::new(&env, "crossing_v1"),
                Symbol::new(&env, "settlement_recorded"),
            ),
            auction.clone(),
        );
        leave(&env);
        Ok(auction)
    }
    pub fn get_clearing(env: Env, auction: Address) -> Option<Observation> {
        env.storage().persistent().get(&Key::Clearing(auction))
    }
    pub fn get_settlement(env: Env, auction: Address) -> Option<Execution> {
        env.storage().persistent().get(&Key::Settlement(auction))
    }
    pub fn latest(env: Env, rwa: Address, quote: Address) -> Option<Observation> {
        let auction: Address = env.storage().persistent().get(&Key::Latest(rwa, quote))?;
        env.storage().persistent().get(&Key::Clearing(auction))
    }
    pub fn latest_settled(env: Env, rwa: Address, quote: Address) -> Option<Execution> {
        let auction: Address = env
            .storage()
            .persistent()
            .get(&Key::LastSettled(rwa, quote))?;
        env.storage().persistent().get(&Key::Settlement(auction))
    }
    pub fn count(env: Env, rwa: Address, quote: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&Key::Count(rwa, quote))
            .unwrap_or(0)
    }
    pub fn history(
        env: Env,
        rwa: Address,
        quote: Address,
        start: u64,
        limit: u32,
    ) -> Result<Vec<Address>, Error> {
        let end = env
            .storage()
            .persistent()
            .get(&Key::Count(rwa.clone(), quote.clone()))
            .unwrap_or(0);
        let mut out = Vec::new(&env);
        for id in cc::ids(&env, start, limit, end)?.iter() {
            out.push_back(
                env.storage()
                    .persistent()
                    .get(&Key::History(rwa.clone(), quote.clone(), id))
                    .ok_or(Error::Missing)?,
            );
        }
        Ok(out)
    }
    pub fn maintain(
        env: Env,
        rwa: Address,
        quote: Address,
        start: u64,
        count: u32,
    ) -> Result<u32, Error> {
        enter(&env)?;
        if count == 0 || count > 8 {
            return Err(Error::Work);
        }
        let ck = Key::Count(rwa.clone(), quote.clone());
        let end = env.storage().persistent().get(&ck).unwrap_or(0u64);
        let ttl = cc::RETENTION.min(env.storage().max_ttl());
        let mut done = 0;
        for id in cc::ids(&env, start, count, end)?.iter() {
            let hk = Key::History(rwa.clone(), quote.clone(), id);
            let auction: Address = env.storage().persistent().get(&hk).ok_or(Error::Missing)?;
            for key in [hk, Key::Clearing(auction.clone()), Key::Settlement(auction)] {
                if env.storage().persistent().has(&key) {
                    env.storage().persistent().extend_ttl(&key, ttl / 2, ttl);
                }
            }
            done += 1;
        }
        for key in [
            ck,
            Key::Latest(rwa.clone(), quote.clone()),
            Key::LastSettled(rwa, quote),
        ] {
            if env.storage().persistent().has(&key) {
                env.storage().persistent().extend_ttl(&key, ttl / 2, ttl);
            }
        }
        leave(&env);
        Ok(done)
    }
}
