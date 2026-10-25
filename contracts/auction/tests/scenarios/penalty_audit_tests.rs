use super::*;

#[test]
fn sealed_sale_reserves_reveal_capacity_before_accepting_collateral() {
    for reverse in [false, true] {
        let f = Fixture::new();
        let mut c = f.config(Mode::Sale, true, 1);
        c.min_price = 1;
        c.max_price = 1;
        c.tick_size = 1;
        c.min_bid_quantity = cc::SCALE;
        c.nonreveal_penalty_bps = 2_500;
        let a = f.auction(&c, 98);
        let client = AuctionClient::new(&f.e, &a);
        let first = f.user();
        let second = f.user();
        f.e.ledger().set_sequence_number(100);
        let first_reveal = f.commit(&a, &first, 1, 100 * cc::SCALE, 100);
        let nonce = BytesN::from_array(&f.e, &[1; 32]);
        let hash = BytesN::from_array(&f.e, &[99; 32]);
        let excessive = i128::MAX / cc::SCALE;
        // Sufficient collateral and exact transfer authorization ensure the old
        // implementation accepts this unsafe commitment.
        f.mint(&second, &f.quote, excessive);
        let before = client.accounting();
        let state = client.state();
        let balance = token::Client::new(&f.e, &f.quote).balance(&second);
        let escrow = client.solvency();
        f.signed(
            &second,
            &a,
            "commit_bid",
            (second.clone(), nonce.clone(), hash.clone(), excessive).into_val(&f.e),
            &[MockAuthInvoke {
                contract: &f.quote,
                fn_name: "transfer",
                args: (second.clone(), a.clone(), excessive).into_val(&f.e),
                sub_invokes: &[],
            }],
        );
        assert_eq!(
            client.try_commit_bid(&second, &nonce, &hash, &excessive),
            Err(Ok(Error::Arithmetic))
        );
        f.clear_auth();
        assert_eq!(client.accounting(), before);
        assert_eq!(client.state(), state);
        assert_eq!(client.solvency(), escrow);
        assert_eq!(token::Client::new(&f.e, &f.quote).balance(&second), balance);
        assert!(client.bid_ids(&second, &0, &32).is_empty());
        // Fixture uses the next bid ID as nonce: this retries the same nonce.
        let accepted = excessive - 100;
        let second_reveal = f.commit(&a, &second, 1, accepted * cc::SCALE, accepted);
        assert_eq!(second_reveal.0, 1);
        assert_eq!(client.get_bid(&1).nonce, Some(nonce));
        f.e.ledger().set_sequence_number(110);
        let reveals = if reverse {
            [
                (1, accepted * cc::SCALE, second_reveal.1),
                (0, 100 * cc::SCALE, first_reveal.1),
            ]
        } else {
            [
                (0, 100 * cc::SCALE, first_reveal.1),
                (1, accepted * cc::SCALE, second_reveal.1),
            ]
        };
        for (id, quantity, salt) in reveals {
            f.reveal(&a, id, 1, quantity, &salt);
        }
        assert_eq!(client.accounting().admitted_quantity, excessive * cc::SCALE);
        f.check(&a);
    }
}

fn drain_penalty_credits(f: &Fixture, auction: &Address, owners: &[&Address]) {
    let client = AuctionClient::new(&f.e, auction);
    for owner in owners {
        for asset in [&f.rwa, &f.quote] {
            let amount = client.credit(owner, asset);
            if amount > 0 {
                f.withdraw(auction, owner, asset, amount);
            }
        }
    }
    assert_eq!(client.accounting().liability_rwa, 0);
    assert_eq!(client.accounting().liability_quote, 0);
    assert_eq!(client.solvency(), (0, 0, 0, 0));
    f.check(auction);
}

