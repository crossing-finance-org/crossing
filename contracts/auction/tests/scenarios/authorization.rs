use super::*;

#[test]
fn signatures_are_enforced_and_failed_calls_do_not_leave_guard_locked() {
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, 10);
    let a = f.auction(&c, 6);
    let u = f.user();
    let client = AuctionClient::new(&f.e, &a);
    f.e.ledger().set_sequence_number(100);
    assert!(client.try_place_bid(&u, &cc::SCALE, &1).is_err());
    assert_eq!(client.state().bid_count, 0);
    f.place(&a, &u, 1, 1);
    assert!(client.try_place_bid(&u, &cc::SCALE, &1).is_err());
    assert_eq!(client.state().bid_count, 1);
    f.set_auth(&u, &f.rwa, false);
    f.signed(
        &u,
        &a,
        "place_bid",
        (u.clone(), cc::SCALE, 1i128).into_val(&f.e),
        &[],
    );
    assert_eq!(
        client.try_place_bid(&u, &cc::SCALE, &1),
        Err(Ok(Error::Unauthorized))
    );
    f.clear_auth();
    assert_eq!(client.state().bid_count, 1);
}

#[test]
fn fake_tokens_clones_and_fake_registry_publications_are_rejected() {
    let f = Fixture::new();
    let mut c = f.config(Mode::Sale, false, 10);
    let fake = f.e.register(Fake, ());
    c.quote_token = fake.clone();
    let salt = BytesN::from_array(&f.e, &[11; 32]);
    f.signed(
        &f.issuer,
        &f.factory,
        "create",
        (c.clone(), salt.clone()).into_val(&f.e),
        &[],
    );
    let got = f.e.try_invoke_contract::<Address, Error>(
        &f.factory,
        &Symbol::new(&f.e, "create"),
        (c, salt).into_val(&f.e),
    );
    assert_eq!(got, Err(Ok(Error::Asset)));
    f.clear_auth();
    let bad = f.e.try_invoke_contract::<Address, Error>(
        &f.registry,
        &Symbol::new(&f.e, "record_clearing"),
        (fake,).into_val(&f.e),
    );
    assert_eq!(bad, Err(Ok(Error::Provenance)));
    let c = f.config(Mode::Sale, false, 10);
    let args = (c, f.factory.clone(), f.registry.clone(), f.template.clone());
    let clone = if f.native_auction {
        f.e.register(crossing_auction::Auction, args)
    } else {
        f.e.register(wasm("crossing_auction"), args)
    };
    let client = AuctionClient::new(&f.e, &clone);
    f.signed(&f.issuer, &clone, "fund", ().into_val(&f.e), &[]);
    assert_eq!(client.try_fund(), Err(Ok(Error::Provenance)));
    f.clear_auth();
}

#[test]
fn auth_required_escrow_must_be_authorized_before_funding() {
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, 10);
    let a = f.create(&c, 13);
    let client = AuctionClient::new(&f.e, &a);
    f.signed(&f.issuer, &a, "fund", ().into_val(&f.e), &[]);
    assert_eq!(client.try_fund(), Err(Ok(Error::Unauthorized)));
    f.clear_auth();
    assert_eq!(client.state().stage, Stage::Created);
    assert_eq!(client.accounting().liability_rwa, 0);
    f.fund(&a, &c);
    assert_eq!(client.state().stage, Stage::Funded);
}

#[test]
fn native_xlm_is_rejected_on_both_sides_of_the_pair() {
    let f = Fixture::new();
    let asset_xdr = soroban_sdk::xdr::WriteXdr::to_xdr(
        &soroban_sdk::xdr::Asset::Native,
        soroban_sdk::xdr::Limits::none(),
    )
    .unwrap();
    let native =
        f.e.deployer()
            .with_stellar_asset(soroban_sdk::Bytes::from_slice(&f.e, &asset_xdr))
            .deploy();
    for rwa_side in [true, false] {
        let mut config = f.config(Mode::Sale, false, 10);
        if rwa_side {
            config.rwa_token = native.clone();
        } else {
            config.quote_token = native.clone();
        }
        let salt = BytesN::from_array(&f.e, &[50; 32]);
        f.signed(
            &f.issuer,
            &f.factory,
            "create",
            (config.clone(), salt.clone()).into_val(&f.e),
            &[],
        );
        let result = f.e.try_invoke_contract::<Address, Error>(
            &f.factory,
            &Symbol::new(&f.e, "create"),
            (config, salt).into_val(&f.e),
        );
        assert_eq!(result, Err(Ok(Error::Asset)), "native RWA side: {rwa_side}");
        f.clear_auth();
        let count: u64 =
            f.e.invoke_contract(&f.factory, &Symbol::new(&f.e, "count"), ().into_val(&f.e));
        assert_eq!(count, 0);
    }
}
