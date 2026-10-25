use super::*;

#[test]
fn sealed_nonreveal_and_excess_collateral_remain_owned() {
    let f = Fixture::new();
    let c = f.config(Mode::Sale, true, 3);
    let a = f.auction(&c, 5);
    let u = f.user();
    let v = f.user();
    let client = AuctionClient::new(&f.e, &a);
    let nonce = BytesN::from_array(&f.e, &[9; 32]);
    let salt = BytesN::from_array(&f.e, &[4; 32]);
    let p = client.provenance();
    let preimage = (
        Symbol::new(&f.e, "CROSSING_BID_V1"),
        f.e.ledger().network_id(),
        a.clone(),
        u.clone(),
        nonce.clone(),
        p.config_hash,
        100i128,
        5 * cc::SCALE,
        3i128,
        salt.clone(),
    )
        .to_xdr(&f.e);
    let hash: BytesN<32> = f.e.crypto().sha256(&preimage).into();
    f.e.ledger().set_sequence_number(100);
    f.signed(
        &u,
        &a,
        "commit_bid",
        (u.clone(), nonce.clone(), hash.clone(), 100i128).into_val(&f.e),
        &[MockAuthInvoke {
            contract: &f.quote,
            fn_name: "transfer",
            args: (u.clone(), a.clone(), 100i128).into_val(&f.e),
            sub_invokes: &[],
        }],
    );
    assert_eq!(client.commit_bid(&u, &nonce, &hash, &100), 0);
    f.clear_auth();
    let other_nonce = BytesN::from_array(&f.e, &[8; 32]);
    f.signed(
        &v,
        &a,
        "commit_bid",
        (v.clone(), other_nonce.clone(), hash.clone(), 200i128).into_val(&f.e),
        &[MockAuthInvoke {
            contract: &f.quote,
            fn_name: "transfer",
            args: (v.clone(), a.clone(), 200i128).into_val(&f.e),
            sub_invokes: &[],
        }],
    );
    client.commit_bid(&v, &other_nonce, &hash, &200);
    f.clear_auth();
    f.e.ledger().set_sequence_number(110);
    f.signed(
        &u,
        &a,
        "reveal_bid",
        (0u32, 5 * cc::SCALE, 3i128, salt.clone()).into_val(&f.e),
        &[],
    );
    client.reveal_bid(&0, &(5 * cc::SCALE), &3, &salt);
    f.clear_auth();
    assert_eq!(client.accounting().pending_quote, 300);
    f.e.ledger().set_sequence_number(115);
    f.close(&a);
    client.claim_bid(&0);
    client.claim_bid(&1);
    assert_eq!(client.credit(&u, &f.quote), 85);
    assert_eq!(client.credit(&v, &f.quote), 200);
    f.check(&a);
}

#[test]
fn sealed_nonreveal_penalty_credits_issuer_and_caps_bidder_loss() {
    let f = Fixture::new();
    let mut c = f.config(Mode::Sale, true, 3);
    c.nonreveal_penalty_bps = 2_500;
    let a = f.auction(&c, 5);
    let u = f.user();
    let v = f.user();
    let client = AuctionClient::new(&f.e, &a);
    let nonce = BytesN::from_array(&f.e, &[9; 32]);
    let salt = BytesN::from_array(&f.e, &[4; 32]);
    let p = client.provenance();
    let preimage = (
        Symbol::new(&f.e, "CROSSING_BID_V1"),
        f.e.ledger().network_id(),
        a.clone(),
        u.clone(),
        nonce.clone(),
        p.config_hash,
        100i128,
        5 * cc::SCALE,
        3i128,
        salt.clone(),
    )
        .to_xdr(&f.e);
    let hash: BytesN<32> = f.e.crypto().sha256(&preimage).into();
    f.e.ledger().set_sequence_number(100);
    f.signed(
        &u,
        &a,
        "commit_bid",
        (u.clone(), nonce.clone(), hash.clone(), 100i128).into_val(&f.e),
        &[MockAuthInvoke {
            contract: &f.quote,
            fn_name: "transfer",
            args: (u.clone(), a.clone(), 100i128).into_val(&f.e),
            sub_invokes: &[],
        }],
    );
    assert_eq!(client.commit_bid(&u, &nonce, &hash, &100), 0);
    f.clear_auth();
    let other_nonce = BytesN::from_array(&f.e, &[8; 32]);
    f.signed(
        &v,
        &a,
        "commit_bid",
        (v.clone(), other_nonce.clone(), hash.clone(), 200i128).into_val(&f.e),
        &[MockAuthInvoke {
            contract: &f.quote,
            fn_name: "transfer",
            args: (v.clone(), a.clone(), 200i128).into_val(&f.e),
            sub_invokes: &[],
        }],
    );
    client.commit_bid(&v, &other_nonce, &hash, &200);
    f.clear_auth();
    f.e.ledger().set_sequence_number(110);
    f.signed(
        &u,
        &a,
        "reveal_bid",
        (0u32, 5 * cc::SCALE, 3i128, salt.clone()).into_val(&f.e),
        &[],
    );
    client.reveal_bid(&0, &(5 * cc::SCALE), &3, &salt);
    f.clear_auth();
    f.e.ledger().set_sequence_number(115);
    f.close(&a);
    client.claim_bid(&0);
    client.claim_bid(&1);
    // v never reveals: its 200-unit deposit is refunded minus a floored 25%
    // penalty (50), and that penalty is credited to the issuer alongside
    // u's delivered payment (15) -- never confiscated, never lost.
    assert_eq!(client.credit(&u, &f.quote), 85);
    assert_eq!(client.credit(&v, &f.quote), 150);
    assert_eq!(client.credit(&f.issuer, &f.quote), 15 + 50);
    f.check(&a);
}

