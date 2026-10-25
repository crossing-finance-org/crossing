use super::*;

#[test]
fn closing_expiry_and_late_funding_are_deterministic() {
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, 10);
    let a = f.auction(&c, 9);
    let dormant = f.create(&c, 10);
    let u = f.user();
    let client = AuctionClient::new(&f.e, &a);
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &u, 5, 1);
    let dormant_client = AuctionClient::new(&f.e, &dormant);
    dormant_client.expire();
    assert_eq!(dormant_client.state().stage, Stage::Refunded);
    f.e.ledger().set_sequence_number(140);
    assert!(client.try_advance(&32).is_err());
    client.expire();
    client.claim_bid(&0);
    assert_eq!(client.credit(&u, &f.quote), 5);
    assert_eq!(client.credit(&f.issuer, &f.rwa), 10);
    f.check(&a);
}

#[test]
fn expiry_batches_overlap_safely_and_preserve_all_credits() {
    let f = Fixture::new();
    let a = f.auction(&f.config(Mode::Sale, false, 10), 73);
    let client = AuctionClient::new(&f.e, &a);
    let user = f.user();
    f.e.ledger().set_sequence_number(100);
    assert_eq!(client.timing(), (false, false));
    for _ in 0..9 {
        f.place(&a, &user, 1, 1);
    }
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    client.claim_bid(&0);
    client.claim_bid(&1);
    let before = client.accounting();
    assert_eq!(client.try_expire_range(&0, &8), Err(Ok(Error::Window)));
    assert_eq!(client.accounting(), before);
    f.e.ledger().set_sequence_number(160);
    assert_eq!(client.timing(), (true, true));
    assert_eq!(client.try_expire_range(&0, &9), Err(Ok(Error::Work)));
    assert_eq!(client.try_claim_bid(&8), Err(Ok(Error::Window)));
    assert_eq!(client.expire_range(&0, &8), (8, 6));
    assert_eq!(client.expire_range(&6, &8), (9, 1));
    let summary = client.settlement().unwrap();
    assert_eq!(client.expire_range(&0, &8), (8, 0));
    assert_eq!(client.expire_range(&9, &8), (9, 0));
    assert_eq!(client.settlement().unwrap(), summary);
    assert_eq!(summary.executed_rwa, 2);
    assert_eq!(summary.refunded_quote, 7);
    assert_eq!(client.credit(&user, &f.quote), 7);
    assert_eq!(client.credit(&f.issuer, &f.rwa), 8);
    assert_eq!(client.credit(&f.issuer, &f.quote), 2);
    f.withdraw(&a, &user, &f.quote, 7);
    f.withdraw(&a, &f.issuer, &f.rwa, 8);
    f.withdraw(&a, &f.issuer, &f.quote, 2);
    assert_eq!(client.accounting().liability_rwa, 0);
    assert_eq!(client.accounting().liability_quote, 0);
    f.check(&a);
}

