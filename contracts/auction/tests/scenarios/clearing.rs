use super::*;

#[test]
fn buyback_gap_never_overpays_or_cuts_better_offers() {
    let f = Fixture::new();
    let c = f.config(Mode::Buyback, false, 100);
    let a = f.auction(&c, 2);
    let client = AuctionClient::new(&f.e, &a);
    let u = f.user();
    let v = f.user();
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &u, 10, 8);
    f.place(&a, &v, 20, 1);
    f.e.ledger().set_sequence_number(110);
    f.close(&a);
    let k = client.clearing().unwrap();
    assert_eq!(k.candidate.price, 10 * cc::SCALE);
    assert_eq!(k.quote, 80);
    assert_eq!(client.credit(&f.issuer, &f.quote), 20);
    f.withdraw(&a, &f.issuer, &f.quote, 20);
    assert!(client.claim_bid(&0).delivered);
    assert!(!client.claim_bid(&1).delivered);
    assert_eq!(client.accounting().executed_quote, 80);
    f.check(&a);
}

#[test]
fn revoked_zero_fill_marginal_demand_is_removed_before_price_is_fixed() {
    for mode in [Mode::Sale, Mode::Buyback] {
        for after_sweep in [false, true] {
            let f = Fixture::new();
            let amount = if mode == Mode::Sale { 3 } else { 6 };
            let config = f.config(mode, false, amount);
            let a = f.auction(&config, 92);
            let better = f.user();
            let marginal_a = f.user();
            let marginal_b = f.user();
            f.e.ledger().set_sequence_number(100);
            f.place(&a, &better, if mode == Mode::Sale { 3 } else { 1 }, 2);
            f.place(&a, &marginal_a, 2, 1);
            f.place(&a, &marginal_b, 2, 1);
            let client = AuctionClient::new(&f.e, &a);
            f.e.ledger().set_sequence_number(110);
            if after_sweep {
                f.ready(&a);
                let candidate = client.state().candidate.ok_or(Error::Phase).unwrap();
                assert_eq!(candidate.price, 2 * cc::SCALE);
                for id in [1, 2] {
                    assert_eq!(
                        cc::allocation(&f.e, &config, &client.get_bid(&id), &candidate),
                        Ok((0, 0))
                    );
                }
            }
            for owner in [&marginal_a, &marginal_b] {
                f.set_auth(owner, &f.rwa, false);
            }
            if after_sweep {
                assert!(matches!(client.finalize(), Finalization::Excluded(1, _)));
            }
            f.close(&a);
            assert!(client.get_bid(&1).excluded);
            assert!(client.get_bid(&2).excluded);
            let clearing = client.clearing().unwrap();
            assert_eq!(clearing.candidate.price, cc::SCALE);
            assert_eq!(clearing.filled, 2);
            assert_eq!(clearing.quote, 2);
            assert_eq!(clearing.winners, 1);
            assert!(client.claim_bid(&0).delivered);
            for (id, owner) in [(1, &marginal_a), (2, &marginal_b)] {
                let outcome = client.claim_bid(&id);
                let deposit = if mode == Mode::Sale { 2 } else { 1 };
                assert_eq!(outcome.refund, deposit);
                assert!(!outcome.delivered);
                assert_eq!(outcome.reason, cc::ReasonValue::Some(cc::Reason::Revoked));
                let asset = if mode == Mode::Sale { &f.quote } else { &f.rwa };
                assert_eq!(client.credit(owner, asset), deposit);
            }
            assert_eq!(client.accounting().executed_rwa, 2);
            assert_eq!(client.accounting().executed_quote, 2);
            f.check(&a);
        }
    }
}

