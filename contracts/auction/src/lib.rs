#![no_std]
use crossing_common::{
    self as cc, Accounting, Bid, BidMode, Candidate, Clearing, Config, Error, FactoryClient,
    Finalization, Mode, Progress, Provenance, Reason, Resolution, Settlement, Stage, State,
};
use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, BytesN, Env, IntoVal, Symbol, Val, Vec,
};

#[contracttype]
#[derive(Clone)]
enum Key {
    Config,
    Provenance,
    State,
    Accounting,
    Clearing,
    Settlement,
    Lock,
    CancelReason,
    Bid(u32),
    Tick(u32),
    Count(Address),
    OwnerBid(Address, u32),
    Nonce(Address, BytesN<32>),
    Credit(Address, Address),
}
#[contracttype]
#[derive(Clone)]
pub enum MaintenanceKey {
    Bid(u32),
    Tick(u32),
    Count(Address),
    OwnerBid(Address, u32),
    Nonce(Address, BytesN<32>),
    Credit(Address, Address),
}
#[contract]
pub struct Auction;
fn read<T: soroban_sdk::TryFromVal<Env, Val>>(env: &Env, key: &Key) -> T {
    env.storage().instance().get(key).unwrap()
}
fn cfg(env: &Env) -> Config {
    read(env, &Key::Config)
}
fn enter(env: &Env) -> Result<(), Error> {
    if env.storage().instance().get(&Key::Lock).unwrap_or(false) {
        return Err(Error::Reentry);
    }
    env.storage().instance().set(&Key::Lock, &true);
    Ok(())
}
// Every caller already has its own `Config` in scope from earlier in the same
// invocation; take it instead of re-reading instance storage a second time.
fn leave(env: &Env, c: &Config) -> Result<(), Error> {
    let ttl = cc::financial_ttl(env, c)?;
    env.storage().instance().set(&Key::Lock, &false);
    env.storage().instance().extend_ttl(ttl / 2, ttl);
    Ok(())
}
// Takes the caller's already-fetched `Config` rather than re-reading instance
// storage on every write: every call site already has one in scope, and a
// single invocation can write several keys (`admit` alone writes up to four).
fn store<T: IntoVal<Env, Val>>(env: &Env, c: &Config, key: &Key, value: &T) -> Result<(), Error> {
    env.storage().persistent().set(key, value);
    let ttl = cc::financial_ttl(env, c)?;
    env.storage().persistent().extend_ttl(key, ttl / 2, ttl);
    Ok(())
}
#[allow(deprecated)]
fn emit<T: IntoVal<Env, Val>>(env: &Env, kind: &str, data: T) {
    env.events().publish(
        (Symbol::new(env, "crossing_v1"), Symbol::new(env, kind)),
        data,
    );
}
fn bid(env: &Env, id: u32) -> Result<Bid, Error> {
    let s: State = read(env, &Key::State);
    if id >= s.bid_count {
        return Err(Error::Missing);
    }
    env.storage()
        .persistent()
        .get(&Key::Bid(id))
        .ok_or(Error::Missing)
}
fn save_accounting(env: &Env, a: &Accounting) -> Result<(), Error> {
    if !a.balanced() {
        return Err(Error::Arithmetic);
    }
    env.storage().instance().set(&Key::Accounting, a);
    Ok(())
}
fn add_credit(
    env: &Env,
    c: &Config,
    a: &mut Accounting,
    owner: &Address,
    asset: &Address,
    n: i128,
) -> Result<(), Error> {
    if n < 0 {
        return Err(Error::Arithmetic);
    }
    if n == 0 {
        return Ok(());
    }
    let key = Key::Credit(owner.clone(), asset.clone());
    let old = env.storage().persistent().get(&key).unwrap_or(0);
    store(env, c, &key, &cc::add(old, n)?)?;
    if asset == &c.rwa_token {
        a.credit_rwa = cc::add(a.credit_rwa, n)?;
    } else if asset == &c.quote_token {
        a.credit_quote = cc::add(a.credit_quote, n)?;
    } else {
        return Err(Error::Asset);
    }
    Ok(())
}
fn solvent(env: &Env, c: &Config, a: &Accounting) -> Result<(i128, i128), Error> {
    let me = env.current_contract_address();
    let r = token::Client::new(env, &c.rwa_token).balance(&me);
    let q = token::Client::new(env, &c.quote_token).balance(&me);
    if r < a.liability_rwa || q < a.liability_quote {
        return Err(Error::Insolvent);
    }
    Ok((r, q))
}
fn transfer(
    env: &Env,
    asset: &Address,
    from: &Address,
    to: &Address,
    n: i128,
) -> Result<(), Error> {
    match token::Client::new(env, asset).try_transfer(from, to, &n) {
        Ok(Ok(())) => Ok(()),
        _ => Err(Error::Transfer),
    }
}
fn send(env: &Env, asset: &Address, to: &Address, n: i128) -> Result<(), Error> {
    if n > 0 {
        transfer(env, asset, &env.current_contract_address(), to, n)?;
    }
    Ok(())
}
fn reset(c: &Config, s: &mut State) -> Result<(), Error> {
    s.epoch = s.epoch.checked_add(1).ok_or(Error::Arithmetic)?;
    s.stage = Stage::Closing;
    s.tick_cursor = if c.mode == Mode::Sale {
        cc::tick_count(c) - 1
    } else {
        0
    };
    s.sweep_cursor = 0;
    s.cumulative = 0;
    s.previous_price = 0;
    s.candidate = cc::CandidateValue::None;
    Ok(())
}
fn contributes_to_price(c: &Config, b: &Bid, k: &Candidate) -> bool {
    // Marginal bids can set the uniform price even if quantity/payment rounding
    // removes their individual fill. Their authorization must still be valid.
    !b.excluded
        && b.quantity.is_some()
        && b.price.is_some_and(|p| match c.mode {
            Mode::Sale => p >= k.price,
            Mode::Buyback => p <= k.price,
        })
}
fn exclude(env: &Env, c: &Config, s: &mut State, id: u32, b: &mut Bid) -> Result<(), Error> {
    if b.excluded {
        return Err(Error::Replay);
    }
    let q = b.quantity.ok_or(Error::Missing)?;
    let p = b.price.ok_or(Error::Missing)?;
    let key = Key::Tick(cc::tick_index(c, p)?);
    let total = env.storage().persistent().get(&key).ok_or(Error::Missing)?;
    store(env, c, &key, &cc::sub(total, q)?)?;
    b.excluded = true;
    store(env, c, &Key::Bid(id), b)?;
    reset(c, s)?;
    emit(env, "excluded", (id, s.epoch));
    Ok(())
}
fn completed(env: &Env, s: &mut State, a: &Accounting) {
    if a.resolved == s.bid_count {
        s.stage = if s.stage == Stage::Cleared {
            Stage::Settled
        } else {
            Stage::Refunded
        };
        let summary = Settlement {
            executed_rwa: a.executed_rwa,
            executed_quote: a.executed_quote,
            refunded_rwa: a.refunded_rwa,
            refunded_quote: a.refunded_quote,
            resolved: a.resolved,
            ledger: env.ledger().sequence(),
            timestamp: env.ledger().timestamp(),
        };
        env.storage().instance().set(&Key::Settlement, &summary);
        emit(env, "completed", summary);
    }
}
fn cancel(env: &Env, c: &Config, s: &mut State, reason: Reason) -> Result<(), Error> {
    if matches!(
        s.stage,
        Stage::Cleared | Stage::Settled | Stage::Cancelled | Stage::Refunded
    ) {
        return Err(Error::Phase);
    }
    let mut a: Accounting = read(env, &Key::Accounting);
    let rwa = a.reserve_rwa;
    let quote = a.reserve_quote;
    add_credit(env, c, &mut a, &c.issuer, &c.rwa_token, rwa)?;
    add_credit(env, c, &mut a, &c.issuer, &c.quote_token, quote)?;
    a.reserve_rwa = 0;
    a.reserve_quote = 0;
    s.stage = Stage::Cancelled;
    env.storage().instance().set(&Key::CancelReason, &reason);
    completed(env, s, &a);
    save_accounting(env, &a)?;
    emit(env, "cancelled", reason);
    Ok(())
}
fn admit(env: &Env, c: &Config, s: &mut State, a: &mut Accounting, b: Bid) -> Result<u32, Error> {
    let now = env.ledger().sequence();
    if s.stage != Stage::Funded || now < c.start_ledger || now >= c.end_ledger {
        return Err(Error::Window);
    }
    let p: Provenance = read(env, &Key::Provenance);
    let me = env.current_contract_address();
    if b.owner == c.issuer
        || b.owner == c.buyback_recipient
        || b.owner == me
        || b.owner == p.factory
        || b.owner == p.registry
    {
        return Err(Error::Recipient);
    }
    if !cc::both(env, c, &b.owner)? || !cc::both(env, c, &me)? {
        return Err(Error::Unauthorized);
    }
    let count_key = Key::Count(b.owner.clone());
    let count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
    if s.bid_count >= c.max_bids || count >= c.max_bids_per_address {
        return Err(Error::Capacity);
    }
    if b.deposit < c.min_deposit {
        return Err(Error::Amount);
    }
    a.admitted_deposit = cc::add(a.admitted_deposit, b.deposit)?;
    if c.bid_mode == BidMode::Sealed && c.mode == Mode::Sale {
        // Every valid reveal has q <= deposit * SCALE / min_price. Reserve
        // quantity capacity before accepting hidden terms, so reveal order
        // cannot turn a fully collateralized bid into a penalized non-reveal.
        cc::mul_div(env, a.admitted_deposit, cc::SCALE, c.min_price, false)?;
    }
    if let Some(q) = b.quantity {
        a.admitted_quantity = cc::add(a.admitted_quantity, q)?;
    }
    let asset = if c.mode == Mode::Sale {
        &c.quote_token
    } else {
        &c.rwa_token
    };
    transfer(env, asset, &b.owner, &me, b.deposit)?;
    let id = s.bid_count;
    s.bid_count = s.bid_count.checked_add(1).ok_or(Error::Arithmetic)?;
    store(env, c, &Key::Bid(id), &b)?;
    store(
        env,
        c,
        &count_key,
        &count.checked_add(1).ok_or(Error::Arithmetic)?,
    )?;
    store(env, c, &Key::OwnerBid(b.owner.clone(), count), &id)?;
    if let (Some(price), Some(quantity)) = (b.price, b.quantity) {
        let key = Key::Tick(cc::tick_index(c, price)?);
        let old = env.storage().persistent().get(&key).unwrap_or(0);
        store(env, c, &key, &cc::add(old, quantity)?)?;
    }
    if c.mode == Mode::Sale {
        a.pending_quote = cc::add(a.pending_quote, b.deposit)?;
        a.liability_quote = cc::add(a.liability_quote, b.deposit)?;
    } else {
        a.pending_rwa = cc::add(a.pending_rwa, b.deposit)?;
        a.liability_rwa = cc::add(a.liability_rwa, b.deposit)?;
    }
    save_accounting(env, a)?;
    env.storage().instance().set(&Key::State, s);
    Ok(id)
}
fn resolve(env: &Env, c: &Config, id: u32, expired: bool) -> Result<Resolution, Error> {
    let mut b = bid(env, id)?;
    if b.resolution.is_some() {
        return Err(Error::Replay);
    }
    let mut s: State = read(env, &Key::State);
    if !matches!(s.stage, Stage::Cleared | Stage::Cancelled) {
        return Err(Error::Phase);
    }
    if expired != (env.ledger().sequence() >= c.delivery_deadline) {
        return Err(Error::Window);
    }
    let mut a: Accounting = read(env, &Key::Accounting);
    let (f, payment) = if s.stage == Stage::Cleared {
        let k: Clearing = read(env, &Key::Clearing);
        cc::allocation(env, c, &b, &k.candidate)?
    } else {
        (0, 0)
    };
    // NonReveal must be classified ahead of the timing-derived `expired`
    // reason: otherwise a sealed bidder who never reveals only needs to wait
    // out the delivery deadline so resolution falls to `expire_bid` instead
    // of `claim_bid`, which reported `DeliveryExpired` and silently skipped
    // the non-reveal penalty below regardless of who called it or when.
    // Cancelled still takes priority: a never-revealed bid in an auction
    // that never cleared didn't distort any price, so it is not penalized.
    let mut reason = if s.stage == Stage::Cancelled {
        Some(read(env, &Key::CancelReason))
    } else if b.quantity.is_none() {
        Some(Reason::NonReveal)
    } else if expired {
        Some(Reason::DeliveryExpired)
    } else if b.excluded {
        Some(Reason::Revoked)
    } else if f == 0 {
        Some(Reason::NotAllocated)
    } else {
        None
    };
    if reason.is_none() {
        let owner = cc::both(env, c, &b.owner)?;
        let escrow = cc::both(env, c, &env.current_contract_address())?;
        let recipient = if c.mode == Mode::Buyback {
            cc::authorized(env, &c.rwa_token, &c.buyback_recipient)?
        } else {
            true
        };
        if !owner || !escrow || !recipient {
            reason = Some(Reason::Revoked);
        }
    }
    let delivered = reason.is_none();
    if delivered {
        solvent(env, c, &a)?;
    }
    if c.mode == Mode::Sale {
        a.pending_quote = cc::sub(a.pending_quote, b.deposit)?;
        a.reserve_rwa = cc::sub(a.reserve_rwa, f)?;
    } else {
        a.pending_rwa = cc::sub(a.pending_rwa, b.deposit)?;
        a.reserve_quote = cc::sub(a.reserve_quote, payment)?;
    }
    let is_nonreveal = matches!(reason, Some(Reason::NonReveal));
    let refund = if !delivered {
        if is_nonreveal {
            cc::sub(
                b.deposit,
                cc::penalty(env, b.deposit, c.nonreveal_penalty_bps)?,
            )?
        } else {
            b.deposit
        }
    } else if c.mode == Mode::Sale {
        cc::sub(b.deposit, payment)?
    } else {
        cc::sub(b.deposit, f)?
    };
    let asset = if c.mode == Mode::Sale {
        &c.quote_token
    } else {
        &c.rwa_token
    };
    add_credit(env, c, &mut a, &b.owner, asset, refund)?;
    if c.mode == Mode::Sale {
        a.refunded_quote = cc::add(a.refunded_quote, refund)?;
    } else {
        a.refunded_rwa = cc::add(a.refunded_rwa, refund)?;
    }
    if !delivered {
        if c.mode == Mode::Sale {
            add_credit(env, c, &mut a, &c.issuer, &c.rwa_token, f)?;
        } else {
            add_credit(env, c, &mut a, &c.issuer, &c.quote_token, payment)?;
        }
        if is_nonreveal {
            let penalty_amount = cc::sub(b.deposit, refund)?;
            add_credit(env, c, &mut a, &c.issuer, asset, penalty_amount)?;
            if penalty_amount > 0 {
                emit(env, "nonreveal_penalty", (id, penalty_amount));
            }
        }
    } else {
        a.executed_rwa = cc::add(a.executed_rwa, f)?;
        a.executed_quote = cc::add(a.executed_quote, payment)?;
        a.liability_rwa = cc::sub(a.liability_rwa, f)?;
        if c.mode == Mode::Sale {
            add_credit(env, c, &mut a, &c.issuer, &c.quote_token, payment)?;
        } else {
            a.liability_quote = cc::sub(a.liability_quote, payment)?;
        }
    }
    let outcome = Resolution {
        delivered,
        rwa: if delivered { f } else { 0 },
        quote: if delivered { payment } else { 0 },
        refund,
        reason: reason.into(),
        ledger: env.ledger().sequence(),
        timestamp: env.ledger().timestamp(),
    };
    b.resolution = cc::ResolutionValue::Some(outcome.clone());
    store(env, c, &Key::Bid(id), &b)?;
    a.resolved = a.resolved.checked_add(1).ok_or(Error::Arithmetic)?;
    completed(env, &mut s, &a);
    save_accounting(env, &a)?;
    env.storage().instance().set(&Key::State, &s);
    if delivered {
        if c.mode == Mode::Sale {
            send(env, &c.rwa_token, &b.owner, f)?;
        } else {
            send(env, &c.rwa_token, &c.buyback_recipient, f)?;
            send(env, &c.quote_token, &b.owner, payment)?;
        }
    }
    emit(
        env,
        if delivered {
            "delivered"
        } else {
            "refund_credited"
        },
        (id, outcome.clone()),
    );
    Ok(outcome)
}

