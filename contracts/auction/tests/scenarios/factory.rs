use super::*;

#[test]
fn factory_real_bootstrap_rejects_mismatched_pins_and_retries_atomically() {
    // These bootstrap calls always deploy the release artifacts, including in
    // native coverage runs. No native installation adapter proves deployment.
    for mismatch in 0..=4 {
        let mut e = Env::default();
        e.set_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        e.cost_estimate().budget().reset_unlimited();
        let registry_hash = e
            .deployer()
            .upload_contract_wasm(wasm("crossing_price_registry"));
        let factory_hash = e
            .deployer()
            .upload_contract_wasm(wasm("crossing_auction_factory"));
        let auction_hash = e.deployer().upload_contract_wasm(wasm("crossing_auction"));
        let bootstrap = e.register(factory_bootstrap::Bootstrap, ());
        let client = factory_bootstrap::BootstrapClient::new(&e, &bootstrap);
        if mismatch != 0 {
            assert!(client
                .try_deploy(&registry_hash, &factory_hash, &auction_hash, &mismatch)
                .is_err());
        }
        // Reuse both deployment salts after failure: neither contract nor its
        // constructor storage may survive a failed enclosing deployment call.
        let (factory, registry) = client.deploy(&registry_hash, &factory_hash, &auction_hash, &0);
        let factory_client = crossing_auction_factory::AuctionFactoryClient::new(&e, &factory);
        assert_eq!(
            factory_client.bindings(),
            (
                registry.clone(),
                registry_hash.clone(),
                auction_hash.clone()
            )
        );
        assert_eq!(
            crossing_price_registry::PriceRegistryClient::new(&e, &registry).bindings(),
            (factory.clone(), factory_hash.clone(), auction_hash.clone())
        );
        assert_eq!(factory_client.count(), 0);
        assert!(factory_client.list(&0, &1).is_empty());
        assert!(e
            .try_invoke_contract::<(), Error>(
                &factory,
                &Symbol::new(&e, "__constructor"),
                (registry.clone(), registry_hash, auction_hash.clone()).into_val(&e),
            )
            .is_err());
        assert!(e
            .try_invoke_contract::<(), Error>(
                &registry,
                &Symbol::new(&e, "__constructor"),
                (factory.clone(), factory_hash, auction_hash).into_val(&e),
            )
            .is_err());
        assert_eq!(factory_client.count(), 0);
    }
}

#[test]
fn factory_creation_requires_authorization_of_exact_config_and_salt() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, false, 10);
    let salt = BytesN::from_array(&f.e, &[85; 32]);
    let client = crossing_auction_factory::AuctionFactoryClient::new(&f.e, &f.factory);
    let predicted = client.predict(&f.issuer, &salt);
    let wrong_signer = Address::generate(&f.e);
    // Missing auth, a different signer, and approvals for different arguments
    // all fail before a contract, count, metadata, or list entry can be created.
    assert!(client.try_create(&config, &salt).is_err());
    f.signed(
        &wrong_signer,
        &f.factory,
        "create",
        (config.clone(), salt.clone()).into_val(&f.e),
        &[],
    );
    assert!(client.try_create(&config, &salt).is_err());
    f.clear_auth();
    let mut changed = config.clone();
    changed.supply_or_budget += 1;
    for (signed_config, signed_salt) in [
        (changed, salt.clone()),
        (config.clone(), BytesN::from_array(&f.e, &[86; 32])),
    ] {
        f.signed(
            &f.issuer,
            &f.factory,
            "create",
            (signed_config, signed_salt).into_val(&f.e),
            &[],
        );
        assert!(client.try_create(&config, &salt).is_err());
        f.clear_auth();
    }
    assert_eq!(client.count(), 0);
    assert!(client.get_auction(&predicted).is_none());
    assert!(client.list(&0, &1).is_empty());
    assert_eq!(f.create(&config, 85), predicted);
    let meta = client.get_auction(&predicted).unwrap();
    assert_eq!(meta.issuer, config.issuer);
    assert_eq!(meta.rwa, config.rwa_token);
    assert_eq!(meta.quote, config.quote_token);
    assert_eq!(meta.mode, config.mode);
    assert_eq!(meta.config_hash, cc::config_hash(&f.e, &config));
    assert_eq!(meta.template, f.template);
    assert_eq!(meta.ledger, f.e.ledger().sequence());
    assert_eq!(client.count(), 1);
    assert_eq!(client.list(&0, &32), Vec::from_array(&f.e, [predicted]));
}

#[test]
fn factory_creation_rechecks_asset_admin_and_failed_attempt_preserves_salt() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, false, 10);
    let new_admin = Address::generate(&f.e);
    let salt = BytesN::from_array(&f.e, &[87; 32]);
    let client = crossing_auction_factory::AuctionFactoryClient::new(&f.e, &f.factory);
    let predicted = client.predict(&f.issuer, &salt);
    for (old, new) in [(&f.issuer, &new_admin), (&new_admin, &f.issuer)] {
        f.signed(old, &f.rwa, "set_admin", (new.clone(),).into_val(&f.e), &[]);
        token::StellarAssetClient::new(&f.e, &f.rwa).set_admin(new);
        f.clear_auth();
        if new == &new_admin {
            f.signed(
                &f.issuer,
                &f.factory,
                "create",
                (config.clone(), salt.clone()).into_val(&f.e),
                &[],
            );
            assert_eq!(
                client.try_create(&config, &salt),
                Err(Ok(Error::Unauthorized))
            );
            f.clear_auth();
            assert_eq!(client.count(), 0);
            assert!(client.list(&0, &1).is_empty());
            assert!(client.get_auction(&predicted).is_none());
        }
    }
    assert_eq!(f.create(&config, 87), predicted);
    assert_eq!(client.count(), 1);
    assert!(client.get_auction(&predicted).is_some());
}

