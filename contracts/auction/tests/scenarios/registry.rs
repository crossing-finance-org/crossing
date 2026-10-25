use super::*;

#[test]
fn registry_uses_source_order_and_separates_zero_execution_from_latest_trade() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, false, 1);
    let old = f.auction(&config, 70);
    let new = f.auction(&config, 71);
    let zero = f.auction(&config, 72);
    let zero_owner = f.user();
    f.e.ledger().set_sequence_number(100);
    for (a, price) in [(&old, 1), (&new, 2)] {
        let user = f.user();
        f.place(a, &user, price, 1);
    }
    f.place(&zero, &zero_owner, 3, 1);
    for (a, ledger) in [(&old, 110), (&new, 111), (&zero, 112)] {
        f.e.ledger().set_sequence_number(ledger);
        f.close(a);
        if a == &zero {
            f.set_auth(&zero_owner, &f.rwa, false);
        }
        AuctionClient::new(&f.e, a).claim_bid(&0);
    }
    f.e.ledger().set_sequence_number(120);
    f.e.ledger().set_timestamp(1_800_001_000);
    for a in [&new, &old, &zero, &old] {
        let _: Address = f.e.invoke_contract(
            &f.registry,
            &Symbol::new(&f.e, "record_settlement"),
            (a.clone(),).into_val(&f.e),
        );
    }
    let count: u64 = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "count"),
        (f.rwa.clone(), f.quote.clone()).into_val(&f.e),
    );
    assert_eq!(count, 3);
    let latest: Option<Observation> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "latest"),
        (f.rwa.clone(), f.quote.clone()).into_val(&f.e),
    );
    let latest = latest.unwrap();
    assert_eq!(latest.auction, zero);
    assert_eq!(latest.clearing.ledger, 112);
    assert_eq!(latest.clearing.timestamp, 1_800_000_000);
    assert_eq!(latest.published_at, 1_800_001_000);
    let traded: Option<cc::Execution> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "latest_settled"),
        (f.rwa.clone(), f.quote.clone()).into_val(&f.e),
    );
    assert_eq!(traded.unwrap().auction, new);
    let failed: Option<cc::Execution> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "get_settlement"),
        (zero,).into_val(&f.e),
    );
    assert_eq!(failed.unwrap().settlement.executed_rwa, 0);
    let wrong_pair: Option<Observation> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "latest"),
        (f.quote.clone(), f.rwa.clone()).into_val(&f.e),
    );
    assert!(wrong_pair.is_none());
    let maintained: u32 = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "maintain"),
        (f.rwa.clone(), f.quote.clone(), 0u64, 3u32).into_val(&f.e),
    );
    assert_eq!(maintained, 3);
    let maintained: u32 = f.e.invoke_contract(
        &f.factory,
        &Symbol::new(&f.e, "maintain"),
        (0u64, 3u32).into_val(&f.e),
    );
    assert_eq!(maintained, 3);
}

#[test]
fn registry_same_ledger_ties_use_raw_contract_id_in_both_publication_orders() {
    for reverse in [false, true] {
        let f = Fixture::new();
        let config = f.config(Mode::Sale, false, 1);
        let a = f.auction(&config, 89);
        let b = f.auction(&config, 90);
        let user = f.user();
        f.e.ledger().set_sequence_number(100);
        f.place(&a, &user, 1, 1);
        f.place(&b, &user, 2, 1);
        f.e.ledger().set_sequence_number(110);
        for auction in [&a, &b] {
            f.close(auction);
            assert!(AuctionClient::new(&f.e, auction).claim_bid(&0).delivered);
        }
        // Both are contract addresses, so their canonical XDR prefixes are
        // identical; comparing these byte vectors independently orders raw IDs.
        let raw_a: std::vec::Vec<_> = a.clone().to_xdr(&f.e).iter().collect();
        let raw_b: std::vec::Vec<_> = b.clone().to_xdr(&f.e).iter().collect();
        let (lower, higher) = if raw_a < raw_b { (a, b) } else { (b, a) };
        let order = if reverse {
            [higher.clone(), lower.clone()]
        } else {
            [lower.clone(), higher.clone()]
        };
        let registry = crossing_price_registry::PriceRegistryClient::new(&f.e, &f.registry);
        for auction in &order {
            registry.record_clearing(auction);
            registry.record_settlement(auction);
        }
        assert_eq!(registry.latest(&f.rwa, &f.quote).unwrap().auction, higher);
        assert_eq!(
            registry.latest_settled(&f.rwa, &f.quote).unwrap().auction,
            higher
        );
        assert_eq!(registry.count(&f.rwa, &f.quote), 2);
        assert_eq!(
            registry.history(&f.rwa, &f.quote, &0, &32),
            Vec::from_array(&f.e, order)
        );
        registry.record_clearing(&lower);
        registry.record_settlement(&lower);
        assert_eq!(registry.count(&f.rwa, &f.quote), 2);
        assert_eq!(registry.latest(&f.rwa, &f.quote).unwrap().auction, higher);
        assert!(registry.latest(&f.quote, &f.rwa).is_none());
        assert!(registry.latest_settled(&f.quote, &f.rwa).is_none());
        assert_eq!(registry.count(&f.quote, &f.rwa), 0);
        assert!(registry.history(&f.quote, &f.rwa, &0, &32).is_empty());
    }
}

