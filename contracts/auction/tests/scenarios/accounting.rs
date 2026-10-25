use super::*;

#[test]
fn sale_delivery_reserves_and_registry_are_consistent() {
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, 10);
    let a = f.auction(&c, 1);
    let client = AuctionClient::new(&f.e, &a);
    let u = f.user();
    let v = f.user();
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &u, 9, 6);
    f.place(&a, &v, 7, 8);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    let clearing = client.clearing().unwrap();
    assert_eq!(clearing.candidate.price, 7 * cc::SCALE);
    assert_eq!(clearing.filled, 10);
    assert_eq!(client.credit(&f.issuer, &f.quote), 0);
    client.claim_bid(&0);
    assert_eq!(client.credit(&f.issuer, &f.quote), 42);
    f.withdraw(&a, &f.issuer, &f.quote, 42);
    f.set_auth(&v, &f.rwa, false);
    let refund = client.claim_bid(&1);
    assert!(!refund.delivered);
    assert_eq!(refund.refund, 56);
    assert_eq!(client.credit(&f.issuer, &f.rwa), 4);
    assert_eq!(client.state().stage, Stage::Settled);
    f.check(&a);
    let _: Address = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "record_settlement"),
        (a.clone(),).into_val(&f.e),
    );
    let record: Option<Observation> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "get_clearing"),
        (a.clone(),).into_val(&f.e),
    );
    assert_eq!(record.unwrap().clearing.filled, 10);
    let execution: Option<cc::Execution> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "get_settlement"),
        (a.clone(),).into_val(&f.e),
    );
    assert_eq!(execution.unwrap().settlement.executed_rwa, 6);
    let history: Vec<Address> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "history"),
        (f.rwa.clone(), f.quote.clone(), 0u64, 32u32).into_val(&f.e),
    );
    assert_eq!(history.len(), 1);
    let _: Address = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "record_clearing"),
        (a.clone(),).into_val(&f.e),
    );
    let history2: Vec<Address> = f.e.invoke_contract(
        &f.registry,
        &Symbol::new(&f.e, "history"),
        (f.rwa.clone(), f.quote.clone(), 0u64, 32u32).into_val(&f.e),
    );
    assert_eq!(history2, history);
}

#[test]
fn genuine_sac_second_leg_failure_rolls_back_entire_buyback() {
    let f = Fixture::new();
    let c = f.config(Mode::Buyback, false, 100);
    let a = f.auction(&c, 3);
    let u = f.user();
    let client = AuctionClient::new(&f.e, &a);
    f.mint(&u, &f.quote, i128::MAX - 1_000_000); // Authorized recipient has no quote-balance capacity.
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &u, 10, 10);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    let accounting = client.accounting();
    let issuer_rwa = token::Client::new(&f.e, &f.rwa).balance(&f.issuer);
    let before = client.solvency();
    assert!(client.try_claim_bid(&0).is_err());
    assert_eq!(client.accounting(), accounting);
    assert_eq!(client.solvency(), before);
    assert_eq!(
        token::Client::new(&f.e, &f.rwa).balance(&f.issuer),
        issuer_rwa
    );
    assert!(client.get_bid(&0).resolution.is_none());
    f.clawback(&u, &f.quote, 100);
    assert!(client.claim_bid(&0).delivered);
    f.check(&a);
}

#[test]
fn frozen_credit_is_independent_and_deadline_refunds_never_forfeit() {
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, 2);
    let a = f.auction(&c, 7);
    let u = f.user();
    let v = f.user();
    let client = AuctionClient::new(&f.e, &a);
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &u, 10, 2);
    f.place(&a, &v, 2, 2);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    f.set_auth(&v, &f.quote, false);
    client.claim_bid(&1);
    assert_eq!(client.credit(&v, &f.quote), 4);
    f.signed(
        &v,
        &a,
        "withdraw",
        (v.clone(), f.quote.clone(), 4i128).into_val(&f.e),
        &[],
    );
    assert!(client.try_withdraw(&v, &f.quote, &4).is_err());
    f.clear_auth();
    assert_eq!(client.credit(&v, &f.quote), 4);
    f.e.ledger().set_sequence_number(160);
    assert!(client.try_claim_bid(&0).is_err());
    client.expire_bid(&0);
    assert_eq!(client.credit(&u, &f.quote), 20);
    assert_eq!(client.accounting().executed_rwa, 0);
    f.set_auth(&v, &f.quote, true);
    f.withdraw(&a, &v, &f.quote, 4);
    f.check(&a);
}