#[test]
fn public_getters_pagination_and_typed_maintenance_are_bounded() {
    use crossing_auction::MaintenanceKey as M;
    let f = Fixture::new();
    let a = f.auction(&f.config(Mode::Sale, false, 10), 74);
    let client = AuctionClient::new(&f.e, &a);
    let user = f.user();
    let unrelated = Address::generate(&f.e);
    assert_eq!(client.cancel_reason(), cc::ReasonValue::None);
    assert_eq!(client.try_credit(&user, &unrelated), Err(Ok(Error::Asset)));
    assert_eq!(client.try_tick_total(&20), Err(Ok(Error::Price)));
    assert_eq!(client.try_get_bid(&0), Err(Ok(Error::Missing)));
    f.e.ledger().set_sequence_number(100);
    for (price, quantity, error) in [
        (0i128, 1i128, Error::Price),
        (cc::SCALE + 1, 1, Error::Price),
        (cc::SCALE, 0, Error::Amount),
    ] {
        f.signed(
            &user,
            &a,
            "place_bid",
            (user.clone(), price, quantity).into_val(&f.e),
            &[],
        );
        assert_eq!(
            client.try_place_bid(&user, &price, &quantity),
            Err(Ok(error))
        );
        f.clear_auth();
    }
    for _ in 0..3 {
        f.place(&a, &user, 1, 1);
    }
    assert_eq!(
        client.bid_ids(&user, &0, &2),
        soroban_sdk::vec![&f.e, 0u32, 1]
    );
    assert_eq!(client.bid_ids(&user, &2, &2), soroban_sdk::vec![&f.e, 2u32]);
    assert!(client.bid_ids(&user, &99, &2).is_empty());
    assert_eq!(client.try_bid_ids(&user, &0, &0), Err(Ok(Error::Work)));
    assert_eq!(client.try_bid_ids(&user, &0, &33), Err(Ok(Error::Work)));
    let listed: Vec<Address> = f.e.invoke_contract(
        &f.factory,
        &Symbol::new(&f.e, "list"),
        (0u64, 32u32).into_val(&f.e),
    );
    assert_eq!(listed, soroban_sdk::vec![&f.e, a.clone()]);
    let empty: Vec<Address> = f.e.invoke_contract(
        &f.factory,
        &Symbol::new(&f.e, "list"),
        (99u64, 32u32).into_val(&f.e),
    );
    assert!(empty.is_empty());
    let keys = soroban_sdk::vec![
        &f.e,
        M::Bid(0),
        M::Tick(0),
        M::Count(user.clone()),
        M::OwnerBid(user.clone(), 0),
        M::Nonce(user.clone(), BytesN::from_array(&f.e, &[1; 32])),
        M::Credit(user.clone(), f.quote.clone())
    ];
    assert_eq!(client.maintain(&keys), 4);
    assert_eq!(client.try_maintain(&Vec::new(&f.e)), Err(Ok(Error::Work)));
    for (key, error) in [
        (M::Bid(3), Error::Missing),
        (M::Tick(20), Error::Price),
        (M::OwnerBid(user.clone(), 16), Error::Missing),
        (M::Credit(user.clone(), unrelated.clone()), Error::Asset),
    ] {
        assert_eq!(
            client.try_maintain(&soroban_sdk::vec![&f.e, key]),
            Err(Ok(error))
        );
    }
    // A batch mixing a real, existing key with a structurally invalid one
    // must fail with the invalid key's error for the whole call -- not
    // silently extend the valid key's TTL and skip the bad one. `return`
    // inside the match propagates out of `maintain` itself, and a Soroban
    // invocation that returns `Err` persists nothing, so this also is a
    // direct regression test for that atomicity, not just an inspection.
    assert_eq!(
        client.try_maintain(&soroban_sdk::vec![&f.e, M::Bid(0), M::Tick(20)]),
        Err(Ok(Error::Price))
    );
    assert_eq!(
        client.try_maintain(&soroban_sdk::vec![
            &f.e,
            M::OwnerBid(user.clone(), 16),
            M::Bid(0)
        ]),
        Err(Ok(Error::Missing))
    );
    // The same valid key alone still succeeds, confirming the failures above
    // were caused by the invalid key, not some other side effect.
    assert_eq!(client.maintain(&soroban_sdk::vec![&f.e, M::Bid(0)]), 1);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    for id in 0..3 {
        client.claim_bid(&id);
    }
    assert_eq!(client.try_claim_bid(&0), Err(Ok(Error::Replay)));
    assert_eq!(client.try_advance(&1), Err(Ok(Error::Phase)));
    assert_eq!(client.try_finalize(), Err(Ok(Error::Phase)));
    assert_eq!(client.try_expire(), Err(Ok(Error::Window)));
    for (asset, amount, error) in [
        (&f.quote, 0i128, Error::Amount),
        (&f.quote, -1, Error::Amount),
        (&unrelated, 1, Error::Asset),
        (&f.quote, 4, Error::Credit),
    ] {
        f.signed(
            &f.issuer,
            &a,
            "withdraw",
            (f.issuer.clone(), asset.clone(), amount).into_val(&f.e),
            &[],
        );
        assert_eq!(
            client.try_withdraw(&f.issuer, asset, &amount),
            Err(Ok(error))
        );
        f.clear_auth();
    }
    assert_eq!(
        client.maintain(&soroban_sdk::vec![
            &f.e,
            M::Credit(f.issuer.clone(), f.quote.clone())
        ]),
        1
    );
    f.withdraw(&a, &f.issuer, &f.quote, 3);
    f.withdraw(&a, &f.issuer, &f.rwa, 7);
    f.check(&a);
}