#[test]
fn oracle_adapter_reports_the_latest_clearing_price_for_its_fixed_base() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, false, 1);
    let a = f.auction(&config, 91);
    let user = f.user();
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &user, 2, 1);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    AuctionClient::new(&f.e, &a).claim_bid(&0);
    let registry = crossing_price_registry::PriceRegistryClient::new(&f.e, &f.registry);
    registry.record_clearing(&a);
    let recorded = registry.latest(&f.rwa, &f.quote).unwrap();
    // The registry's own executable identity depends on how this run deployed
    // it: native coverage modes give it the SDK's synthetic marker, not a real
    // uploaded WASM hash. Match whichever one `f.registry` actually has, the
    // same way `Fixture::new` itself branches, so the adapter's one-time
    // provenance pin succeeds under every `CROSSING_TEST_MODE`.
    let native_services = !std::env::var("CROSSING_TEST_MODE")
        .unwrap_or_default()
        .is_empty();
    let registry_hash = if native_services {
        f.e.crypto().sha256(&soroban_sdk::Bytes::new(&f.e)).into()
    } else {
        f.e.deployer()
            .upload_contract_wasm(wasm("crossing_price_registry"))
    };
    let adapter_args = (f.registry.clone(), registry_hash, f.quote.clone());
    let adapter_address = if native_services {
        f.e.register(crossing_oracle_adapter::OracleAdapter, adapter_args)
    } else {
        f.e.register(wasm("crossing_oracle_adapter"), adapter_args)
    };
    let adapter = crossing_oracle_adapter::OracleAdapterClient::new(&f.e, &adapter_address);
    assert_eq!(adapter.base(), cc::OracleAsset::Stellar(f.quote.clone()));
    assert_eq!(adapter.decimals(), 7);
    assert_eq!(adapter.resolution(), 0);
    let price = adapter
        .lastprice(&cc::OracleAsset::Stellar(f.rwa.clone()))
        .unwrap();
    assert_eq!(price.price, recorded.clearing.candidate.price);
    assert_eq!(price.timestamp, recorded.clearing.timestamp);
    // A pair with no recorded clearing yet reports no price.
    assert!(adapter
        .lastprice(&cc::OracleAsset::Stellar(f.quote.clone()))
        .is_none());
    // The `Other` variant is accepted by the interface but never resolves,
    // since this registry only ever prices Stellar Asset Contract pairs.
    assert!(adapter
        .lastprice(&cc::OracleAsset::Other(Symbol::new(&f.e, "XLM")))
        .is_none());
}

#[test]
fn registry_failed_early_publication_rolls_back_indexes_and_duplicate_guards() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, false, 2);
    let a = f.auction(&config, 77);
    let client = AuctionClient::new(&f.e, &a);
    let args = (a.clone(),).into_val(&f.e);
    assert_eq!(
        f.e.try_invoke_contract::<Address, Error>(
            &f.registry,
            &Symbol::new(&f.e, "record_clearing"),
            args
        ),
        Err(Ok(Error::Phase))
    );
    let user = f.user();
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &user, 1, 1);
    f.place(&a, &user, 1, 1);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    client.claim_bid(&0);
    assert_eq!(
        f.e.try_invoke_contract::<Address, Error>(
            &f.registry,
            &Symbol::new(&f.e, "record_settlement"),
            (a.clone(),).into_val(&f.e)
        ),
        Err(Ok(Error::Phase))
    );
    let record: Option<Observation> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "get_clearing"),
        (a.clone(),).into_val(&f.e),
    );
    assert!(record.is_none());
    let count: u64 = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "count"),
        (f.rwa.clone(), f.quote.clone()).into_val(&f.e),
    );
    assert_eq!(count, 0);
    let _: Address = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "record_clearing"),
        (a.clone(),).into_val(&f.e),
    );
    client.claim_bid(&1);
    let _: Address = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "record_settlement"),
        (a.clone(),).into_val(&f.e),
    );
    for count in [0u32, 9] {
        assert_eq!(
            f.e.try_invoke_contract::<u32, Error>(
                &f.registry,
                &Symbol::new(&f.e, "maintain"),
                (f.rwa.clone(), f.quote.clone(), 0u64, count).into_val(&f.e)
            ),
            Err(Ok(Error::Work))
        );
        assert_eq!(
            f.e.try_invoke_contract::<u32, Error>(
                &f.factory,
                &Symbol::new(&f.e, "maintain"),
                (0u64, count).into_val(&f.e)
            ),
            Err(Ok(Error::Work))
        );
    }
    let count: u64 = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "count"),
        (f.rwa.clone(), f.quote.clone()).into_val(&f.e),
    );
    assert_eq!(count, 1);
}
