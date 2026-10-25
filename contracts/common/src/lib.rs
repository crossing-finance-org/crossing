#![no_std]

use soroban_sdk::Executable;
use soroban_sdk::{
    contractclient, contracterror, contracttype, token, xdr::ToXdr, Address, BytesN, Env, Symbol,
    Vec, U256,
};

pub const SCALE: i128 = 10_000_000;
pub const DECIMALS: u32 = 7;
pub const MAX_BIDS: u32 = 16;
pub const MAX_NONREVEAL_PENALTY_BPS: u32 = 5_000;
pub const MAX_TICKS: u32 = 4096;
pub const TICK_BATCH: u32 = 32;
pub const BID_BATCH: u32 = 8;
pub const RECOVERY: u32 = 120_960;
pub const RETENTION: u32 = 518_400;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    Phase = 1,
    Window = 2,
    Config = 3,
    Asset = 4,
    Unauthorized = 5,
    Arithmetic = 6,
    Capacity = 7,
    Price = 8,
    Amount = 9,
    Missing = 10,
    Replay = 11,
    Commitment = 12,
    Collateral = 13,
    Query = 14,
    Credit = 15,
    Insolvent = 16,
    Provenance = 17,
    Work = 18,
    Ttl = 19,
    Reentry = 20,
    Recipient = 21,
    Transfer = 22,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Sale,
    Buyback,
}
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BidMode {
    Open,
    Sealed,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub version: u32,
    pub rwa_token: Address,
    pub quote_token: Address,
    pub issuer: Address,
    pub buyback_recipient: Address,
    pub mode: Mode,
    pub bid_mode: BidMode,
    pub nonreveal_penalty_bps: u32,
    pub supply_or_budget: i128,
    pub min_fill_quantity: i128,
    pub min_bid_quantity: i128,
    pub min_deposit: i128,
    pub min_price: i128,
    pub max_price: i128,
    pub tick_size: i128,
    pub max_bids: u32,
    pub max_bids_per_address: u32,
    pub start_ledger: u32,
    pub end_ledger: u32,
    pub reveal_end_ledger: u32,
    pub close_deadline: u32,
    pub delivery_deadline: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provenance {
    pub factory: Address,
    pub registry: Address,
    pub template: BytesN<32>,
    pub config_hash: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionMeta {
    pub issuer: Address,
    pub rwa: Address,
    pub quote: Address,
    pub mode: Mode,
    pub config_hash: BytesN<32>,
    pub template: BytesN<32>,
    pub ledger: u32,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Created,
    Funded,
    Closing,
    Sweep,
    Ready,
    Cleared,
    Cancelled,
    Settled,
    Refunded,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub price: i128,
    pub target: i128,
    pub numerator: i128,
    pub denominator: i128,
}

// Explicit tagged optional records avoid SDK 25's testutils-only
// Option<custom contract type> -> ScVal conversion limitation. No environment
// dependent encoding or test-only production type substitution is used.
macro_rules! optional_record {
    ($name:ident, $inner:ident) => {
        #[contracttype]
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum $name {
            None,
            Some($inner),
        }
        impl $name {
            pub fn is_some(&self) -> bool {
                matches!(self, Self::Some(_))
            }
            pub fn is_none(&self) -> bool {
                !self.is_some()
            }
            pub fn ok_or<E>(self, error: E) -> Result<$inner, E> {
                match self {
                    Self::Some(value) => Ok(value),
                    Self::None => Err(error),
                }
            }
        }
        impl From<Option<$inner>> for $name {
            fn from(value: Option<$inner>) -> Self {
                match value {
                    Some(v) => Self::Some(v),
                    None => Self::None,
                }
            }
        }
    };
}
optional_record!(CandidateValue, Candidate);

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub stage: Stage,
    pub epoch: u32,
    pub bid_count: u32,
    pub tick_cursor: u32,
    pub sweep_cursor: u32,
    pub cumulative: i128,
    pub previous_price: i128,
    pub candidate: CandidateValue,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
    NoDemand,
    Minimum,
    Revoked,
    GlobalAuth,
    CloseExpired,
    Unfunded,
    DeliveryExpired,
    NonReveal,
    NotAllocated,
}

optional_record!(ReasonValue, Reason);

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resolution {
    pub delivered: bool,
    pub rwa: i128,
    pub quote: i128,
    pub refund: i128,
    pub reason: ReasonValue,
    pub ledger: u32,
    pub timestamp: u64,
}

optional_record!(ResolutionValue, Resolution);

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bid {
    pub owner: Address,
    pub deposit: i128,
    pub nonce: Option<BytesN<32>>,
    pub commitment: Option<BytesN<32>>,
    pub price: Option<i128>,
    pub quantity: Option<i128>,
    pub excluded: bool,
    pub resolution: ResolutionValue,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Accounting {
    pub pending_rwa: i128,
    pub pending_quote: i128,
    pub reserve_rwa: i128,
    pub reserve_quote: i128,
    pub credit_rwa: i128,
    pub credit_quote: i128,
    pub liability_rwa: i128,
    pub liability_quote: i128,
    pub admitted_quantity: i128,
    pub admitted_deposit: i128,
    pub resolved: u32,
    pub executed_rwa: i128,
    pub executed_quote: i128,
    pub refunded_rwa: i128,
    pub refunded_quote: i128,
    pub withdrawn_rwa: i128,
    pub withdrawn_quote: i128,
}
impl Accounting {
    pub fn zero() -> Self {
        Self {
            pending_rwa: 0,
            pending_quote: 0,
            reserve_rwa: 0,
            reserve_quote: 0,
            credit_rwa: 0,
            credit_quote: 0,
            liability_rwa: 0,
            liability_quote: 0,
            admitted_quantity: 0,
            admitted_deposit: 0,
            resolved: 0,
            executed_rwa: 0,
            executed_quote: 0,
            refunded_rwa: 0,
            refunded_quote: 0,
            withdrawn_rwa: 0,
            withdrawn_quote: 0,
        }
    }
    pub fn balanced(&self) -> bool {
        [
            self.pending_rwa,
            self.pending_quote,
            self.reserve_rwa,
            self.reserve_quote,
            self.credit_rwa,
            self.credit_quote,
            self.liability_rwa,
            self.liability_quote,
        ]
        .iter()
        .all(|x| *x >= 0)
            && self
                .pending_rwa
                .checked_add(self.reserve_rwa)
                .and_then(|v| v.checked_add(self.credit_rwa))
                == Some(self.liability_rwa)
            && self
                .pending_quote
                .checked_add(self.reserve_quote)
                .and_then(|v| v.checked_add(self.credit_quote))
                == Some(self.liability_quote)
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Clearing {
    pub candidate: Candidate,
    pub filled: i128,
    pub quote: i128,
    pub winners: u32,
    pub epoch: u32,
    pub ledger: u32,
    pub timestamp: u64,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settlement {
    pub executed_rwa: i128,
    pub executed_quote: i128,
    pub refunded_rwa: i128,
    pub refunded_quote: i128,
    pub resolved: u32,
    pub ledger: u32,
    pub timestamp: u64,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Finalization {
    Cleared(Clearing),
    Excluded(u32, u32),
    Cancelled(Reason),
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Progress {
    pub stage: Stage,
    pub epoch: u32,
    pub cursor: u32,
    pub processed: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub auction: Address,
    pub meta: AuctionMeta,
    pub clearing: Clearing,
    pub delivery_deadline: u32,
    pub published_at: u64,
}
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Execution {
    pub auction: Address,
    pub clearing_ledger: u32,
    pub settlement: Settlement,
    pub published_at: u64,
}

#[contractclient(name = "FactoryClient")]
pub trait FactoryApi {
    fn get_auction(env: Env, auction: Address) -> Option<AuctionMeta>;
    fn bindings(env: Env) -> (Address, BytesN<32>, BytesN<32>);
}
#[contractclient(name = "RegistryClient")]
pub trait RegistryApi {
    fn bindings(env: Env) -> (Address, BytesN<32>, BytesN<32>);
    fn latest(env: Env, rwa: Address, quote: Address) -> Option<Observation>;
}

/// SEP-40's asset discriminant. `Other` exists for interface completeness;
/// this registry only ever prices Stellar Asset Contract pairs, so an adapter
/// only ever returns data for `Stellar`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OracleAsset {
    Stellar(Address),
    Other(Symbol),
}
/// SEP-40's `PriceData` shape. `timestamp` is the clearing auction's own
/// ledger-close time, not the time an adapter call happens to run.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OraclePriceData {
    pub price: i128,
    pub timestamp: u64,
}
#[contractclient(name = "AuctionClient")]
pub trait AuctionApi {
    fn config(env: Env) -> Config;
    fn provenance(env: Env) -> Provenance;
    fn state(env: Env) -> State;
    fn clearing(env: Env) -> Option<Clearing>;
    fn settlement(env: Env) -> Option<Settlement>;
}

pub fn add(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_add(b).ok_or(Error::Arithmetic)
}
pub fn sub(a: i128, b: i128) -> Result<i128, Error> {
    let n = a.checked_sub(b).ok_or(Error::Arithmetic)?;
    if n < 0 {
        Err(Error::Arithmetic)
    } else {
        Ok(n)
    }
}
pub fn mul_div(env: &Env, a: i128, b: i128, d: i128, ceil: bool) -> Result<i128, Error> {
    if a < 0 || b < 0 || d <= 0 {
        return Err(Error::Arithmetic);
    }
    let product = U256::from_u128(env, a as u128).mul(&U256::from_u128(env, b as u128));
    let den = U256::from_u128(env, d as u128);
    let mut q = product.div(&den).to_u128().ok_or(Error::Arithmetic)?;
    if ceil && product.rem_euclid(&den) != U256::from_u128(env, 0) {
        q = q.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    i128::try_from(q).map_err(|_| Error::Arithmetic)
}
pub fn cost(env: &Env, mode: Mode, q: i128, p: i128) -> Result<i128, Error> {
    mul_div(env, q, p, SCALE, mode == Mode::Sale)
}
/// Floors in the bidder's favor: a non-revealing bidder is never charged more
/// than the configured proportion of their own locked deposit.
pub fn penalty(env: &Env, deposit: i128, bps: u32) -> Result<i128, Error> {
    if bps == 0 {
        return Ok(0);
    }
    mul_div(env, deposit, bps as i128, 10_000, false)
}
/// Precondition: `c` has already passed `validate_config` (which bounds
/// `tick_size > 0` and `(max_price - min_price) / tick_size < MAX_TICKS`).
/// Given an unvalidated `Config`, the raw division and `as u32` cast below can
/// silently wrap a huge or undefined quotient into a small, wrong count rather
/// than panicking or erroring — there is no runtime check here, by design,
/// since every call site only ever holds an already-validated, immutable
/// `Config` for the lifetime of a deployed auction.
pub fn tick_count(c: &Config) -> u32 {
    ((c.max_price - c.min_price) / c.tick_size) as u32 + 1
}
pub fn tick_index(c: &Config, p: i128) -> Result<u32, Error> {
    if p < c.min_price || p > c.max_price || (p - c.min_price) % c.tick_size != 0 {
        return Err(Error::Price);
    }
    Ok(((p - c.min_price) / c.tick_size) as u32)
}
pub fn validate_bid(c: &Config, p: i128, q: i128) -> Result<u32, Error> {
    if q < c.min_bid_quantity {
        return Err(Error::Amount);
    }
    tick_index(c, p)
}
pub fn genuine(token: &Address) -> Result<(), Error> {
    if token.executable() != Some(Executable::StellarAsset) {
        return Err(Error::Asset);
    }
    Ok(())
}
pub fn wasm(address: &Address, hash: &BytesN<32>) -> Result<(), Error> {
    if address.executable() != Some(Executable::Wasm(hash.clone())) {
        return Err(Error::Provenance);
    }
    Ok(())
}
pub fn authorized(env: &Env, token: &Address, owner: &Address) -> Result<bool, Error> {
    match token::StellarAssetClient::new(env, token).try_authorized(owner) {
        Ok(Ok(v)) => Ok(v),
        _ => Err(Error::Query),
    }
}
pub fn both(env: &Env, c: &Config, owner: &Address) -> Result<bool, Error> {
    // Do not short-circuit away errors on the other asset.
    let rwa = authorized(env, &c.rwa_token, owner)?;
    let quote = authorized(env, &c.quote_token, owner)?;
    Ok(rwa && quote)
}
pub fn config_hash(env: &Env, c: &Config) -> BytesN<32> {
    env.crypto().sha256(&c.clone().to_xdr(env)).into()
}
#[allow(clippy::too_many_arguments)] // The protocol's fixed, domain-bound preimage.
pub fn commitment(
    env: &Env,
    p: &Provenance,
    owner: &Address,
    nonce: &BytesN<32>,
    locked: i128,
    price: i128,
    quantity: i128,
    salt: &BytesN<32>,
) -> BytesN<32> {
    let value = (
        Symbol::new(env, "CROSSING_BID_V1"),
        env.ledger().network_id(),
        env.current_contract_address(),
        owner.clone(),
        nonce.clone(),
        p.config_hash.clone(),
        locked,
        price,
        quantity,
        salt.clone(),
    );
    env.crypto().sha256(&value.to_xdr(env)).into()
}
/// Does not bound the gap between `reveal_end_ledger` and `close_deadline`
/// against the worst-case clearing workload (up to `MAX_TICKS`/`TICK_BATCH`
/// tick-batches plus `MAX_BIDS`/`BID_BATCH` sweep-batches). A config with a
/// near-maximal tick range or bid count and a too-tight closing window passes
/// validation but may not leave enough permissionless `advance()` calls'
/// worth of ledgers to finish clearing before `expire()` forces cancellation.
/// This only harms that auction's own creator (self-inflicted
/// misconfiguration, refundable via cancellation) — never protocol funds —
/// so it is intentionally not a hard validation rule here.
pub fn validate_config(env: &Env, c: &Config) -> Result<(), Error> {
    let now = env.ledger().sequence();
    if c.version != 1
        || c.rwa_token == c.quote_token
        || c.supply_or_budget <= 0
        || c.min_fill_quantity <= 0
        || c.min_bid_quantity <= 0
        || c.min_deposit <= 0
        || c.min_price <= 0
        || c.max_price < c.min_price
        || c.tick_size <= 0
        || (c.max_price - c.min_price) % c.tick_size != 0
        || (c.max_price - c.min_price) / c.tick_size >= MAX_TICKS as i128
        || c.max_bids == 0
        || c.max_bids > MAX_BIDS
        || c.max_bids_per_address == 0
        || c.max_bids_per_address > c.max_bids
        || c.nonreveal_penalty_bps > MAX_NONREVEAL_PENALTY_BPS
        || (c.bid_mode == BidMode::Open && c.nonreveal_penalty_bps != 0)
        || !(now < c.start_ledger
            && c.start_ledger < c.end_ledger
            && c.end_ledger <= c.reveal_end_ledger
            && c.reveal_end_ledger < c.close_deadline
            && c.close_deadline < c.delivery_deadline)
        || (c.bid_mode == BidMode::Open && c.end_ledger != c.reveal_end_ledger)
        || (c.bid_mode == BidMode::Sealed && c.end_ledger == c.reveal_end_ledger)
    {
        return Err(Error::Config);
    }
    let cap = if c.mode == Mode::Sale {
        c.supply_or_budget
    } else {
        mul_div(env, c.supply_or_budget, SCALE, c.min_price, false)?
    };
    if c.min_fill_quantity > cap || mul_div(env, c.min_bid_quantity, c.min_price, SCALE, false)? < 1
    {
        return Err(Error::Config);
    }
    if c.mode == Mode::Sale && c.buyback_recipient != c.issuer {
        return Err(Error::Recipient);
    }
    genuine(&c.rwa_token)?;
    genuine(&c.quote_token)?;
    if token::Client::new(env, &c.rwa_token).decimals() != DECIMALS
        || token::Client::new(env, &c.quote_token).decimals() != DECIMALS
    {
        return Err(Error::Asset);
    }
    let admin = match token::StellarAssetClient::new(env, &c.rwa_token).try_admin() {
        Ok(Ok(a)) => a,
        _ => return Err(Error::Asset),
    };
    if admin != c.issuer {
        return Err(Error::Unauthorized);
    }
    // Native XLM is also a genuine seven-decimal SAC, but has no admin.
    // V1 supports issued assets on both sides; executable/decimals alone
    // cannot distinguish the native SAC from an issued settlement asset.
    if !matches!(
        token::StellarAssetClient::new(env, &c.quote_token).try_admin(),
        Ok(Ok(_))
    ) {
        return Err(Error::Asset);
    }
    financial_ttl(env, c)?;
    Ok(())
}
pub fn financial_ttl(env: &Env, c: &Config) -> Result<u32, Error> {
    let target = c
        .delivery_deadline
        .checked_add(RECOVERY)
        .ok_or(Error::Ttl)?;
    let remaining = target.saturating_sub(env.ledger().sequence());
    if remaining > env.storage().max_ttl() {
        return Err(Error::Ttl);
    }
    // TTL excludes the current ledger: live-until = current + extend_to.
    Ok(remaining.max(RETENTION.min(env.storage().max_ttl())))
}
pub fn touch(env: &Env) {
    let ttl = RETENTION.min(env.storage().max_ttl());
    env.storage().instance().extend_ttl(ttl / 2, ttl);
}

/// Returns `(0, 0)` — not an error — whenever the nominal fill `f` would cost
/// a `payment` that floors to zero (e.g. a tiny pro-rata Buyback fill at a low
/// clearing tick), even though `f` itself may be positive. This is a
/// deliberate anti-dust rule, consistent with `cost()`'s protocol-favoring
/// rounding elsewhere: it means the sum of winners' allocations at the
/// clearing tick can be strictly less than `candidate.target` by rounding
/// alone, never more.
pub fn allocation(env: &Env, c: &Config, b: &Bid, k: &Candidate) -> Result<(i128, i128), Error> {
    if b.excluded {
        return Ok((0, 0));
    }
    let (p, q) = match (b.price, b.quantity) {
        (Some(p), Some(q)) => (p, q),
        _ => return Ok((0, 0)),
    };
    let better = if c.mode == Mode::Sale {
        p > k.price
    } else {
        p < k.price
    };
    let f = if better {
        q
    } else if p == k.price {
        mul_div(env, q, k.numerator, k.denominator, false)?
    } else {
        0
    };
    let payment = cost(env, c.mode, f, k.price)?;
    if payment == 0 {
        Ok((0, 0))
    } else {
        Ok((f, payment))
    }
}

pub fn ids(env: &Env, start: u64, limit: u32, end: u64) -> Result<Vec<u64>, Error> {
    if limit == 0 || limit > 32 {
        return Err(Error::Work);
    }
    let mut out = Vec::new(env);
    let mut id = start;
    for _ in 0..limit {
        if id >= end {
            break;
        }
        out.push_back(id);
        id = id.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    Ok(out)
}

#[cfg(test)]
mod test;
