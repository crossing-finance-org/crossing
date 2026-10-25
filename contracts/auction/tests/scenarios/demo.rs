use super::*;

#[test]
fn poc_demo_open_sale_publishes_one_price_and_delivers_assets() {
    let f = Fixture::new();
    let unit = cc::SCALE;
    f.mint(&f.issuer, &f.rwa, 100 * unit);
    let a = f.auction(&f.config(Mode::Sale, false, 10 * unit), 20);
    let client = AuctionClient::new(&f.e, &a);
    let alice = f.user();
    let bob = f.user();
    for user in [&alice, &bob] {
        f.mint(user, &f.quote, 100 * unit);
    }
    let rwa = token::Client::new(&f.e, &f.rwa);
    let quote = token::Client::new(&f.e, &f.quote);
    let before_alice = (rwa.balance(&alice), quote.balance(&alice));
    let before_bob = (rwa.balance(&bob), quote.balance(&bob));
    let before_issuer = quote.balance(&f.issuer);
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &alice, 9, 6 * unit);
    f.place(&a, &bob, 7, 8 * unit);
    assert_eq!(client.tick_total(&8), 6 * unit);
    assert_eq!(client.tick_total(&6), 8 * unit);
    assert_eq!(client.accounting().pending_quote, 110 * unit);
    println!(
        "SALE: issuer escrows 10 RWA; Alice locks 54 quote for 6 @ 9; Bob locks 56 for 8 @ 7."
    );

    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    let clearing = client.clearing().unwrap();
    assert_eq!(clearing.candidate.price, 7 * unit);
    assert_eq!((clearing.filled, clearing.quote), (10 * unit, 70 * unit));
    assert_eq!(client.credit(&f.issuer, &f.quote), 0);
    let alice_result = client.claim_bid(&0);
    let bob_result = client.claim_bid(&1);
    assert!(alice_result.delivered && bob_result.delivered);
    assert_eq!(
        (alice_result.rwa, alice_result.quote),
        (6 * unit, 42 * unit)
    );
    assert_eq!((bob_result.rwa, bob_result.quote), (4 * unit, 28 * unit));
    f.withdraw(&a, &alice, &f.quote, 12 * unit);
    f.withdraw(&a, &bob, &f.quote, 28 * unit);
    f.withdraw(&a, &f.issuer, &f.quote, 70 * unit);
    assert_eq!(rwa.balance(&alice) - before_alice.0, 6 * unit);
    assert_eq!(rwa.balance(&bob) - before_bob.0, 4 * unit);
    assert_eq!(before_alice.1 - quote.balance(&alice), 42 * unit);
    assert_eq!(before_bob.1 - quote.balance(&bob), 28 * unit);
    assert_eq!(quote.balance(&f.issuer) - before_issuer, 70 * unit);
    assert_eq!(client.accounting().liability_quote, 0);
    f.check(&a);
    let _: Address = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "record_settlement"),
        (a.clone(),).into_val(&f.e),
    );
    let record: Option<cc::Execution> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "latest_settled"),
        (f.rwa.clone(), f.quote.clone()).into_val(&f.e),
    );
    let record = record.unwrap();
    assert_eq!(record.auction, a);
    assert_eq!(record.clearing_ledger, clearing.ledger);
    let published: Option<Observation> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "latest"),
        (f.rwa.clone(), f.quote.clone()).into_val(&f.e),
    );
    assert_eq!(published.unwrap().clearing.candidate.price, 7 * unit);
    assert_eq!(record.settlement.executed_rwa, 10 * unit);
    assert_eq!(record.settlement.executed_quote, 70 * unit);
    println!("SALE RESULT: uniform price 7; Alice receives 6 RWA and refund 12; Bob receives 4 and refund 28; issuer receives 70 quote.");
    println!(
        "REGISTRY: verified public clearing price 7, allocated 10 RWA, executed 10 RWA / 70 quote."
    );
}

