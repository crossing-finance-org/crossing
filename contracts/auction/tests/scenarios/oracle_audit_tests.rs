use super::*;

#[contract]
struct AdapterDeployer;

#[contractimpl]
impl AdapterDeployer {
    pub fn deploy(
        env: Env,
        adapter_hash: BytesN<32>,
        registry: Address,
        registry_hash: BytesN<32>,
        base: Address,
    ) -> Address {
        env.deployer()
            .with_current_contract(BytesN::from_array(&env, &[0xa7; 32]))
            .deploy_v2(adapter_hash, (registry, registry_hash, base))
    }
}

fn registry_hash(f: &Fixture) -> BytesN<32> {
    match f.registry.executable() {
        Some(soroban_sdk::Executable::Wasm(hash)) => hash,
        _ => panic!("fixture registry must be WASM or the native WASM marker"),
    }
}

fn register_adapter(f: &Fixture) -> Address {
    let args = (f.registry.clone(), registry_hash(f), f.quote.clone());
    if std::env::var("CROSSING_TEST_MODE")
        .unwrap_or_default()
        .is_empty()
    {
        f.e.register(wasm("crossing_oracle_adapter"), args)
    } else {
        f.e.register(crossing_oracle_adapter::OracleAdapter, args)
    }
}

#[test]
fn oracle_constructor_rejects_wrong_pins_and_unsupported_bases_atomically() {
    // These negative constructors always run real WASM through deploy_v2,
    // including when the surrounding factory/registry fixture runs natively.
    for invalid in 0..3 {
        let f = Fixture::new();
        let code =
            f.e.deployer()
                .upload_contract_wasm(wasm("crossing_oracle_adapter"));
        let deployer = f.e.register(AdapterDeployer, ());
        let correct_hash = registry_hash(&f);
        let mut hash = correct_hash.clone();
        let mut base = f.quote.clone();
        match invalid {
            0 => {
                hash = BytesN::from_array(&f.e, &[0xab; 32]);
                assert_ne!(hash, correct_hash);
            }
            1 => base = f.registry.clone(), // An arbitrary WASM is not a SAC.
            _ => {
                let asset = soroban_sdk::xdr::WriteXdr::to_xdr(
                    &soroban_sdk::xdr::Asset::Native,
                    soroban_sdk::xdr::Limits::none(),
                )
                .unwrap();
                base =
                    f.e.deployer()
                        .with_stellar_asset(soroban_sdk::Bytes::from_slice(&f.e, &asset))
                        .deploy();
            }
        }
        let predicted =
            f.e.deployer()
                .with_address(deployer.clone(), BytesN::from_array(&f.e, &[0xa7; 32]))
                .deployed_address();
        let client = AdapterDeployerClient::new(&f.e, &deployer);
        assert!(client.try_deploy(&code, &f.registry, &hash, &base).is_err());
        assert!(predicted.executable().is_none());
        if invalid == 2 {
            // Execute the same native-SAC rejection in native Rust as well,
            // so source coverage observes this handwritten validation branch.
            // This direct test-host call checks validation only; deploy_v2 above
            // establishes actual deployment failure, and reinvocation below
            // separately checks the host's reserved-constructor protection.
            let native_address = f.e.register(
                crossing_oracle_adapter::OracleAdapter,
                (f.registry.clone(), correct_hash.clone(), f.quote.clone()),
            );
            let result = f.e.as_contract(&native_address, || {
                crossing_oracle_adapter::OracleAdapter::__constructor(
                    f.e.clone(),
                    f.registry.clone(),
                    correct_hash.clone(),
                    base.clone(),
                )
            });
            assert_eq!(result, Err(Error::Asset));
            assert_eq!(
                crossing_oracle_adapter::OracleAdapterClient::new(&f.e, &native_address).base(),
                cc::OracleAsset::Stellar(f.quote.clone())
            );
        }
        let address = client.deploy(&code, &f.registry, &correct_hash, &f.quote);
        assert_eq!(address, predicted);
        let adapter = crossing_oracle_adapter::OracleAdapterClient::new(&f.e, &address);
        assert_eq!(adapter.base(), cc::OracleAsset::Stellar(f.quote.clone()));
        assert!(f
            .e
            .try_invoke_contract::<(), Error>(
                &address,
                &Symbol::new(&f.e, "__constructor"),
                (f.registry.clone(), correct_hash, f.rwa.clone()).into_val(&f.e),
            )
            .is_err());
        assert_eq!(adapter.base(), cc::OracleAsset::Stellar(f.quote.clone()));
    }
}