#[test]
fn factory_post_deployment_counter_fault_rolls_back_contract_and_registration() {
    let f = Fixture::new();
    if f.native_auction {
        return;
    }
    let config = f.config(Mode::Sale, false, 10);
    let salt = BytesN::from_array(&f.e, &[91; 32]);
    let client = crossing_auction_factory::AuctionFactoryClient::new(&f.e, &f.factory);
    let predicted = client.predict(&f.issuer, &salt);
    // This state is not realistically reachable. Inject it to fail immediately
    // after real deploy_v2, proving root rollback also removes the new instance.
    f.e.as_contract(&f.factory, || {
        f.e.storage()
            .instance()
            .set(&factory_bootstrap::FactoryStateKey::Count, &u64::MAX);
    });
    f.signed(
        &f.issuer,
        &f.factory,
        "create",
        (config.clone(), salt.clone()).into_val(&f.e),
        &[],
    );
    assert_eq!(
        client.try_create(&config, &salt),
        Err(Ok(Error::Arithmetic))
    );
    f.clear_auth();
    assert_eq!(client.count(), u64::MAX);
    assert!(client.get_auction(&predicted).is_none());
    assert!(predicted.executable().is_none());
    f.e.as_contract(&f.factory, || {
        assert!(!f
            .e
            .storage()
            .persistent()
            .has(&factory_bootstrap::FactoryStateKey::At(0)));
        assert!(!f
            .e
            .storage()
            .persistent()
            .has(&factory_bootstrap::FactoryStateKey::At(u64::MAX)));
        f.e.storage()
            .instance()
            .set(&factory_bootstrap::FactoryStateKey::Count, &0u64);
    });
    assert_eq!(f.create(&config, 91), predicted);
    assert_eq!(client.count(), 1);
    assert_eq!(client.list(&0, &32), Vec::from_array(&f.e, [predicted]));
}

#[test]
fn factory_salts_are_scoped_to_issuer_network_and_factory() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, false, 10);
    let salt = BytesN::from_array(&f.e, &[88; 32]);
    let client = crossing_auction_factory::AuctionFactoryClient::new(&f.e, &f.factory);
    let predicted = client.predict(&f.issuer, &salt);
    let next_issuer = Address::generate(&f.e);
    let other_predicted = client.predict(&next_issuer, &salt);
    assert_ne!(predicted, other_predicted);
    let mut other = config.clone();
    other.issuer = next_issuer.clone();
    other.buyback_recipient = next_issuer.clone();
    other.rwa_token =
        f.e.register_stellar_asset_contract_v2(next_issuer.clone())
            .address();
    f.signed(
        &next_issuer,
        &f.factory,
        "create",
        (other.clone(), salt.clone()).into_val(&f.e),
        &[],
    );
    assert_eq!(client.create(&other, &salt), other_predicted);
    f.clear_auth();
    assert_eq!(f.create(&config, 88), predicted);
    assert_eq!(client.count(), 2);
    assert_eq!(
        client.list(&0, &32),
        Vec::from_array(&f.e, [other_predicted, predicted.clone()])
    );
    let network = f.e.ledger().network_id();
    f.e.ledger().set_network_id([0xaa; 32]);
    assert_ne!(client.predict(&f.issuer, &salt), predicted);
    f.e.ledger().set_network_id(network.to_array());
    assert_eq!(client.predict(&f.issuer, &salt), predicted);
    // A second real bootstrap creates a distinct factory namespace.
    let bootstrap = f.e.register(factory_bootstrap::Bootstrap, ());
    let (second_factory, _) = factory_bootstrap::BootstrapClient::new(&f.e, &bootstrap).deploy(
        &f.e.deployer()
            .upload_contract_wasm(wasm("crossing_price_registry")),
        &f.e.deployer()
            .upload_contract_wasm(wasm("crossing_auction_factory")),
        &f.e.deployer()
            .upload_contract_wasm(wasm("crossing_auction")),
        &0,
    );
    assert_ne!(
        crossing_auction_factory::AuctionFactoryClient::new(&f.e, &second_factory)
            .predict(&f.issuer, &salt),
        predicted
    );
}

#[test]
fn factory_prediction_duplicate_salt_and_forbidden_participants_are_enforced() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, false, 10);
    let salt = BytesN::from_array(&f.e, &[78; 32]);
    let predicted: Address = f.e.invoke_contract(
        &f.factory,
        &Symbol::new(&f.e, "predict"),
        (f.issuer.clone(), salt.clone()).into_val(&f.e),
    );
    let a = f.auction(&config, 78);
    assert_eq!(a, predicted);
    f.signed(
        &f.issuer,
        &f.factory,
        "create",
        (config.clone(), salt.clone()).into_val(&f.e),
        &[],
    );
    assert_eq!(
        f.e.try_invoke_contract::<Address, Error>(
            &f.factory,
            &Symbol::new(&f.e, "create"),
            (config, salt).into_val(&f.e)
        ),
        Err(Ok(Error::Replay))
    );
    f.clear_auth();
    let client = AuctionClient::new(&f.e, &a);
    f.e.ledger().set_sequence_number(100);
    f.signed(
        &f.issuer,
        &a,
        "place_bid",
        (f.issuer.clone(), cc::SCALE, 1i128).into_val(&f.e),
        &[],
    );
    assert_eq!(
        client.try_place_bid(&f.issuer, &cc::SCALE, &1),
        Err(Ok(Error::Recipient))
    );
    f.clear_auth();
    assert_eq!(client.state().bid_count, 0);
    assert_eq!(client.try_advance(&0), Err(Ok(Error::Work)));
    assert_eq!(client.try_advance(&1), Err(Ok(Error::Window)));
}
