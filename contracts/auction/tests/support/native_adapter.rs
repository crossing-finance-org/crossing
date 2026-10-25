//! Native line-coverage fixture only; never compiled into a contract artifact.
//! SDK native contracts use SHA256(empty) as an executable marker. Production
//! factory deployment cannot install a native Rust function table for a new
//! address. This adapter performs that single test-host installation step while
//! the real factory is separately exercised with real WASM auction deployment.
use crossing_common::{self as cc, AuctionMeta, Config, Error};
use soroban_sdk::{
    contracttype, testutils::ContractFunctionSet, Address, BytesN, Env, IntoVal, TryFromVal, Val,
};

#[contracttype]
enum FactoryKey {
    Count,
    Auction(Address),
    At(u64),
}

pub struct NativeAuctionFactory;
impl ContractFunctionSet for NativeAuctionFactory {
    fn call(&self, function: &str, env: Env, args: &[Val]) -> Option<Val> {
        if function != "create" {
            return crossing_auction_factory::AuctionFactory.call(function, env, args);
        }
        let result = (|| -> Result<Address, Error> {
            let config = Config::try_from_val(&env, &args[0]).unwrap();
            let user_salt = BytesN::<32>::try_from_val(&env, &args[1]).unwrap();
            config.issuer.require_auth();
            cc::validate_config(&env, &config)?;
            let (registry, _, template) =
                crossing_auction_factory::AuctionFactory::bindings(env.clone());
            let predicted = crossing_auction_factory::AuctionFactory::predict(
                env.clone(),
                config.issuer.clone(),
                user_salt,
            );
            if env
                .storage()
                .persistent()
                .has(&FactoryKey::Auction(predicted.clone()))
            {
                return Err(Error::Replay);
            }
            env.register_at(
                &predicted,
                crossing_auction::Auction,
                (
                    config.clone(),
                    env.current_contract_address(),
                    registry,
                    template.clone(),
                ),
            );
            let count = crossing_auction_factory::AuctionFactory::count(env.clone());
            let meta = AuctionMeta {
                issuer: config.issuer.clone(),
                rwa: config.rwa_token.clone(),
                quote: config.quote_token.clone(),
                mode: config.mode,
                config_hash: cc::config_hash(&env, &config),
                template,
                ledger: env.ledger().sequence(),
            };
            env.storage()
                .persistent()
                .set(&FactoryKey::Auction(predicted.clone()), &meta);
            env.storage()
                .persistent()
                .set(&FactoryKey::At(count), &predicted);
            env.storage()
                .instance()
                .set(&FactoryKey::Count, &(count + 1));
            Ok(predicted)
        })();
        Some(match result {
            Ok(address) => address.into_val(&env),
            Err(error) => env.panic_with_error(error),
        })
    }
}