#[test]
fn nonreveal_penalty_reason_precedence_matches_terminal_refund_policy() {
    for mode in [Mode::Sale, Mode::Buyback] {
        for terminal in ["cleared", "cancelled", "expired"] {
            for revoked in [false, true] {
                let f = Fixture::new();
                let mut c = f.config(mode, true, 10);
                c.max_price = cc::SCALE;
                c.nonreveal_penalty_bps = 2_500;
                if terminal == "cancelled" {
                    c.min_fill_quantity = 6;
                }
                let a = f.auction(&c, 99);
                let client = AuctionClient::new(&f.e, &a);
                let good = f.user();
                let absent = f.user();
                let asset = if mode == Mode::Sale { &f.quote } else { &f.rwa };
                f.e.ledger().set_sequence_number(100);
                let (good_id, salt) = f.commit(&a, &good, cc::SCALE, 5, 5);
                let (absent_id, _) = f.commit(&a, &absent, cc::SCALE, 100, 100);
                f.e.ledger().set_sequence_number(110);
                f.reveal(&a, good_id, cc::SCALE, 5, &salt);
                f.e.ledger().set_sequence_number(115);
                f.close(&a);
                if revoked {
                    f.set_auth(&absent, asset, false);
                }
                let outcome = if terminal == "expired" {
                    f.e.ledger().set_sequence_number(c.delivery_deadline);
                    let outcome = client.expire_bid(&absent_id);
                    client.expire_bid(&good_id);
                    outcome
                } else {
                    let outcome = client.claim_bid(&absent_id);
                    client.claim_bid(&good_id);
                    outcome
                };
                // NonReveal is classified ahead of the timing-derived
                // `expired` label (resolve() in auction/src/lib.rs), so a
                // never-revealed bid pays the same penalty whether it is
                // resolved via claim_bid before the delivery deadline or via
                // expire_bid after it -- waiting out the deadline must not
                // be a way to dodge the penalty. Only a genuinely cancelled
                // auction (which never cleared, so no price was distorted)
                // skips it.
                let fee = if terminal == "cancelled" { 0 } else { 25 };
                let reason = match terminal {
                    "cancelled" => cc::Reason::Minimum,
                    _ => cc::Reason::NonReveal,
                };
                assert_eq!(outcome.reason, cc::ReasonValue::Some(reason));
                assert_eq!(outcome.refund, 100 - fee);
                assert_eq!(client.credit(&absent, asset), 100 - fee);
                let proceeds = if mode == Mode::Sale && terminal == "cleared" {
                    5
                } else {
                    0
                };
                assert_eq!(client.credit(&f.issuer, asset), fee + proceeds);
                assert_eq!(
                    client.accounting().executed_rwa,
                    if terminal == "cleared" { 5 } else { 0 }
                );
                if revoked {
                    f.set_auth(&absent, asset, true);
                }
                drain_penalty_credits(&f, &a, &[&absent, &good, &f.issuer]);
            }
        }
    }
}

#[test]
fn penalty_rounding_wide_amounts_and_failed_withdrawals_preserve_all_ownership() {
    for mode in [Mode::Sale, Mode::Buyback] {
        for (deposit, rate) in [
            (1, 5_000u32),
            (999, 2_500),
            (1_000, 5_000),
            (i128::MAX - 1, 5_000),
        ] {
            let f = Fixture::new();
            let mut c = f.config(mode, true, 1);
            c.max_price = cc::SCALE;
            c.nonreveal_penalty_bps = rate;
            let a = f.auction(&c, 100);
            let client = AuctionClient::new(&f.e, &a);
            let good = f.user();
            let absent = f.user();
            let asset = if mode == Mode::Sale { &f.quote } else { &f.rwa };
            if deposit > 1_000_000 {
                f.mint(&absent, asset, deposit - 1_000_000);
            }
            f.e.ledger().set_sequence_number(100);
            let (good_id, salt) = f.commit(&a, &good, cc::SCALE, 1, 1);
            let (absent_id, _) = f.commit(&a, &absent, cc::SCALE, deposit, deposit);
            f.e.ledger().set_sequence_number(110);
            f.reveal(&a, good_id, cc::SCALE, 1, &salt);
            f.e.ledger().set_sequence_number(115);
            f.close(&a);
            let outcome = client.claim_bid(&absent_id);
            // Divide first because every chosen rate divides 10_000; this
            // independent expression also handles deposits near i128::MAX.
            let fee = deposit / (10_000 / rate as i128);
            assert_eq!(outcome.refund, deposit - fee);
            assert_eq!(client.credit(&f.issuer, asset), fee);
            assert_eq!(client.credit(&absent, asset) + fee, deposit);
            let credit = outcome.refund;
            f.set_auth(&absent, asset, false);
            let before = client.accounting();
            let balances = client.solvency();
            f.signed(
                &absent,
                &a,
                "withdraw",
                (absent.clone(), asset.clone(), credit).into_val(&f.e),
                &[],
            );
            assert_eq!(
                client.try_withdraw(&absent, asset, &credit),
                Err(Ok(Error::Transfer))
            );
            f.clear_auth();
            assert_eq!(client.accounting(), before);
            assert_eq!(client.solvency(), balances);
            assert_eq!(client.credit(&absent, asset), credit);
            f.set_auth(&absent, asset, true);
            // Interleave refunds and penalty withdrawal before delivery, then
            // drain all remaining credits after successful delivery.
            f.withdraw(&a, &absent, asset, credit);
            if fee > 0 {
                f.withdraw(&a, &f.issuer, asset, fee);
            }
            assert!(client.claim_bid(&good_id).delivered);
            assert_eq!(client.settlement().unwrap().executed_rwa, 1);
            drain_penalty_credits(&f, &a, &[&absent, &good, &f.issuer]);
        }
    }
}