#[test]
fn poc_demo_sealed_buyback_checks_reveals_and_refunds_nonreveals() {
    let f = Fixture::new();
    let unit = cc::SCALE;
    f.mint(&f.issuer, &f.quote, 100 * unit);
    let a = f.auction(&f.config(Mode::Buyback, true, 100 * unit), 21);
    let client = AuctionClient::new(&f.e, &a);
    let alice = f.user();
    let bob = f.user();
    let carol = f.user();
    for user in [&alice, &bob, &carol] {
        f.mint(user, &f.rwa, 10 * unit);
    }
    let rwa = token::Client::new(&f.e, &f.rwa);
    let quote = token::Client::new(&f.e, &f.quote);
    let issuer_before = rwa.balance(&f.issuer);
    let alice_before = quote.balance(&alice);
    let bob_before = quote.balance(&bob);
    let carol_before = rwa.balance(&carol);
    f.e.ledger().set_sequence_number(100);
    let (alice_id, alice_salt) = f.commit(&a, &alice, 8 * unit, 4 * unit, 4 * unit);
    let (bob_id, bob_salt) = f.commit(&a, &bob, 10 * unit, 6 * unit, 6 * unit);
    let (carol_id, _) = f.commit(&a, &carol, 11 * unit, 3 * unit, 3 * unit);
    assert!(client.get_bid(&alice_id).price.is_none());
    assert!(client.get_bid(&bob_id).quantity.is_none());
    assert_eq!(client.tick_total(&7), 0);
    println!("SEALED BUYBACK: issuer escrows 100 quote; three commitments lock 4, 6, and 3 RWA; prices and quantities are absent from bid records.");

    f.e.ledger().set_sequence_number(110);
    f.signed(
        &alice,
        &a,
        "reveal_bid",
        (alice_id, 9 * unit, 4 * unit, alice_salt.clone()).into_val(&f.e),
        &[],
    );
    assert_eq!(
        client.try_reveal_bid(&alice_id, &(9 * unit), &(4 * unit), &alice_salt),
        Err(Ok(Error::Commitment))
    );
    f.clear_auth();
    f.reveal(&a, alice_id, 8 * unit, 4 * unit, &alice_salt);
    f.reveal(&a, bob_id, 10 * unit, 6 * unit, &bob_salt);
    f.e.ledger().set_sequence_number(115);
    f.close(&a);
    let clearing = client.clearing().unwrap();
    assert_eq!(clearing.candidate.price, 10 * unit);
    assert_eq!((clearing.filled, clearing.quote), (10 * unit, 100 * unit));
    assert!(client.claim_bid(&alice_id).delivered);
    assert!(client.claim_bid(&bob_id).delivered);
    let nonreveal = client.claim_bid(&carol_id);
    assert!(!nonreveal.delivered);
    assert_eq!(nonreveal.refund, 3 * unit);
    f.withdraw(&a, &carol, &f.rwa, 3 * unit);
    assert_eq!(rwa.balance(&carol), carol_before);
    assert_eq!(rwa.balance(&f.issuer) - issuer_before, 10 * unit);
    assert_eq!(quote.balance(&alice) - alice_before, 40 * unit);
    assert_eq!(quote.balance(&bob) - bob_before, 60 * unit);
    assert_eq!(client.accounting().liability_rwa, 0);
    assert_eq!(client.accounting().liability_quote, 0);
    assert_eq!(client.state().stage, Stage::Settled);
    f.check(&a);
    println!("SEALED RESULT: altered reveal rejected; valid asks 4 @ 8 and 6 @ 10 clear at 10; sellers receive 40 and 60 quote; issuer receives 10 RWA.");
    println!("NONREVEAL: Carol is excluded from clearing and withdraws all 3 RWA; both escrow liabilities finish at zero.");
}

#[test]
fn poc_demo_revoked_winner_is_excluded_before_clearing() {
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, 10);
    let a = f.auction(&c, 4);
    let u = f.user();
    let v = f.user();
    let client = AuctionClient::new(&f.e, &a);
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &u, 9, 6);
    f.place(&a, &v, 7, 8);
    f.e.ledger().set_sequence_number(110);
    f.ready(&a);
    f.set_auth(&u, &f.rwa, false);
    assert!(matches!(client.finalize(), Finalization::Excluded(0, _)));
    assert!(client.get_bid(&0).excluded);
    println!("COMPLIANCE: a bidder revoked after the preliminary sweep is atomically excluded; clearing restarts without its demand.");
    f.set_auth(&u, &f.rwa, true);
    f.close(&a);
    assert_eq!(client.clearing().unwrap().filled, 8);
    assert_eq!(client.clearing().unwrap().candidate.price, cc::SCALE);
    assert_eq!(client.claim_bid(&0).refund, 54);
    assert!(client.claim_bid(&1).delivered);
    f.withdraw(&a, &u, &f.quote, 54);
    f.check(&a);
    println!("COMPLIANCE RESULT: excluded bid stays excluded after reauthorization and recovers its full deposit; remaining demand determines the price.");
}
