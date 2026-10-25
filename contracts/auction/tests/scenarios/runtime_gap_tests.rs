//! Bounded worst-case continuation and snapshot recovery. Snapshot restoration
//! exercises the pinned host's recording-mode auto-restoration, not network
//! transaction fees, restoration footprints, or RPC submission acceptance.
use super::*;

#[soroban_sdk::contracttype]
enum RegistryFaultKey {
    Lock,
}

#[test]
fn registry_quote_pairs_stay_isolated_and_publication_failure_cannot_block_delivery() {
    let f = Fixture::new();
    let second_quote =
        f.e.register_stellar_asset_contract_v2(f.quote_admin.clone())
            .address();
    let first_config = f.config(Mode::Sale, false, 1);
    let first = f.auction(&first_config, 0xb2);
    let mut second_config = first_config.clone();
    second_config.quote_token = second_quote.clone();
    let second = f.create(&second_config, 0xb3);
    f.set_auth(&second, &second_quote, true);
    f.set_auth(&f.issuer, &second_quote, true);
    f.fund(&second, &second_config);
    let user = f.user();
    f.set_auth(&user, &second_quote, true);
    f.mint(&user, &second_quote, 10);
    f.e.ledger().set_sequence_number(100);
    f.place(&first, &user, 2, 1);
    f.signed(
        &user,
        &second,
        "place_bid",
        (user.clone(), 3 * cc::SCALE, 1i128).into_val(&f.e),
        &[MockAuthInvoke {
            contract: &second_quote,
            fn_name: "transfer",
            args: (user.clone(), second.clone(), 3i128).into_val(&f.e),
            sub_invokes: &[],
        }],
    );
    AuctionClient::new(&f.e, &second).place_bid(&user, &(3 * cc::SCALE), &1);
    f.clear_auth();
    f.e.ledger().set_sequence_number(110);
    f.close(&first);
    f.close(&second);
    let registry = crossing_price_registry::PriceRegistryClient::new(&f.e, &f.registry);
    // Inject a registry-only publication failure. Escrow remains independently
    // usable and does not read publication locks while delivering either pair.
    f.e.as_contract(&f.registry, || {
        f.e.storage().instance().set(&RegistryFaultKey::Lock, &true)
    });
    assert_eq!(
        registry.try_record_clearing(&first),
        Err(Ok(Error::Reentry))
    );
    assert!(AuctionClient::new(&f.e, &first).claim_bid(&0).delivered);
    assert!(AuctionClient::new(&f.e, &second).claim_bid(&0).delivered);
    f.e.as_contract(&f.registry, || {
        f.e.storage()
            .instance()
            .set(&RegistryFaultKey::Lock, &false)
    });
    for auction in [&second, &first, &second] {
        registry.record_settlement(auction);
    }
    for (quote, auction, price) in [(&f.quote, &first, 2), (&second_quote, &second, 3)] {
        assert_eq!(registry.count(&f.rwa, quote), 1);
        assert_eq!(
            registry.history(&f.rwa, quote, &0, &32),
            Vec::from_array(&f.e, [auction.clone()])
        );
        let observation = registry.latest(&f.rwa, quote).unwrap();
        assert_eq!(observation.auction, *auction);
        assert_eq!(observation.clearing.candidate.price, price * cc::SCALE);
        assert_eq!(
            registry.latest_settled(&f.rwa, quote).unwrap().auction,
            *auction
        );
        assert!(registry.latest(quote, &f.rwa).is_none());
    }
}

#[test]
fn maximum_tick_book_excludes_every_revoked_bid_within_the_published_call_bound() {
    let f = Fixture::new();
    let mut config = f.config(Mode::Sale, false, 1);
    config.max_price = cc::MAX_TICKS as i128 * cc::SCALE;
    let a = f.auction(&config, 0xb0);
    let owners: std::vec::Vec<_> = (0..cc::MAX_BIDS).map(|_| f.user()).collect();
    f.e.ledger().set_sequence_number(100);
    for owner in &owners {
        f.place(&a, owner, 1, 1);
    }
    for owner in &owners {
        f.set_auth(owner, &f.rwa, false);
    }
    f.e.ledger().set_sequence_number(110);
    let client = AuctionClient::new(&f.e, &a);
    let mut calls = 0;
    const BOUND: u32 = 2_227;
    while !matches!(client.state().stage, Stage::Cancelled | Stage::Refunded) {
        assert!(calls < BOUND);
        let progress = client.advance(&cc::TICK_BATCH);
        assert!(progress.processed <= cc::TICK_BATCH);
        calls += 1;
    }
    assert_eq!(calls, 2_200);
    assert_eq!(client.state().epoch, cc::MAX_BIDS + 1);
    assert_eq!(client.tick_total(&0), 0);
    assert!(client.clearing().is_none());
    for (id, owner) in owners.iter().enumerate() {
        assert!(client.get_bid(&(id as u32)).excluded);
        assert_eq!(client.claim_bid(&(id as u32)).refund, 1);
        f.withdraw(&a, owner, &f.quote, 1);
    }
    f.withdraw(&a, &f.issuer, &f.rwa, 1);
    assert_eq!(client.accounting().liability_rwa, 0);
    assert_eq!(client.accounting().liability_quote, 0);
    f.check(&a);
}