#[test]
fn full_sealed_book_with_mixed_reveals_and_penalty_clears_across_batched_sweep() {
    // A full MAX_BIDS (16) sealed book, 10 of which reveal across three price
    // tiers and 6 of which never reveal, with a nonzero non-reveal penalty.
    // BID_BATCH is 8, so closing this book cannot finish sweep in one
    // advance() call -- it must span at least two batches. This exercises
    // the combination no existing test covers: a full-capacity sealed book,
    // a real mix of revealed/non-revealed bids, and the penalty feature,
    // swept across multiple advance() calls rather than a single pass.
    let f = Fixture::new();
    let mut c = f.config(Mode::Sale, true, 50);
    c.nonreveal_penalty_bps = 1_000; // 10%
    c.max_bids = cc::MAX_BIDS;
    c.max_bids_per_address = cc::MAX_BIDS;
    let a = f.auction(&c, 17);
    let client = AuctionClient::new(&f.e, &a);
    f.e.ledger().set_sequence_number(100);

    // Three revealed price tiers: 3@5 (qty 5), 3@4 (qty 5), 4@3 (qty 10).
    // Cumulative demand at/above price 3 is 15+15+40 = 70 against a supply
    // of 50, so price 3 is the marginal tick with a 20/40 pro-rata split.
    let mut revealed: std::vec::Vec<(Address, u32, i128, i128, BytesN<32>)> = std::vec::Vec::new();
    for (price, qty, count) in [
        (5 * cc::SCALE, 5i128, 3),
        (4 * cc::SCALE, 5i128, 3),
        (3 * cc::SCALE, 10i128, 4),
    ] {
        for _ in 0..count {
            let user = f.user();
            let locked = cc::cost(&f.e, Mode::Sale, qty, price).unwrap();
            let (id, salt) = f.commit(&a, &user, price, qty, locked);
            revealed.push((user, id, price, qty, salt));
        }
    }
    assert_eq!(revealed.len(), 10);

    // Six bidders commit and lock real collateral but never reveal.
    let mut nonrevealers: std::vec::Vec<(Address, u32)> = std::vec::Vec::new();
    for _ in 0..6 {
        let user = f.user();
        let locked = cc::cost(&f.e, Mode::Sale, 5, 2 * cc::SCALE).unwrap();
        let (id, _salt) = f.commit(&a, &user, 2 * cc::SCALE, 5, locked);
        nonrevealers.push((user, id));
    }
    assert_eq!(client.state().bid_count, 16);

    f.e.ledger().set_sequence_number(110);
    for (_user, id, price, qty, salt) in &revealed {
        f.reveal(&a, *id, *price, *qty, salt);
    }
    f.e.ledger().set_sequence_number(115);
    f.close(&a);

    let cleared = client.clearing().unwrap();
    assert_eq!(cleared.candidate.price, 3 * cc::SCALE);
    assert_eq!(cleared.filled, 50);
    assert_eq!(cleared.winners, 10);

    let rwa_token = token::Client::new(&f.e, &f.rwa);
    for (user, id, price, qty, _salt) in &revealed {
        let before = rwa_token.balance(user);
        let outcome = client.claim_bid(id);
        assert!(outcome.delivered);
        let expected_fill = if *price > 3 * cc::SCALE {
            *qty
        } else {
            *qty / 2
        };
        assert_eq!(outcome.rwa, expected_fill);
        // Sale-mode delivery is a direct transfer, not a withdrawable credit
        // (unlike refunds/proceeds, which use the credit+withdraw path).
        assert_eq!(rwa_token.balance(user) - before, expected_fill);
    }
    let penalty_bps = c.nonreveal_penalty_bps as i128;
    let mut total_penalty = 0i128;
    for (user, id) in &nonrevealers {
        let locked = cc::cost(&f.e, Mode::Sale, 5, 2 * cc::SCALE).unwrap();
        let outcome = client.claim_bid(id);
        assert!(!outcome.delivered);
        let penalty = locked * penalty_bps / 10_000;
        total_penalty += penalty;
        assert_eq!(outcome.refund, locked - penalty);
        assert_eq!(client.credit(user, &f.quote), locked - penalty);
    }
    assert_eq!(
        client.credit(&f.issuer, &f.quote),
        cleared.quote + total_penalty
    );
    f.check(&a);
}