#[test]
fn oracle_reused_wasm_and_permissionless_maintenance_retain_instance_and_code() {
    let f = Fixture::new();
    // Keep this code real in every fixture mode: native contracts share a
    // synthetic code entry and cannot establish shared-WASM aging behavior.
    let code =
        f.e.deployer()
            .upload_contract_wasm(wasm("crossing_oracle_adapter"));
    let first_deployer = f.e.register(AdapterDeployer, ());
    let first = AdapterDeployerClient::new(&f.e, &first_deployer).deploy(
        &code,
        &f.registry,
        &registry_hash(&f),
        &f.quote,
    );
    let target = cc::RETENTION.min(f.e.storage().max_ttl());
    assert_eq!(f.e.deployer().get_contract_code_ttl(&first), target);
    let age = target / 3;
    f.e.ledger().set_sequence_number(90 + age);
    let remaining = f.e.deployer().get_contract_code_ttl(&first);
    assert!(remaining > target / 2 && remaining < target);
    let second_deployer = f.e.register(AdapterDeployer, ());
    let second = AdapterDeployerClient::new(&f.e, &second_deployer).deploy(
        &code,
        &f.registry,
        &registry_hash(&f),
        &f.quote,
    );
    assert_eq!(f.e.deployer().get_contract_instance_ttl(&second), target);
    assert_eq!(f.e.deployer().get_contract_code_ttl(&second), target);

    f.e.ledger().set_sequence_number(90 + age + target / 2 + 1);
    assert!(f.e.deployer().get_contract_instance_ttl(&second) < target / 2);
    assert!(f.e.deployer().get_contract_code_ttl(&second) < target / 2);
    // No issuer signature or registry access is needed to keep the adapter live.
    f.clear_auth();
    crossing_oracle_adapter::OracleAdapterClient::new(&f.e, &second).maintain();
    assert_eq!(f.e.deployer().get_contract_instance_ttl(&second), target);
    assert_eq!(f.e.deployer().get_contract_code_ttl(&second), target);
}

#[test]
fn oracle_native_or_wasm_maintenance_renews_without_changing_bindings() {
    let f = Fixture::new();
    let address = register_adapter(&f);
    let target = cc::RETENTION.min(f.e.storage().max_ttl());
    f.e.ledger().set_sequence_number(90 + target / 2 + 1);
    f.clear_auth();
    let adapter = crossing_oracle_adapter::OracleAdapterClient::new(&f.e, &address);
    adapter.maintain();
    assert_eq!(f.e.deployer().get_contract_instance_ttl(&address), target);
    assert_eq!(f.e.deployer().get_contract_code_ttl(&address), target);
    assert_eq!(adapter.base(), cc::OracleAsset::Stellar(f.quote.clone()));
    assert_eq!(adapter.decimals(), cc::DECIMALS);
    assert_eq!(adapter.resolution(), 0);
}

#[test]
fn oracle_preserves_source_time_and_reports_raw_pre_and_zero_execution_clearing() {
    let f = Fixture::new();
    let address = register_adapter(&f);
    let adapter = crossing_oracle_adapter::OracleAdapterClient::new(&f.e, &address);
    let asset = cc::OracleAsset::Stellar(f.rwa.clone());
    assert!(adapter.lastprice(&asset).is_none());
    let config = f.config(Mode::Sale, false, 1);
    let auction = f.auction(&config, 0xa8);
    let bidder = f.user();
    f.e.ledger().set_sequence_number(100);
    f.place(&auction, &bidder, 2, 1);
    f.e.ledger().set_sequence_number(110);
    let source_time = 1_800_000_123;
    f.e.ledger().set_timestamp(source_time);
    f.close(&auction);
    let auction_client = AuctionClient::new(&f.e, &auction);
    assert_eq!(auction_client.state().stage, Stage::Cleared);
    assert_eq!(auction_client.accounting().executed_rwa, 0);
    assert!(adapter.lastprice(&asset).is_none());

    f.e.ledger().set_sequence_number(120);
    f.e.ledger().set_timestamp(source_time + 500);
    let registry = crossing_price_registry::PriceRegistryClient::new(&f.e, &f.registry);
    registry.record_clearing(&auction);
    let observation = registry.latest(&f.rwa, &f.quote).unwrap();
    assert_eq!(observation.published_at, source_time + 500);
    let expected = cc::OraclePriceData {
        price: 2 * cc::SCALE,
        timestamp: source_time,
    };
    // This is deliberately a raw clearing observation, before any delivery.
    assert_eq!(adapter.lastprice(&asset), Some(expected.clone()));

    f.e.ledger().set_sequence_number(config.delivery_deadline);
    f.e.ledger().set_timestamp(source_time + 1_000);
    assert!(!auction_client.expire_bid(&0).delivered);
    assert_eq!(auction_client.state().stage, Stage::Settled);
    assert_eq!(auction_client.settlement().unwrap().executed_rwa, 0);
    registry.record_settlement(&auction);
    assert_eq!(
        registry
            .get_settlement(&auction)
            .unwrap()
            .settlement
            .executed_rwa,
        0
    );
    assert!(registry.latest_settled(&f.rwa, &f.quote).is_none());
    // Neither zero actual execution nor later reads refresh the source time.
    assert_eq!(adapter.lastprice(&asset), Some(expected));
    assert!(adapter
        .lastprice(&cc::OracleAsset::Stellar(f.quote.clone()))
        .is_none());
    f.check(&auction);
}
