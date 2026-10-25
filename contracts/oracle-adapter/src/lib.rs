#![no_std]
use crossing_common::{self as cc, Error, OracleAsset, OraclePriceData, RegistryClient};
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, BytesN, Env};

#[contracttype]
#[derive(Clone)]
enum Key {
    Registry,
    Base,
}
#[contract]
pub struct OracleAdapter;

fn read<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val>>(env: &Env, k: &Key) -> T {
    env.storage().instance().get(k).unwrap()
}

#[contractimpl]
impl OracleAdapter {
    /// `registry` and `base` are fixed for the lifetime of this adapter: one
    /// instance always answers SEP-40 queries against one quote asset. The
    /// registry's WASM hash is pinned once, here. Unlike the registry's own
    /// `source()` check, nothing later lets an untrusted caller supply a
    /// registry or auction address for this contract to verify, so there is
    /// no analogous re-check to repeat on every read. This one-time pin is
    /// safe only because `PriceRegistry` has no self-upgrade entrypoint and
    /// is therefore immutable post-deploy; it is a cross-contract invariant,
    /// not something this contract can detect — if a future `PriceRegistry`
    /// version ever added an upgrade path, every already-deployed
    /// `OracleAdapter` would keep trusting the old pinned hash with no way to
    /// notice a switch short of redeployment.
    pub fn __constructor(
        env: Env,
        registry: Address,
        registry_wasm_hash: BytesN<32>,
        base: Address,
    ) -> Result<(), Error> {
        cc::wasm(&registry, &registry_wasm_hash)?;
        cc::genuine(&base)?;
        if token::Client::new(&env, &base).decimals() != cc::DECIMALS
            || !matches!(
                token::StellarAssetClient::new(&env, &base).try_admin(),
                Ok(Ok(_))
            )
        {
            return Err(Error::Asset);
        }
        env.storage().instance().set(&Key::Registry, &registry);
        env.storage().instance().set(&Key::Base, &base);
        // Reused code can have a shorter remaining lifetime than this instance.
        let ttl = cc::RETENTION.min(env.storage().max_ttl());
        env.storage().instance().extend_ttl(ttl, ttl);
        Ok(())
    }
    /// Renew this adapter's instance and code; registry retention is separate.
    pub fn maintain(env: Env) {
        cc::touch(&env);
    }
    pub fn base(env: Env) -> OracleAsset {
        OracleAsset::Stellar(read(&env, &Key::Base))
    }
    pub fn decimals(_env: Env) -> u32 {
        cc::DECIMALS
    }
    /// Zero signals an event-driven feed: a clearing price is exposed when
    /// its observation is published, including before settlement. A SEP-40
    /// consumer that requires a periodic, nonzero resolution is not a fit
    /// for this feed as it stands today.
    pub fn resolution(_env: Env) -> u32 {
        0
    }
    pub fn lastprice(env: Env, asset: OracleAsset) -> Option<OraclePriceData> {
        let OracleAsset::Stellar(rwa) = asset else {
            return None;
        };
        let registry: Address = read(&env, &Key::Registry);
        let base: Address = read(&env, &Key::Base);
        let observation = RegistryClient::new(&env, &registry).latest(&rwa, &base)?;
        Some(OraclePriceData {
            price: observation.clearing.candidate.price,
            timestamp: observation.clearing.timestamp,
        })
    }
}