#[test]
fn open_mode_nonreveal_penalty_is_always_zero() {
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, 3);
    assert_eq!(c.nonreveal_penalty_bps, 0);
    let a = f.auction(&c, 9);
    let u = f.user();
    f.e.ledger().set_sequence_number(100);
    f.place(&a, &u, 1, 1);
    let client = AuctionClient::new(&f.e, &a);
    assert_eq!(client.accounting().pending_quote, 1);
    f.check(&a);
}

#[test]
fn sealed_replay_domain_mutations_and_insufficient_collateral_preserve_escrow() {
    let f = Fixture::new();
    let config = f.config(Mode::Sale, true, 3);
    let a = f.auction(&config, 60);
    let other = f.auction(&config, 61);
    let client = AuctionClient::new(&f.e, &a);
    let other_client = AuctionClient::new(&f.e, &other);
    let alice = f.user();
    let bob = f.user();
    let carol = f.user();
    f.e.ledger().set_sequence_number(100);
    let (id, salt) = f.commit(&a, &alice, 5 * cc::SCALE, 3, 20);
    let bid = client.get_bid(&id);
    let nonce = bid.nonce.unwrap();
    let hash = bid.commitment.unwrap();
    let before = client.accounting();
    f.signed(
        &alice,
        &a,
        "commit_bid",
        (alice.clone(), nonce.clone(), hash.clone(), 20i128).into_val(&f.e),
        &[],
    );
    assert_eq!(
        client.try_commit_bid(&alice, &nonce, &hash, &20),
        Err(Ok(Error::Replay))
    );
    f.clear_auth();
    assert_eq!(client.accounting(), before);
    let copied_owner = f.submit_commit(&a, &bob, &nonce, &hash, 20);
    let copied_auction = f.submit_commit(&other, &alice, &nonce, &hash, 20);
    let (underfunded, underfunded_salt) = f.commit(&a, &carol, 5 * cc::SCALE, 3, 1);
    f.e.ledger().set_sequence_number(110);
    let wrong_salt = BytesN::from_array(&f.e, &[99; 32]);
    for (address, bid_id, owner, price, quantity, secret, expected) in [
        (&a, id, &alice, 6 * cc::SCALE, 3, &salt, Error::Commitment),
        (&a, id, &alice, 5 * cc::SCALE, 4, &salt, Error::Commitment),
        (
            &a,
            id,
            &alice,
            5 * cc::SCALE,
            3,
            &wrong_salt,
            Error::Commitment,
        ),
        (
            &a,
            copied_owner,
            &bob,
            5 * cc::SCALE,
            3,
            &salt,
            Error::Commitment,
        ),
        (
            &other,
            copied_auction,
            &alice,
            5 * cc::SCALE,
            3,
            &salt,
            Error::Commitment,
        ),
        (
            &a,
            underfunded,
            &carol,
            5 * cc::SCALE,
            3,
            &underfunded_salt,
            Error::Collateral,
        ),
    ] {
        let target = AuctionClient::new(&f.e, address);
        let before = target.accounting();
        f.signed(
            owner,
            address,
            "reveal_bid",
            (bid_id, price, quantity, secret.clone()).into_val(&f.e),
            &[],
        );
        assert_eq!(
            target.try_reveal_bid(&bid_id, &price, &quantity, secret),
            Err(Ok(expected))
        );
        f.clear_auth();
        assert_eq!(target.accounting(), before);
        assert!(target.get_bid(&bid_id).quantity.is_none());
    }
    f.reveal(&a, id, 5 * cc::SCALE, 3, &salt);
    f.signed(
        &alice,
        &a,
        "reveal_bid",
        (id, 5 * cc::SCALE, 3i128, salt.clone()).into_val(&f.e),
        &[],
    );
    assert_eq!(
        client.try_reveal_bid(&id, &(5 * cc::SCALE), &3, &salt),
        Err(Ok(Error::Replay))
    );
    f.clear_auth();
    f.e.ledger().set_sequence_number(115);
    f.close(&a);
    f.close(&other);
    assert!(client.claim_bid(&id).delivered);
    assert_eq!(client.claim_bid(&copied_owner).refund, 20);
    assert_eq!(client.claim_bid(&underfunded).refund, 1);
    assert_eq!(other_client.claim_bid(&copied_auction).refund, 20);
    assert_eq!(client.credit(&alice, &f.quote), 5);
    f.check(&a);
    f.check(&other);
}
