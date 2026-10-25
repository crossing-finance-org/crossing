#![no_std]
use crossing_common::{self as cc, AuctionMeta, Config, Error, RegistryClient};
use soroban_sdk::{
    contract, contractimpl, contracttype, xdr::ToXdr, Address, BytesN, Env, Symbol, Vec,
};

#[contracttype]
#[derive(Clone)]
enum Key {
    Registry,
    RegistryHash,
    Template,
    Count,
    Auction(Address),
    At(u64),
    Lock,
}
#[contract]
pub struct AuctionFactory;

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
fn read<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(env: &Env, k: &Key) -> T {
    env.storage().instance().get(k).unwrap()
}
fn store<T: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, key: &Key, value: &T) {
    env.storage().persistent().set(key, value);
    let ttl = cc::RETENTION.min(env.storage().max_ttl());
    env.storage().persistent().extend_ttl(key, ttl / 2, ttl);
}
fn salt(env: &Env, issuer: &Address, user: &BytesN<32>) -> BytesN<32> {
    env.crypto()
        .sha256(
            &(
                Symbol::new(env, "CROSSING_AUC_V1"),
                env.ledger().network_id(),
                env.current_contract_address(),
                issuer.clone(),
                user.clone(),
            )
                .to_xdr(env),
        )
        .into()
}

#[contractimpl]
impl AuctionFactory {
    pub fn __constructor(
        env: Env,
        registry: Address,
        registry_wasm_hash: BytesN<32>,
        auction_wasm_hash: BytesN<32>,
    ) -> Result<(), Error> {
        cc::wasm(&registry, &registry_wasm_hash)?;
        let (factory, expected_hash, template) = RegistryClient::new(&env, &registry).bindings();
        if factory != env.current_contract_address() || template != auction_wasm_hash {
            return Err(Error::Provenance);
        }
        cc::wasm(&factory, &expected_hash)?;
        env.storage().instance().set(&Key::Registry, &registry);
        env.storage()
            .instance()
            .set(&Key::RegistryHash, &registry_wasm_hash);
        env.storage()
            .instance()
            .set(&Key::Template, &auction_wasm_hash);
        env.storage().instance().set(&Key::Count, &0u64);
        cc::touch(&env);
        Ok(())
    }
    pub fn bindings(env: Env) -> (Address, BytesN<32>, BytesN<32>) {
        (
            read(&env, &Key::Registry),
            read(&env, &Key::RegistryHash),
            read(&env, &Key::Template),
        )
    }
    pub fn predict(env: Env, issuer: Address, user_salt: BytesN<32>) -> Address {
        env.deployer()
            .with_current_contract(salt(&env, &issuer, &user_salt))
            .deployed_address()
    }
    pub fn create(env: Env, config: Config, user_salt: BytesN<32>) -> Result<Address, Error> {
        enter(&env)?;
        config.issuer.require_auth();
        cc::validate_config(&env, &config)?;
        let registry: Address = read(&env, &Key::Registry);
        let registry_hash: BytesN<32> = read(&env, &Key::RegistryHash);
        let template: BytesN<32> = read(&env, &Key::Template);
        cc::wasm(&registry, &registry_hash)?;
        let deployer = env
            .deployer()
            .with_current_contract(salt(&env, &config.issuer, &user_salt));
        let predicted = deployer.deployed_address();
        if config.buyback_recipient == predicted
            || config.buyback_recipient == env.current_contract_address()
            || config.buyback_recipient == registry
        {
            return Err(Error::Recipient);
        }
        if env
            .storage()
            .persistent()
            .has(&Key::Auction(predicted.clone()))
        {
            return Err(Error::Replay);
        }
        let hash = cc::config_hash(&env, &config);
        let address = deployer.deploy_v2(
            template.clone(),
            (
                config.clone(),
                env.current_contract_address(),
                registry.clone(),
                template.clone(),
            ),
        );
        let count: u64 = read(&env, &Key::Count);
        let next = count.checked_add(1).ok_or(Error::Arithmetic)?;
        let meta = AuctionMeta {
            issuer: config.issuer,
            rwa: config.rwa_token,
            quote: config.quote_token,
            mode: config.mode,
            config_hash: hash,
            template,
            ledger: env.ledger().sequence(),
        };
        store(&env, &Key::Auction(address.clone()), &meta);
        store(&env, &Key::At(count), &address);
        env.storage().instance().set(&Key::Count, &next);
        #[allow(deprecated)]
        env.events().publish(
            (
                Symbol::new(&env, "crossing_v1"),
                Symbol::new(&env, "created"),
            ),
            (address.clone(), meta),
        );
        leave(&env);
        Ok(address)
    }
    pub fn get_auction(env: Env, auction: Address) -> Option<AuctionMeta> {
        env.storage().persistent().get(&Key::Auction(auction))
    }
    pub fn count(env: Env) -> u64 {
        read(&env, &Key::Count)
    }
    pub fn list(env: Env, start: u64, limit: u32) -> Result<Vec<Address>, Error> {
        let mut out = Vec::new(&env);
        for id in cc::ids(&env, start, limit, read(&env, &Key::Count))?.iter() {
            out.push_back(
                env.storage()
                    .persistent()
                    .get(&Key::At(id))
                    .ok_or(Error::Missing)?,
            );
        }
        Ok(out)
    }
    pub fn maintain(env: Env, start: u64, count: u32) -> Result<u32, Error> {
        enter(&env)?;
        if count == 0 || count > 8 {
            return Err(Error::Work);
        }
        let mut done = 0;
        let ttl = cc::RETENTION.min(env.storage().max_ttl());
        for id in cc::ids(&env, start, count, read(&env, &Key::Count))?.iter() {
            let auction: Address = env
                .storage()
                .persistent()
                .get(&Key::At(id))
                .ok_or(Error::Missing)?;
            env.storage()
                .persistent()
                .extend_ttl(&Key::At(id), ttl / 2, ttl);
            env.storage()
                .persistent()
                .extend_ttl(&Key::Auction(auction), ttl / 2, ttl);
            done += 1;
        }
        leave(&env);
        Ok(done)
    }
}