#[test]
fn revoked_zero_payment_bid_cannot_force_an_otherwise_deliverable_book_to_cancel() {
    let f = Fixture::new();
    let mut config = f.config(Mode::Buyback, false, 1);
    config.min_price = 1;
    config.max_price = 1;
    config.tick_size = 1;
    config.min_bid_quantity = cc::SCALE;
    let a = f.auction(&config, 93);
    let eligible = f.user();
    let revoked = f.user();
    for owner in [&eligible, &revoked] {
        f.mint(owner, &f.rwa, cc::SCALE);
    }
    f.e.ledger().set_sequence_number(100);
    f.place_raw(&a, &eligible, 1, cc::SCALE);
    f.place_raw(&a, &revoked, 1, cc::SCALE);
    f.e.ledger().set_sequence_number(110);
    f.ready(&a);
    let client = AuctionClient::new(&f.e, &a);
    let candidate = client.state().candidate.ok_or(Error::Phase).unwrap();
    assert_eq!(candidate.numerator, cc::SCALE);
    assert_eq!(candidate.denominator, 2 * cc::SCALE);
    // Each nominal half-token allocation has a zero quote payment and would
    // be filtered out; removing revoked demand permits one full, paid token.
    assert_eq!(
        cc::allocation(&f.e, &config, &client.get_bid(&1), &candidate),
        Ok((0, 0))
    );
    f.set_auth(&revoked, &f.rwa, false);
    assert!(matches!(client.finalize(), Finalization::Excluded(1, _)));
    f.close(&a);
    assert_eq!(client.clearing().unwrap().filled, cc::SCALE);
    assert_eq!(client.clearing().unwrap().quote, 1);
    assert!(client.claim_bid(&0).delivered);
    assert_eq!(client.claim_bid(&1).refund, cc::SCALE);
    assert_eq!(client.credit(&revoked, &f.rwa), cc::SCALE);
    f.check(&a);
}

#[test]
fn wide_amounts_execute_fractional_price_prorata_and_drain_both_liabilities() {
    // Independent integer arithmetic for q=(2^127-1)/8, p=30_000_001, S=10^7:
    // sale target q, buyback budget floor(q*p/S), buyback target q-1;
    // both produce floor(q/2) per bid. Products exceed the i128 range.
    const QUANTITY: i128 = 21_267_647_932_558_653_966_460_912_964_485_513_215;
    const BUDGET: i128 = 63_802_945_924_440_755_155_248_135_539_547_836_093;
    const FILL: i128 = 10_633_823_966_279_326_983_230_456_482_242_756_607;
    const FLOOR_PAYMENT: i128 = 31_901_472_962_220_377_577_624_067_769_773_918_045;
    for mode in [Mode::Sale, Mode::Buyback] {
        let f = Fixture::new();
        let funding = if mode == Mode::Sale { QUANTITY } else { BUDGET };
        let funding_asset = if mode == Mode::Sale { &f.rwa } else { &f.quote };
        f.mint(&f.issuer, funding_asset, funding);
        let mut config = f.config(mode, false, funding);
        config.min_price = 30_000_001;
        config.max_price = 30_000_001;
        config.tick_size = 1;
        let a = f.auction(&config, 94);
        let owners = [f.user(), f.user()];
        let deposit = if mode == Mode::Sale {
            BUDGET + 1
        } else {
            QUANTITY
        };
        let deposit_asset = if mode == Mode::Sale { &f.quote } else { &f.rwa };
        for owner in &owners {
            f.mint(owner, deposit_asset, deposit);
        }
        f.e.ledger().set_sequence_number(100);
        for owner in &owners {
            f.place_raw(&a, owner, config.min_price, QUANTITY);
        }
        f.e.ledger().set_sequence_number(110);
        f.close(&a);
        let client = AuctionClient::new(&f.e, &a);
        let payment = FLOOR_PAYMENT + if mode == Mode::Sale { 1 } else { 0 };
        let clearing = client.clearing().unwrap();
        assert_eq!(clearing.candidate.price, 30_000_001);
        assert_eq!(clearing.filled, 2 * FILL);
        assert_eq!(clearing.quote, 2 * payment);
        for id in [1, 0] {
            let outcome = client.claim_bid(&id);
            assert_eq!(outcome.rwa, FILL);
            assert_eq!(outcome.quote, payment);
            assert_eq!(
                outcome.refund,
                deposit - if mode == Mode::Sale { payment } else { FILL }
            );
        }
        for owner in [&f.issuer, &owners[0], &owners[1]] {
            for asset in [&f.rwa, &f.quote] {
                let credit = client.credit(owner, asset);
                if credit > 0 {
                    f.withdraw(&a, owner, asset, credit);
                }
            }
        }
        assert_eq!(client.accounting().liability_rwa, 0);
        assert_eq!(client.accounting().liability_quote, 0);
        f.check(&a);
    }
}