#[test]
fn funding_rechecks_transferred_admin_and_failed_funding_can_be_retried() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, false, 10);
    let a = f.create(&config, 76);
    let next_admin = Address::generate(&f.e);
    for (old, new) in [(&f.issuer, &next_admin), (&next_admin, &f.issuer)] {
        f.signed(old, &f.rwa, "set_admin", (new.clone(),).into_val(&f.e), &[]);
        token::StellarAssetClient::new(&f.e, &f.rwa).set_admin(new);
        f.clear_auth();
        if new == &next_admin {
            f.signed(&f.issuer, &a, "fund", ().into_val(&f.e), &[]);
            assert_eq!(
                AuctionClient::new(&f.e, &a).try_fund(),
                Err(Ok(Error::Unauthorized))
            );
            f.clear_auth();
            assert_eq!(AuctionClient::new(&f.e, &a).state().stage, Stage::Created);
        }
    }
    f.fund(&a, &config);
    let client = AuctionClient::new(&f.e, &a);
    assert_eq!(client.accounting().liability_rwa, 10);
    f.signed(&f.issuer, &a, "fund", ().into_val(&f.e), &[]);
    assert_eq!(client.try_fund(), Err(Ok(Error::Phase)));
    f.clear_auth();
    assert_eq!(client.accounting().liability_rwa, 10);
    f.check(&a);
}

#[test]
fn configuration_boundaries_fail_before_any_escrow_moves() {
    let f = Fixture::new();
    let valid = f.config(Mode::Sale, false, 10);
    let mut cases = std::vec::Vec::new();
    macro_rules! invalid {
        ($field:ident, $value:expr) => {{
            let mut config = valid.clone();
            config.$field = $value;
            cases.push((config, Error::Config));
        }};
    }
    invalid!(version, 2);
    invalid!(supply_or_budget, 0);
    invalid!(min_price, 0);
    invalid!(tick_size, 0);
    invalid!(max_price, cc::SCALE - 1);
    invalid!(max_bids, 17);
    invalid!(max_bids_per_address, 17);
    invalid!(start_ledger, 90);
    invalid!(reveal_end_ledger, 115);
    invalid!(min_fill_quantity, 11);
    invalid!(min_bid_quantity, 0);
    invalid!(min_deposit, 0);
    invalid!(nonreveal_penalty_bps, 1);
    invalid!(rwa_token, f.quote.clone());
    let mut config = f.config(Mode::Sale, true, 10);
    config.nonreveal_penalty_bps = cc::MAX_NONREVEAL_PENALTY_BPS + 1;
    cases.push((config, Error::Config));
    let mut config = valid.clone();
    config.buyback_recipient = f.user();
    cases.push((config, Error::Recipient));
    let mut config = valid.clone();
    config.issuer = Address::generate(&f.e);
    config.buyback_recipient = config.issuer.clone();
    cases.push((config, Error::Unauthorized));
    let mut config = f.config(Mode::Buyback, false, i128::MAX);
    config.min_price = 1;
    config.max_price = 1;
    config.tick_size = 1;
    config.min_bid_quantity = cc::SCALE;
    cases.push((config, Error::Arithmetic));
    let mut config = valid.clone();
    config.delivery_deadline = u32::MAX;
    cases.push((config, Error::Ttl));
    let mut config = valid;
    config.delivery_deadline = f.e.storage().max_ttl() + 1000;
    cases.push((config, Error::Ttl));
    for (config, expected) in cases {
        assert_eq!(cc::validate_config(&f.e, &config), Err(expected));
    }
    let absent: Address = soroban_sdk::xdr::ScAddress::Account(soroban_sdk::xdr::AccountId(
        soroban_sdk::xdr::PublicKey::PublicKeyTypeEd25519(soroban_sdk::xdr::Uint256([0xa5; 32])),
    ))
    .try_into_val(&f.e)
    .unwrap();
    assert_eq!(cc::authorized(&f.e, &f.rwa, &absent), Err(Error::Query));
}

#[test]
fn financial_horizon_accepts_exact_maximum_ttl_and_rejects_one_beyond() {
    let f = Fixture::new();
    let now = f.e.ledger().sequence();
    let maximum = f.e.storage().max_ttl();
    let mut config = f.config(Mode::Sale, false, 10);
    config.delivery_deadline = now + maximum - cc::RECOVERY;
    assert_eq!(cc::financial_ttl(&f.e, &config), Ok(maximum));
    assert_eq!(cc::validate_config(&f.e, &config), Ok(()));
    let a = f.auction(&config, 82);
    assert_eq!(f.e.deployer().get_contract_instance_ttl(&a), maximum);
    assert_eq!(f.e.deployer().get_contract_code_ttl(&a), maximum);
    assert_eq!(now + maximum, config.delivery_deadline + cc::RECOVERY);
    config.delivery_deadline += 1;
    assert_eq!(cc::financial_ttl(&f.e, &config), Err(Error::Ttl));
    assert_eq!(cc::validate_config(&f.e, &config), Err(Error::Ttl));
}

