//! Exercise real registry/factory constructors through the deployment host call.
use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env};

// Matching key encoding for an explicitly injected, unreachable counter fault.
#[contracttype]
pub enum FactoryStateKey {
    Count,
    At(u64),
}

#[contract]
pub struct Bootstrap;

#[contractimpl]
impl Bootstrap {
    pub fn deploy(
        env: Env,
        registry_hash: BytesN<32>,
        factory_hash: BytesN<32>,
        auction_hash: BytesN<32>,
        mismatch: u32,
    ) -> (Address, Address) {
        let deployer = env
            .deployer()
            .with_current_contract(BytesN::from_array(&env, &[1; 32]));
        let predicted = deployer.deployed_address();
        let factory_pin = if mismatch == 1 {
            env.current_contract_address()
        } else {
            predicted
        };
        let factory_hash_pin = if mismatch == 2 {
            registry_hash.clone()
        } else {
            factory_hash.clone()
        };
        let template_pin = if mismatch == 3 {
            registry_hash.clone()
        } else {
            auction_hash.clone()
        };
        let registry = env
            .deployer()
            .with_current_contract(BytesN::from_array(&env, &[2; 32]))
            .deploy_v2(
                registry_hash.clone(),
                (factory_pin, factory_hash_pin, template_pin),
            );
        let registry_hash_pin = if mismatch == 4 {
            factory_hash.clone()
        } else {
            registry_hash
        };
        let factory = deployer.deploy_v2(
            factory_hash,
            (registry.clone(), registry_hash_pin, auction_hash),
        );
        (factory, registry)
    }
}