#[test]
fn clawback_deficit_stops_outflows_but_not_refund_accounting() {
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, 10);
    let a = f.auction(&c, 8);
    let u = f.user();
    let client = AuctionClient::new(&f.e, &a);
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &u, 5, 5);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    f.clawback(&a, &f.rwa, 1);
    assert_eq!(client.try_claim_bid(&0), Err(Ok(Error::Insolvent)));
    assert!(client.get_bid(&0).resolution.is_none());
    f.e.ledger().set_sequence_number(160);
    client.expire_bid(&0);
    assert_eq!(client.credit(&u, &f.quote), 25);
    f.mint(&a, &f.rwa, 1);
    f.withdraw(&a, &u, &f.quote, 25);
    f.check(&a);
}

#[test]
fn withdraw_solvency_check_is_global_not_per_asset() {
    // solvent() checks both assets' balance against their liability
    // regardless of which asset is being withdrawn. This locks in that
    // documented, intentional conservatism as a regression test: a future
    // change that made the check per-asset would silently change a
    // security-relevant invariant without failing any existing test.
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, 10);
    let a = f.auction(&c, 23);
    let u = f.user();
    let client = AuctionClient::new(&f.e, &a);
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &u, 5, 5);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    f.clawback(&a, &f.rwa, 1);
    assert_eq!(client.try_claim_bid(&0), Err(Ok(Error::Insolvent)));
    f.e.ledger().set_sequence_number(160);
    client.expire_bid(&0);
    // The user's quote credit is perfectly backed; only the unrelated RWA
    // leg is in deficit. Withdrawal is still blocked.
    assert_eq!(client.credit(&u, &f.quote), 25);
    f.signed(
        &u,
        &a,
        "withdraw",
        (u.clone(), f.quote.clone(), 25i128).into_val(&f.e),
        &[],
    );
    assert_eq!(
        client.try_withdraw(&u, &f.quote, &25),
        Err(Ok(Error::Insolvent))
    );
    f.clear_auth();
    assert_eq!(
        client.credit(&u, &f.quote),
        25,
        "blocked withdrawal must not consume the credit"
    );
    // Restoring the unrelated RWA leg's solvency unblocks the quote withdrawal.
    f.mint(&a, &f.rwa, 1);
    f.withdraw(&a, &u, &f.quote, 25);
    f.check(&a);
}

#[test]
fn buyback_recipient_revocation_cancels_or_refunds_without_consuming_budget() {
    for after_clearing in [false, true] {
        let f = Fixture::new();
        let recipient = f.user();
        let seller = f.user();
        let mut config = f.config(Mode::Buyback, false, 100);
        config.buyback_recipient = recipient.clone();
        let a = f.auction(&config, 75);
        let client = AuctionClient::new(&f.e, &a);
        f.e.ledger().set_sequence_number(100);
        f.place(&a, &seller, 10, 10);
        f.e.ledger().set_sequence_number(110);
        if after_clearing {
            f.close(&a);
        }
        f.set_auth(&recipient, &f.rwa, false);
        if !after_clearing {
            f.ready(&a);
            assert_eq!(
                client.finalize(),
                Finalization::Cancelled(cc::Reason::GlobalAuth)
            );
            assert_eq!(
                client.cancel_reason(),
                cc::ReasonValue::Some(cc::Reason::GlobalAuth)
            );
        }
        let outcome = client.claim_bid(&0);
        assert!(!outcome.delivered);
        assert_eq!(outcome.refund, 10);
        assert_eq!(client.accounting().executed_rwa, 0);
        assert_eq!(client.credit(&f.issuer, &f.quote), 100);
        f.withdraw(&a, &seller, &f.rwa, 10);
        f.withdraw(&a, &f.issuer, &f.quote, 100);
        f.check(&a);
    }
}