#[test]
fn reused_auction_wasm_covers_each_new_auctions_financial_horizon() {
    let f = Fixture::new();
    // Native contracts share a synthetic executable marker. This regression
    // specifically requires a real, independently aged auction WASM entry.
    if f.native_auction {
        return;
    }
    let first = f.create(&f.config(Mode::Sale, false, 10), 83);
    assert_eq!(f.e.deployer().get_contract_code_ttl(&first), cc::RETENTION);
    f.e.ledger().set_sequence_number(218_490);
    assert_eq!(f.e.deployer().get_contract_code_ttl(&first), 300_000);
    let now = f.e.ledger().sequence();
    let mut config = f.config(Mode::Sale, false, 10);
    config.start_ledger = now + 10;
    config.end_ledger = now + 20;
    config.reveal_end_ledger = config.end_ledger;
    config.close_deadline = now + 30;
    config.delivery_deadline = now + 350_000;
    let second = f.create(&config, 84);
    let target = config.delivery_deadline + cc::RECOVERY;
    assert!(now + f.e.deployer().get_contract_instance_ttl(&second) >= target);
    assert!(now + f.e.deployer().get_contract_code_ttl(&second) >= target);
    f.fund(&second, &config);
    assert!(now + f.e.deployer().get_contract_instance_ttl(&second) >= target);
    assert!(now + f.e.deployer().get_contract_code_ttl(&second) >= target);
    assert_eq!(
        AuctionClient::new(&f.e, &second).accounting().liability_rwa,
        10
    );
}

#[test]
fn bid_caps_mode_guards_and_deposit_minimum_fail_without_taking_tokens() {
    let f = Fixture::new();
    let mut config = f.config(Mode::Sale, false, 2);
    config.max_bids = 2;
    config.max_bids_per_address = 1;
    let a = f.auction(&config, 79);
    let sealed = f.auction(&f.config(Mode::Sale, true, 2), 80);
    let mut minimum = f.config(Mode::Sale, false, 2);
    minimum.min_deposit = 2;
    let min_auction = f.auction(&minimum, 81);
    let user = f.user();
    let other = f.user();
    let third = f.user();
    f.e.ledger().set_sequence_number(100);
    let client = AuctionClient::new(&f.e, &a);
    f.place(&a, &user, 1, 1);
    for owner in [&user, &third] {
        let before = token::Client::new(&f.e, &f.quote).balance(owner);
        f.signed(
            owner,
            &a,
            "place_bid",
            (owner.clone(), cc::SCALE, 1i128).into_val(&f.e),
            &[],
        );
        assert_eq!(
            client.try_place_bid(owner, &cc::SCALE, &1),
            Err(Ok(Error::Capacity))
        );
        f.clear_auth();
        assert_eq!(token::Client::new(&f.e, &f.quote).balance(owner), before);
        if owner == &user {
            f.place(&a, &other, 1, 1);
        }
    }
    for (address, price, quantity, expected) in [
        (&min_auction, cc::SCALE, 1i128, Error::Amount),
        (&sealed, cc::SCALE, 1, Error::Phase),
        (&a, 20 * cc::SCALE, i128::MAX, Error::Arithmetic),
    ] {
        f.signed(
            &user,
            address,
            "place_bid",
            (user.clone(), price, quantity).into_val(&f.e),
            &[],
        );
        assert_eq!(
            AuctionClient::new(&f.e, address).try_place_bid(&user, &price, &quantity),
            Err(Ok(expected))
        );
        f.clear_auth();
    }
    let nonce = BytesN::from_array(&f.e, &[1; 32]);
    for (address, deposit, expected) in [(&a, 2i128, Error::Phase), (&sealed, 0, Error::Amount)] {
        f.signed(
            &user,
            address,
            "commit_bid",
            (user.clone(), nonce.clone(), nonce.clone(), deposit).into_val(&f.e),
            &[],
        );
        assert_eq!(
            AuctionClient::new(&f.e, address).try_commit_bid(&user, &nonce, &nonce, &deposit),
            Err(Ok(expected))
        );
        f.clear_auth();
    }
    assert_eq!(client.state().bid_count, 2);
    assert_eq!(AuctionClient::new(&f.e, &sealed).state().bid_count, 0);
    assert_eq!(AuctionClient::new(&f.e, &min_auction).state().bid_count, 0);
    f.check(&a);
}