#[contractimpl]
impl Auction {
    pub fn __constructor(
        env: Env,
        config: Config,
        factory: Address,
        registry: Address,
        auction_wasm_hash: BytesN<32>,
    ) -> Result<(), Error> {
        cc::validate_config(&env, &config)?;
        cc::wasm(&env.current_contract_address(), &auction_wasm_hash)?;
        if config.buyback_recipient == env.current_contract_address()
            || config.buyback_recipient == factory
            || config.buyback_recipient == registry
        {
            return Err(Error::Recipient);
        }
        let p = Provenance {
            factory,
            registry,
            template: auction_wasm_hash,
            config_hash: cc::config_hash(&env, &config),
        };
        env.storage().instance().set(&Key::Config, &config);
        env.storage().instance().set(&Key::Provenance, &p);
        env.storage().instance().set(
            &Key::State,
            &State {
                stage: Stage::Created,
                epoch: 0,
                bid_count: 0,
                tick_cursor: 0,
                sweep_cursor: 0,
                cumulative: 0,
                previous_price: 0,
                candidate: cc::CandidateValue::None,
            },
        );
        env.storage()
            .instance()
            .set(&Key::Accounting, &Accounting::zero());
        // Shared WASM may predate this instance. Force each independently to
        // the full financial horizon before using rolling extension thresholds.
        let ttl = cc::financial_ttl(&env, &config)?;
        env.storage().instance().extend_ttl(ttl, ttl);
        leave(&env, &config)
    }
    pub fn config(env: Env) -> Config {
        cfg(&env)
    }
    pub fn provenance(env: Env) -> Provenance {
        read(&env, &Key::Provenance)
    }
    pub fn state(env: Env) -> State {
        read(&env, &Key::State)
    }
    pub fn accounting(env: Env) -> Accounting {
        read(&env, &Key::Accounting)
    }
    pub fn clearing(env: Env) -> Option<Clearing> {
        env.storage().instance().get(&Key::Clearing)
    }
    pub fn settlement(env: Env) -> Option<Settlement> {
        env.storage().instance().get(&Key::Settlement)
    }
    pub fn get_bid(env: Env, id: u32) -> Result<Bid, Error> {
        bid(&env, id)
    }
    pub fn credit(env: Env, owner: Address, asset: Address) -> Result<i128, Error> {
        let c = cfg(&env);
        if asset != c.rwa_token && asset != c.quote_token {
            return Err(Error::Asset);
        }
        Ok(env
            .storage()
            .persistent()
            .get(&Key::Credit(owner, asset))
            .unwrap_or(0))
    }
    pub fn tick_total(env: Env, index: u32) -> Result<i128, Error> {
        if index >= cc::tick_count(&cfg(&env)) {
            return Err(Error::Price);
        }
        Ok(env
            .storage()
            .persistent()
            .get(&Key::Tick(index))
            .unwrap_or(0))
    }
    pub fn cancel_reason(env: Env) -> cc::ReasonValue {
        env.storage()
            .instance()
            .get::<_, Reason>(&Key::CancelReason)
            .into()
    }
    pub fn solvency(env: Env) -> (i128, i128, i128, i128) {
        let c = cfg(&env);
        let a: Accounting = read(&env, &Key::Accounting);
        let me = env.current_contract_address();
        (
            token::Client::new(&env, &c.rwa_token).balance(&me),
            a.liability_rwa,
            token::Client::new(&env, &c.quote_token).balance(&me),
            a.liability_quote,
        )
    }
    pub fn timing(env: Env) -> (bool, bool) {
        let c = cfg(&env);
        (
            env.ledger().sequence() >= c.close_deadline,
            env.ledger().sequence() >= c.delivery_deadline,
        )
    }
    pub fn bid_ids(env: Env, owner: Address, start: u32, limit: u32) -> Result<Vec<u32>, Error> {
        let end: u32 = env
            .storage()
            .persistent()
            .get(&Key::Count(owner.clone()))
            .unwrap_or(0);
        let mut out = Vec::new(&env);
        for i in cc::ids(&env, start as u64, limit, end as u64)?.iter() {
            out.push_back(
                env.storage()
                    .persistent()
                    .get(&Key::OwnerBid(owner.clone(), i as u32))
                    .ok_or(Error::Missing)?,
            );
        }
        Ok(out)
    }
    pub fn fund(env: Env) -> Result<(), Error> {
        enter(&env)?;
        let c = cfg(&env);
        c.issuer.require_auth();
        let mut s: State = read(&env, &Key::State);
        if s.stage != Stage::Created {
            return Err(Error::Phase);
        }
        // Explicit, self-documenting guard: do not rely solely on
        // validate_config's own timing clause to reject late funding, so a
        // future change to that function's reused-validation path can't
        // silently remove the only check preventing funding after bidding
        // should have started.
        if env.ledger().sequence() >= c.start_ledger {
            return Err(Error::Window);
        }
        cc::validate_config(&env, &c)?;
        let p: Provenance = read(&env, &Key::Provenance);
        let f = FactoryClient::new(&env, &p.factory);
        let (registry, rhash, template) = f.bindings();
        cc::wasm(&registry, &rhash)?;
        let (factory, fhash, rtemplate) = cc::RegistryClient::new(&env, &registry).bindings();
        cc::wasm(&factory, &fhash)?;
        let meta = f
            .get_auction(&env.current_contract_address())
            .ok_or(Error::Provenance)?;
        if registry != p.registry
            || template != p.template
            || rtemplate != template
            || factory != p.factory
            || meta.config_hash != p.config_hash
            || meta.template != template
            || meta.issuer != c.issuer
            || meta.rwa != c.rwa_token
            || meta.quote != c.quote_token
            || meta.mode != c.mode
        {
            return Err(Error::Provenance);
        }
        if !cc::both(&env, &c, &c.issuer)? || !cc::both(&env, &c, &env.current_contract_address())?
        {
            return Err(Error::Unauthorized);
        }
        if c.mode == Mode::Buyback && !cc::authorized(&env, &c.rwa_token, &c.buyback_recipient)? {
            return Err(Error::Unauthorized);
        }
        if c.buyback_recipient == env.current_contract_address()
            || c.buyback_recipient == p.factory
            || c.buyback_recipient == p.registry
        {
            return Err(Error::Recipient);
        }
        let asset = if c.mode == Mode::Sale {
            &c.rwa_token
        } else {
            &c.quote_token
        };
        transfer(
            &env,
            asset,
            &c.issuer,
            &env.current_contract_address(),
            c.supply_or_budget,
        )?;
        let mut a = Accounting::zero();
        if c.mode == Mode::Sale {
            a.reserve_rwa = c.supply_or_budget;
            a.liability_rwa = c.supply_or_budget;
        } else {
            a.reserve_quote = c.supply_or_budget;
            a.liability_quote = c.supply_or_budget;
        }
        s.stage = Stage::Funded;
        save_accounting(&env, &a)?;
        env.storage().instance().set(&Key::State, &s);
        emit(&env, "funded", c.supply_or_budget);
        leave(&env, &c)
    }
    pub fn place_bid(env: Env, bidder: Address, price: i128, quantity: i128) -> Result<u32, Error> {
        enter(&env)?;
        bidder.require_auth();
        let c = cfg(&env);
        if c.bid_mode != BidMode::Open {
            return Err(Error::Phase);
        }
        cc::validate_bid(&c, price, quantity)?;
        let deposit = if c.mode == Mode::Sale {
            cc::cost(&env, c.mode, quantity, price)?
        } else {
            quantity
        };
        let mut s = read(&env, &Key::State);
        let mut a = read(&env, &Key::Accounting);
        let id = admit(
            &env,
            &c,
            &mut s,
            &mut a,
            Bid {
                owner: bidder,
                deposit,
                nonce: None,
                commitment: None,
                price: Some(price),
                quantity: Some(quantity),
                excluded: false,
                resolution: cc::ResolutionValue::None,
            },
        )?;
        emit(&env, "bid", (id, price, quantity, deposit));
        leave(&env, &c)?;
        Ok(id)
    }
    pub fn commit_bid(
        env: Env,
        bidder: Address,
        nonce: BytesN<32>,
        hash: BytesN<32>,
        max_locked: i128,
    ) -> Result<u32, Error> {
        enter(&env)?;
        bidder.require_auth();
        let c = cfg(&env);
        if c.bid_mode != BidMode::Sealed {
            return Err(Error::Phase);
        }
        let key = Key::Nonce(bidder.clone(), nonce.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::Replay);
        }
        let mut s = read(&env, &Key::State);
        let mut a = read(&env, &Key::Accounting);
        let id = admit(
            &env,
            &c,
            &mut s,
            &mut a,
            Bid {
                owner: bidder,
                deposit: max_locked,
                nonce: Some(nonce),
                commitment: Some(hash.clone()),
                price: None,
                quantity: None,
                excluded: false,
                resolution: cc::ResolutionValue::None,
            },
        )?;
        store(&env, &c, &key, &true)?;
        emit(&env, "commit", (id, hash, max_locked));
        leave(&env, &c)?;
        Ok(id)
    }
    pub fn reveal_bid(
        env: Env,
        id: u32,
        price: i128,
        quantity: i128,
        salt: BytesN<32>,
    ) -> Result<(), Error> {
        enter(&env)?;
        let c = cfg(&env);
        let s: State = read(&env, &Key::State);
        let now = env.ledger().sequence();
        if c.bid_mode != BidMode::Sealed
            || s.stage != Stage::Funded
            || now < c.end_ledger
            || now >= c.reveal_end_ledger
        {
            return Err(Error::Window);
        }
        let mut b = bid(&env, id)?;
        b.owner.require_auth();
        if b.quantity.is_some() || b.excluded {
            return Err(Error::Replay);
        }
        let index = cc::validate_bid(&c, price, quantity)?;
        let p: Provenance = read(&env, &Key::Provenance);
        let hash = cc::commitment(
            &env,
            &p,
            &b.owner,
            &b.nonce.clone().ok_or(Error::Commitment)?,
            b.deposit,
            price,
            quantity,
            &salt,
        );
        if Some(hash) != b.commitment {
            return Err(Error::Commitment);
        }
        let needed = if c.mode == Mode::Sale {
            cc::cost(&env, c.mode, quantity, price)?
        } else {
            quantity
        };
        if needed > b.deposit {
            return Err(Error::Collateral);
        }
        let mut a: Accounting = read(&env, &Key::Accounting);
        a.admitted_quantity = cc::add(a.admitted_quantity, quantity)?;
        let key = Key::Tick(index);
        let old = env.storage().persistent().get(&key).unwrap_or(0);
        store(&env, &c, &key, &cc::add(old, quantity)?)?;
        b.price = Some(price);
        b.quantity = Some(quantity);
        store(&env, &c, &Key::Bid(id), &b)?;
        save_accounting(&env, &a)?;
        emit(&env, "reveal", (id, price, quantity));
        leave(&env, &c)
    }
    pub fn advance(env: Env, work_limit: u32) -> Result<Progress, Error> {
        enter(&env)?;
        if work_limit == 0 {
            return Err(Error::Work);
        }
        let c = cfg(&env);
        let now = env.ledger().sequence();
        if now < c.reveal_end_ledger || now >= c.close_deadline {
            return Err(Error::Window);
        }
        let mut s: State = read(&env, &Key::State);
        if s.stage == Stage::Funded {
            reset(&c, &mut s)?;
        }
        let mut processed = 0;
        if s.stage == Stage::Closing {
            for _ in 0..work_limit.min(cc::TICK_BATCH) {
                let idx = s.tick_cursor;
                let p = cc::add(
                    c.min_price,
                    c.tick_size
                        .checked_mul(idx as i128)
                        .ok_or(Error::Arithmetic)?,
                )?;
                let q = env.storage().persistent().get(&Key::Tick(idx)).unwrap_or(0);
                let total = cc::add(s.cumulative, q)?;
                processed += 1;
                if c.mode == Mode::Sale {
                    if total >= c.supply_or_budget {
                        s.candidate = cc::CandidateValue::Some(Candidate {
                            price: p,
                            target: c.supply_or_budget,
                            numerator: cc::sub(c.supply_or_budget, s.cumulative)?,
                            denominator: q,
                        });
                    } else {
                        s.cumulative = total;
                    }
                } else if q > 0 {
                    let cap = cc::mul_div(&env, c.supply_or_budget, cc::SCALE, p, false)?;
                    if cap <= s.cumulative {
                        if s.cumulative > 0 {
                            s.candidate = cc::CandidateValue::Some(Candidate {
                                price: s.previous_price,
                                target: s.cumulative,
                                numerator: 1,
                                denominator: 1,
                            });
                        } else {
                            cancel(&env, &c, &mut s, Reason::NoDemand)?;
                            break;
                        }
                    } else if total >= cap {
                        s.candidate = cc::CandidateValue::Some(Candidate {
                            price: p,
                            target: cap,
                            numerator: cc::sub(cap, s.cumulative)?,
                            denominator: q,
                        });
                    } else {
                        s.cumulative = total;
                        s.previous_price = p;
                    }
                }
                if s.candidate.is_some() {
                    s.stage = Stage::Sweep;
                    s.sweep_cursor = 0;
                    break;
                }
                let end = if c.mode == Mode::Sale {
                    idx == 0
                } else {
                    idx + 1 == cc::tick_count(&c)
                };
                if end {
                    if s.cumulative == 0 {
                        cancel(&env, &c, &mut s, Reason::NoDemand)?;
                    } else {
                        s.candidate = cc::CandidateValue::Some(Candidate {
                            price: if c.mode == Mode::Sale {
                                c.min_price
                            } else {
                                s.previous_price
                            },
                            target: s.cumulative,
                            numerator: 1,
                            denominator: 1,
                        });
                        s.stage = Stage::Sweep;
                        s.sweep_cursor = 0;
                    }
                    break;
                }
                s.tick_cursor = if c.mode == Mode::Sale {
                    idx - 1
                } else {
                    idx + 1
                };
            }
        } else if s.stage == Stage::Sweep {
            let k = s.candidate.clone().ok_or(Error::Phase)?;
            for _ in 0..work_limit.min(cc::BID_BATCH) {
                if s.sweep_cursor == s.bid_count {
                    s.stage = Stage::Ready;
                    break;
                }
                let id = s.sweep_cursor;
                let mut b = bid(&env, id)?;
                processed += 1;
                if contributes_to_price(&c, &b, &k) && !cc::both(&env, &c, &b.owner)? {
                    exclude(&env, &c, &mut s, id, &mut b)?;
                    break;
                }
                s.sweep_cursor += 1;
                if s.sweep_cursor == s.bid_count {
                    s.stage = Stage::Ready;
                    break;
                }
            }
        } else {
            return Err(Error::Phase);
        }
        env.storage().instance().set(&Key::State, &s);
        let out = Progress {
            stage: s.stage,
            epoch: s.epoch,
            cursor: if s.stage == Stage::Sweep {
                s.sweep_cursor
            } else {
                s.tick_cursor
            },
            processed,
        };
        emit(&env, "progress", out.clone());
        leave(&env, &c)?;
        Ok(out)
    }
    pub fn finalize(env: Env) -> Result<Finalization, Error> {
        enter(&env)?;
        let c = cfg(&env);
        let mut s: State = read(&env, &Key::State);
        if s.stage != Stage::Ready {
            return Err(Error::Phase);
        }
        if env.ledger().sequence() >= c.close_deadline {
            return Err(Error::Window);
        }
        let global = cc::both(&env, &c, &env.current_contract_address())?;
        let recipient = if c.mode == Mode::Buyback {
            cc::authorized(&env, &c.rwa_token, &c.buyback_recipient)?
        } else {
            true
        };
        if !global || !recipient {
            cancel(&env, &c, &mut s, Reason::GlobalAuth)?;
            env.storage().instance().set(&Key::State, &s);
            leave(&env, &c)?;
            return Ok(Finalization::Cancelled(Reason::GlobalAuth));
        }
        let k = s.candidate.clone().ok_or(Error::Phase)?;
        let mut filled = 0;
        let mut quote = 0;
        let mut winners = 0;
        for id in 0..s.bid_count {
            let mut b = bid(&env, id)?;
            let (f, payment) = cc::allocation(&env, &c, &b, &k)?;
            if contributes_to_price(&c, &b, &k) && !cc::both(&env, &c, &b.owner)? {
                exclude(&env, &c, &mut s, id, &mut b)?;
                env.storage().instance().set(&Key::State, &s);
                leave(&env, &c)?;
                return Ok(Finalization::Excluded(id, s.epoch));
            }
            if f > 0 {
                if c.mode == Mode::Sale && payment > b.deposit {
                    return Err(Error::Collateral);
                }
                filled = cc::add(filled, f)?;
                quote = cc::add(quote, payment)?;
                winners += 1;
            }
        }
        if filled == 0 || filled < c.min_fill_quantity {
            cancel(&env, &c, &mut s, Reason::Minimum)?;
            env.storage().instance().set(&Key::State, &s);
            leave(&env, &c)?;
            return Ok(Finalization::Cancelled(Reason::Minimum));
        }
        let mut a: Accounting = read(&env, &Key::Accounting);
        if c.mode == Mode::Sale {
            let unused = cc::sub(a.reserve_rwa, filled)?;
            a.reserve_rwa = filled;
            add_credit(&env, &c, &mut a, &c.issuer, &c.rwa_token, unused)?;
        } else {
            let unused = cc::sub(a.reserve_quote, quote)?;
            a.reserve_quote = quote;
            add_credit(&env, &c, &mut a, &c.issuer, &c.quote_token, unused)?;
        }
        let result = Clearing {
            candidate: k,
            filled,
            quote,
            winners,
            epoch: s.epoch,
            ledger: env.ledger().sequence(),
            timestamp: env.ledger().timestamp(),
        };
        s.stage = Stage::Cleared;
        env.storage().instance().set(&Key::Clearing, &result);
        env.storage().instance().set(&Key::State, &s);
        save_accounting(&env, &a)?;
        emit(&env, "cleared", result.clone());
        leave(&env, &c)?;
        Ok(Finalization::Cleared(result))
    }
    pub fn expire(env: Env) -> Result<Reason, Error> {
        enter(&env)?;
        let c = cfg(&env);
        let mut s: State = read(&env, &Key::State);
        let now = env.ledger().sequence();
        let reason = if s.stage == Stage::Created && now >= c.start_ledger {
            Reason::Unfunded
        } else if matches!(
            s.stage,
            Stage::Funded | Stage::Closing | Stage::Sweep | Stage::Ready
        ) && now >= c.close_deadline
        {
            Reason::CloseExpired
        } else {
            return Err(Error::Window);
        };
        cancel(&env, &c, &mut s, reason)?;
        env.storage().instance().set(&Key::State, &s);
        leave(&env, &c)?;
        Ok(reason)
    }
    pub fn claim_bid(env: Env, id: u32) -> Result<Resolution, Error> {
        enter(&env)?;
        let c = cfg(&env);
        let out = resolve(&env, &c, id, false)?;
        leave(&env, &c)?;
        Ok(out)
    }
    pub fn expire_bid(env: Env, id: u32) -> Result<Resolution, Error> {
        enter(&env)?;
        let c = cfg(&env);
        let out = resolve(&env, &c, id, true)?;
        leave(&env, &c)?;
        Ok(out)
    }
    pub fn expire_range(env: Env, start_id: u32, count: u32) -> Result<(u32, u32), Error> {
        enter(&env)?;
        let c = cfg(&env);
        let s: State = read(&env, &Key::State);
        if count == 0 || count > 8 {
            return Err(Error::Work);
        }
        if env.ledger().sequence() < c.delivery_deadline {
            return Err(Error::Window);
        }
        if !matches!(
            s.stage,
            Stage::Cleared | Stage::Cancelled | Stage::Settled | Stage::Refunded
        ) {
            return Err(Error::Phase);
        }
        let mut next = start_id;
        let mut resolved = 0;
        for _ in 0..count {
            if next >= s.bid_count {
                break;
            }
            if bid(&env, next)?.resolution.is_none() {
                resolve(&env, &c, next, true)?;
                resolved += 1;
            }
            next = next.checked_add(1).ok_or(Error::Arithmetic)?;
        }
        leave(&env, &c)?;
        Ok((next, resolved))
    }
    pub fn withdraw(env: Env, owner: Address, asset: Address, amount: i128) -> Result<i128, Error> {
        enter(&env)?;
        owner.require_auth();
        let c = cfg(&env);
        if amount <= 0 {
            return Err(Error::Amount);
        }
        if asset != c.rwa_token && asset != c.quote_token {
            return Err(Error::Asset);
        }
        let key = Key::Credit(owner.clone(), asset.clone());
        let credit = env.storage().persistent().get(&key).unwrap_or(0);
        if amount > credit {
            return Err(Error::Credit);
        }
        let mut a: Accounting = read(&env, &Key::Accounting);
        solvent(&env, &c, &a)?;
        store(&env, &c, &key, &cc::sub(credit, amount)?)?;
        if asset == c.rwa_token {
            a.withdrawn_rwa = cc::add(a.withdrawn_rwa, amount)?;
            a.credit_rwa = cc::sub(a.credit_rwa, amount)?;
            a.liability_rwa = cc::sub(a.liability_rwa, amount)?;
        } else {
            a.withdrawn_quote = cc::add(a.withdrawn_quote, amount)?;
            a.credit_quote = cc::sub(a.credit_quote, amount)?;
            a.liability_quote = cc::sub(a.liability_quote, amount)?;
        }
        save_accounting(&env, &a)?;
        send(&env, &asset, &owner, amount)?;
        emit(&env, "withdrawn", (owner, asset, amount));
        leave(&env, &c)?;
        Ok(amount)
    }
    pub fn maintain(env: Env, keys: Vec<MaintenanceKey>) -> Result<u32, Error> {
        enter(&env)?;
        if keys.is_empty() || keys.len() > 8 {
            return Err(Error::Work);
        }
        let c = cfg(&env);
        let s: State = read(&env, &Key::State);
        let ttl = cc::financial_ttl(&env, &c)?;
        let mut extended = 0;
        for key in keys.iter() {
            let key = match key {
                MaintenanceKey::Bid(id) => {
                    if id >= s.bid_count {
                        return Err(Error::Missing);
                    }
                    Key::Bid(id)
                }
                MaintenanceKey::Tick(index) => {
                    if index >= cc::tick_count(&c) {
                        return Err(Error::Price);
                    }
                    Key::Tick(index)
                }
                MaintenanceKey::Count(owner) => Key::Count(owner),
                MaintenanceKey::OwnerBid(owner, index) => {
                    if index >= c.max_bids_per_address {
                        return Err(Error::Missing);
                    }
                    Key::OwnerBid(owner, index)
                }
                MaintenanceKey::Nonce(owner, nonce) => Key::Nonce(owner, nonce),
                MaintenanceKey::Credit(owner, asset) => {
                    if asset != c.rwa_token && asset != c.quote_token {
                        return Err(Error::Asset);
                    }
                    Key::Credit(owner, asset)
                }
            };
            if env.storage().persistent().has(&key) {
                env.storage().persistent().extend_ttl(&key, ttl / 2, ttl);
                extended += 1;
            }
        }
        leave(&env, &c)?;
        Ok(extended)
    }
}