#[test]
fn expired_snapshot_restores_owned_credits_bid_nonces_and_publication_guards() {
    // Snapshot cannot serialize native test function tables. Always exercise
    // actual factory/auction/registry WASM and built-in SAC state in this case.
    let f = Fixture::with_mode("");
    let config = f.config(Mode::Sale, true, 3);
    let a = f.auction(&config, 0xb1);
    let alice = f.user();
    let bob = f.user();
    f.e.ledger().set_sequence_number(100);
    let first = f.commit(&a, &alice, 3 * cc::SCALE, 2, 8);
    let second = f.commit(&a, &bob, 2 * cc::SCALE, 2, 4);
    f.commit(&a, &alice, cc::SCALE, 1, 1);
    f.e.ledger().set_sequence_number(110);
    f.reveal(&a, first.0, 3 * cc::SCALE, 2, &first.1);
    f.reveal(&a, second.0, 2 * cc::SCALE, 2, &second.1);
    f.e.ledger().set_sequence_number(115);
    f.close(&a);
    let client = AuctionClient::new(&f.e, &a);
    assert!(client.claim_bid(&0).delivered);
    let clearing = client.clearing().unwrap();
    let registry = crossing_price_registry::PriceRegistryClient::new(&f.e, &f.registry);
    registry.record_clearing(&a);
    let meta = crossing_auction_factory::AuctionFactoryClient::new(&f.e, &f.factory)
        .get_auction(&a)
        .unwrap();
    let saved_addresses = [
        &f.issuer,
        &f.quote_admin,
        &f.rwa,
        &f.quote,
        &f.factory,
        &f.registry,
        &a,
        &alice,
        &bob,
    ]
    .map(soroban_sdk::xdr::ScAddress::from);
    let template = f.template.to_array();
    let mut snapshot = f.e.to_snapshot();
    let expired_at = snapshot
        .ledger
        .ledger_entries
        .iter()
        .filter_map(|(_, (_, live_until))| *live_until)
        .max()
        .unwrap()
        + 1;
    assert!(expired_at > config.delivery_deadline + cc::RECOVERY);
    snapshot.ledger.sequence_number = expired_at;
    snapshot.ledger.timestamp += 10_000_000;
    let mut e = Env::from_snapshot(snapshot);
    e.set_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    e.cost_estimate().budget().reset_unlimited();
    let [issuer, quote_admin, rwa, quote, factory, registry, a, alice, bob] =
        saved_addresses.map(|address| address.try_into_val(&e).unwrap());
    let recovered = Fixture {
        template: BytesN::from_array(&e, &template),
        e,
        issuer,
        quote_admin,
        rwa,
        quote,
        factory,
        registry,
        native_auction: false,
    };
    let client = AuctionClient::new(&recovered.e, &a);
    assert_eq!(client.state().stage, Stage::Cleared);
    let restored_clearing = client.clearing().unwrap();
    assert_eq!(restored_clearing.candidate.price, clearing.candidate.price);
    assert_eq!(restored_clearing.filled, clearing.filled);
    assert_eq!(restored_clearing.ledger, clearing.ledger);
    assert_eq!(client.credit(&alice, &recovered.quote), 4);
    assert_eq!(client.try_claim_bid(&0), Err(Ok(Error::Replay)));
    let unrevealed = client.get_bid(&2);
    let nonce = unrevealed.nonce.unwrap();
    let hash = unrevealed.commitment.unwrap();
    recovered.signed(
        &alice,
        &a,
        "commit_bid",
        (alice.clone(), nonce.clone(), hash.clone(), 1i128).into_val(&recovered.e),
        &[],
    );
    assert_eq!(
        client.try_commit_bid(&alice, &nonce, &hash, &1),
        Err(Ok(Error::Replay))
    );
    recovered.clear_auth();
    let factory_client =
        crossing_auction_factory::AuctionFactoryClient::new(&recovered.e, &recovered.factory);
    assert_eq!(factory_client.count(), 1);
    let restored_meta = factory_client.get_auction(&a).unwrap();
    assert_eq!(
        restored_meta.config_hash.to_array(),
        meta.config_hash.to_array()
    );
    assert_eq!(restored_meta.ledger, meta.ledger);
    let registry =
        crossing_price_registry::PriceRegistryClient::new(&recovered.e, &recovered.registry);
    assert_eq!(registry.count(&recovered.rwa, &recovered.quote), 1);
    assert_eq!(client.expire_range(&0, &8), (3, 2));
    assert_eq!(client.state().stage, Stage::Settled);
    registry.record_settlement(&a);
    registry.record_settlement(&a);
    assert_eq!(registry.count(&recovered.rwa, &recovered.quote), 1);
    assert_eq!(
        registry.get_settlement(&a).unwrap().settlement.executed_rwa,
        2
    );
    for owner in [&recovered.issuer, &alice, &bob] {
        for asset in [&recovered.rwa, &recovered.quote] {
            let credit = client.credit(owner, asset);
            if credit > 0 {
                recovered.withdraw(&a, owner, asset, credit);
            }
        }
    }
    assert_eq!(client.accounting().liability_rwa, 0);
    assert_eq!(client.accounting().liability_quote, 0);
    recovered.check(&a);
}
