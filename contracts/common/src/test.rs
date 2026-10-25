use super::*;
use soroban_sdk::{contract, contractimpl, testutils::Ledger, xdr, TryIntoVal};
#[contract]
struct Context;
#[contractimpl]
impl Context {
    pub fn noop() {}
}
fn contract_address(env: &Env, fill: u8) -> Address {
    xdr::ScAddress::Contract(xdr::ContractId(xdr::Hash([fill; 32])))
        .try_into_val(env)
        .unwrap()
}
#[test]
fn canonical_xdr_agrees_with_independent_javascript_encoder() {
    let e = Env::default();
    e.ledger().set_network_id([0x33; 32]);
    let auction = contract_address(&e, 0x11);
    let owner: Address = xdr::ScAddress::Account(xdr::AccountId(
        xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256([0x22; 32])),
    ))
    .try_into_val(&e)
    .unwrap();
    let c = Config {
        version: 1,
        rwa_token: contract_address(&e, 0x77),
        quote_token: contract_address(&e, 0x88),
        issuer: owner.clone(),
        buyback_recipient: owner.clone(),
        mode: Mode::Sale,
        bid_mode: BidMode::Sealed,
        nonreveal_penalty_bps: 2_500,
        supply_or_budget: 3,
        min_fill_quantity: 1,
        min_bid_quantity: 1,
        min_deposit: 1,
        min_price: SCALE,
        max_price: 10 * SCALE,
        tick_size: SCALE,
        max_bids: 16,
        max_bids_per_address: 4,
        start_ledger: 100,
        end_ledger: 110,
        reveal_end_ledger: 115,
        close_deadline: 140,
        delivery_deadline: 160,
    };
    assert_eq!(
        config_hash(&e, &c).to_array(),
        [
            91, 204, 195, 218, 9, 34, 50, 245, 99, 203, 35, 19, 13, 220, 131, 215, 165, 18, 203,
            103, 234, 206, 207, 49, 66, 248, 70, 125, 36, 143, 182, 97
        ]
    );
    e.register_at(&auction, Context, ());
    let provenance = Provenance {
        factory: contract_address(&e, 0x99),
        registry: contract_address(&e, 0xaa),
        template: BytesN::from_array(&e, &[0; 32]),
        config_hash: config_hash(&e, &c),
    };
    let value = e.as_contract(&auction, || {
        commitment(
            &e,
            &provenance,
            &owner,
            &BytesN::from_array(&e, &[0x55; 32]),
            12345678901234567890123456789,
            2 * SCALE,
            1234567,
            &BytesN::from_array(&e, &[0x66; 32]),
        )
    });
    assert_eq!(
        value.to_array(),
        [
            235, 157, 90, 82, 9, 218, 243, 74, 85, 175, 211, 19, 207, 187, 122, 129, 134, 206, 50,
            51, 237, 100, 91, 139, 74, 87, 212, 251, 41, 44, 166, 182
        ]
    );
}
#[test]
fn wide_arithmetic_and_checked_result() {
    let e = Env::default();
    assert_eq!(
        mul_div(&e, i128::MAX, i128::MAX, i128::MAX, false),
        Ok(i128::MAX)
    );
    assert_eq!(mul_div(&e, i128::MAX, 2, 1, false), Err(Error::Arithmetic));
    assert_eq!(mul_div(&e, 5, 3, 10, true), Ok(2));
    assert_eq!(mul_div(&e, 5, 3, 10, false), Ok(1));
    assert_eq!(mul_div(&e, -1, 1, 1, false), Err(Error::Arithmetic));
    assert_eq!(mul_div(&e, 1, -1, 1, false), Err(Error::Arithmetic));
    assert_eq!(mul_div(&e, 1, 1, 0, false), Err(Error::Arithmetic));
    assert_eq!(add(i128::MAX, 1), Err(Error::Arithmetic));
    assert_eq!(sub(0, 1), Err(Error::Arithmetic));
    assert_eq!(sub(3, 2), Ok(1));
    let mut accounting = Accounting::zero();
    assert!(accounting.balanced());
    accounting.credit_quote = 1;
    assert!(!accounting.balanced());
    accounting.liability_quote = 1;
    assert!(accounting.balanced());
    accounting.reserve_rwa = -1;
    assert!(!accounting.balanced());
}

#[test]
fn public_wire_records_match_native_scval_and_canonical_guest_encoding() {
    let e = Env::default();
    let owner = contract_address(&e, 1);
    let rwa = contract_address(&e, 2);
    let quote = contract_address(&e, 3);
    let hash = BytesN::from_array(&e, &[4; 32]);
    // Native ScVal conversion is a different generated path from guest IntoVal.
    // Both must yield identical canonical XDR, including every optional record.
    macro_rules! wire {
        ($value:expr) => {{
            let value = $value;
            let scval = xdr::ScVal::try_from(&value).unwrap();
            let encoded = xdr::WriteXdr::to_xdr(&scval, xdr::Limits::none()).unwrap();
            assert_eq!(
                soroban_sdk::Bytes::from_slice(&e, &encoded),
                value.to_xdr(&e)
            );
        }};
    }
    for mode in [Mode::Sale, Mode::Buyback] {
        wire!(mode);
    }
    for mode in [BidMode::Open, BidMode::Sealed] {
        wire!(mode);
    }
    for stage in [
        Stage::Created,
        Stage::Funded,
        Stage::Closing,
        Stage::Sweep,
        Stage::Ready,
        Stage::Cleared,
        Stage::Cancelled,
        Stage::Settled,
        Stage::Refunded,
    ] {
        wire!(stage);
    }
    for reason in [
        Reason::NoDemand,
        Reason::Minimum,
        Reason::Revoked,
        Reason::GlobalAuth,
        Reason::CloseExpired,
        Reason::Unfunded,
        Reason::DeliveryExpired,
        Reason::NonReveal,
        Reason::NotAllocated,
    ] {
        wire!(reason);
    }
    let config = Config {
        version: 1,
        rwa_token: rwa.clone(),
        quote_token: quote.clone(),
        issuer: owner.clone(),
        buyback_recipient: owner.clone(),
        mode: Mode::Sale,
        bid_mode: BidMode::Sealed,
        nonreveal_penalty_bps: 0,
        supply_or_budget: 10,
        min_fill_quantity: 1,
        min_bid_quantity: 1,
        min_deposit: 1,
        min_price: SCALE,
        max_price: 10 * SCALE,
        tick_size: SCALE,
        max_bids: 16,
        max_bids_per_address: 4,
        start_ledger: 100,
        end_ledger: 110,
        reveal_end_ledger: 115,
        close_deadline: 140,
        delivery_deadline: 160,
    };
    wire!(config.clone());
    let provenance = Provenance {
        factory: owner.clone(),
        registry: quote.clone(),
        template: hash.clone(),
        config_hash: config_hash(&e, &config),
    };
    wire!(provenance.clone());
    let meta = AuctionMeta {
        issuer: owner.clone(),
        rwa,
        quote,
        mode: Mode::Sale,
        config_hash: provenance.config_hash.clone(),
        template: hash.clone(),
        ledger: 90,
    };
    wire!(meta.clone());
    let candidate = Candidate {
        price: 2 * SCALE,
        target: 10,
        numerator: 3,
        denominator: 5,
    };
    wire!(candidate.clone());
    wire!(CandidateValue::None);
    wire!(CandidateValue::Some(candidate.clone()));
    assert!(CandidateValue::from(None).is_none());
    assert_eq!(
        CandidateValue::None.ok_or(Error::Missing),
        Err(Error::Missing)
    );
    assert_eq!(
        CandidateValue::from(Some(candidate.clone())).ok_or(Error::Missing),
        Ok(candidate.clone())
    );
    wire!(State {
        stage: Stage::Ready,
        epoch: 1,
        bid_count: 2,
        tick_cursor: 0,
        sweep_cursor: 2,
        cumulative: 5,
        previous_price: SCALE,
        candidate: CandidateValue::Some(candidate.clone())
    });
    wire!(ReasonValue::None);
    wire!(ReasonValue::Some(Reason::Revoked));
    assert!(ReasonValue::None.is_none());
    assert!(ReasonValue::from(Some(Reason::Revoked)).is_some());
    assert_eq!(ReasonValue::None.ok_or(Error::Missing), Err(Error::Missing));
    assert_eq!(
        ReasonValue::Some(Reason::Minimum).ok_or(Error::Missing),
        Ok(Reason::Minimum)
    );
    let resolution = Resolution {
        delivered: true,
        rwa: 3,
        quote: 6,
        refund: 4,
        reason: ReasonValue::None,
        ledger: 120,
        timestamp: 1800000000,
    };
    wire!(resolution.clone());
    wire!(ResolutionValue::None);
    wire!(ResolutionValue::Some(resolution.clone()));
    assert!(ResolutionValue::from(None).is_none());
    assert_eq!(
        ResolutionValue::None.ok_or(Error::Missing),
        Err(Error::Missing)
    );
    assert_eq!(
        ResolutionValue::from(Some(resolution.clone())).ok_or(Error::Missing),
        Ok(resolution.clone())
    );
    wire!(Bid {
        owner: owner.clone(),
        deposit: 10,
        nonce: None,
        commitment: None,
        price: None,
        quantity: None,
        excluded: false,
        resolution: ResolutionValue::None
    });
    wire!(Bid {
        owner: owner.clone(),
        deposit: 10,
        nonce: Some(hash.clone()),
        commitment: Some(hash),
        price: Some(2 * SCALE),
        quantity: Some(3),
        excluded: false,
        resolution: ResolutionValue::Some(resolution)
    });
    wire!(Accounting::zero());
    let clearing = Clearing {
        candidate,
        filled: 10,
        quote: 20,
        winners: 2,
        epoch: 1,
        ledger: 115,
        timestamp: 1800000000,
    };
    let settlement = Settlement {
        executed_rwa: 10,
        executed_quote: 20,
        refunded_rwa: 0,
        refunded_quote: 5,
        resolved: 2,
        ledger: 120,
        timestamp: 1800000010,
    };
    wire!(clearing.clone());
    wire!(settlement.clone());
    wire!(Finalization::Cleared(clearing.clone()));
    wire!(Finalization::Excluded(0, 1));
    wire!(Finalization::Cancelled(Reason::Minimum));
    wire!(Progress {
        stage: Stage::Ready,
        epoch: 1,
        cursor: 2,
        processed: 2
    });
    wire!(Observation {
        auction: owner.clone(),
        meta,
        clearing,
        delivery_deadline: 160,
        published_at: 1800000011
    });
    wire!(Execution {
        auction: owner,
        clearing_ledger: 115,
        settlement,
        published_at: 1800000012
    });
}

fn base_config(e: &Env) -> Config {
    let owner = contract_address(e, 0x22);
    Config {
        version: 1,
        rwa_token: contract_address(e, 0x77),
        quote_token: contract_address(e, 0x88),
        issuer: owner.clone(),
        buyback_recipient: owner,
        mode: Mode::Sale,
        bid_mode: BidMode::Open,
        nonreveal_penalty_bps: 0,
        supply_or_budget: 10,
        min_fill_quantity: 1,
        min_bid_quantity: 1,
        min_deposit: 1,
        min_price: SCALE,
        max_price: 10 * SCALE,
        tick_size: SCALE,
        max_bids: MAX_BIDS,
        max_bids_per_address: MAX_BIDS,
        start_ledger: 100,
        end_ledger: 110,
        reveal_end_ledger: 110,
        close_deadline: 140,
        delivery_deadline: 160,
    }
}

// These bound checks all fail inside validate_config's leading OR-chain,
// before any SAC/token call, so a Config with non-genuine placeholder
// addresses can exercise them directly without registering real assets --
// real-asset-dependent paths (admin/decimals mismatches) are already
// covered end to end via the auction contract's integration tests.
#[test]
fn validate_config_rejects_each_leading_bound_independently() {
    let e = Env::default();
    e.ledger().set_sequence_number(90);
    let valid = base_config(&e);
    macro_rules! invalid {
        ($field:ident, $value:expr) => {{
            let mut c = valid.clone();
            c.$field = $value;
            assert_eq!(
                validate_config(&e, &c),
                Err(Error::Config),
                "expected Config error for {} = {:?}",
                stringify!($field),
                $value
            );
        }};
    }
    invalid!(version, 2);
    invalid!(supply_or_budget, 0);
    invalid!(min_fill_quantity, 0);
    invalid!(min_bid_quantity, 0);
    invalid!(min_deposit, 0);
    invalid!(min_price, 0);
    invalid!(max_price, SCALE - 1);
    invalid!(tick_size, 0);
    invalid!(max_bids, 0);
    invalid!(max_bids, MAX_BIDS + 1);
    invalid!(max_bids_per_address, 0);
    invalid!(nonreveal_penalty_bps, MAX_NONREVEAL_PENALTY_BPS + 1);
    invalid!(start_ledger, 90);
    invalid!(reveal_end_ledger, 109);
    invalid!(close_deadline, 109);
    invalid!(delivery_deadline, 139);

    let mut sealed_zero = valid.clone();
    sealed_zero.bid_mode = BidMode::Open;
    sealed_zero.nonreveal_penalty_bps = 1;
    assert_eq!(
        validate_config(&e, &sealed_zero),
        Err(Error::Config),
        "Open mode must force nonreveal_penalty_bps to zero"
    );

    let mut sealed_window = valid.clone();
    sealed_window.bid_mode = BidMode::Sealed;
    sealed_window.reveal_end_ledger = sealed_window.end_ledger; // must be strictly after end
    assert_eq!(validate_config(&e, &sealed_window), Err(Error::Config));
}

#[test]
fn financial_ttl_boundary_matches_auction_contract_expectations() {
    let e = Env::default();
    e.ledger().set_sequence_number(1_000);
    let max = e.storage().max_ttl();
    let mut c = base_config(&e);
    // Exactly at the maximum supported horizon: accepted, and the returned
    // TTL is at least RETENTION (never less than the rolling-extension floor).
    c.delivery_deadline = 1_000 + max - RECOVERY;
    let ttl = financial_ttl(&e, &c).unwrap();
    assert!(ttl >= RETENTION.min(max));
    assert!(ttl <= max);
    // One ledger further out than the network's maximum TTL can support.
    c.delivery_deadline += 1;
    assert_eq!(financial_ttl(&e, &c), Err(Error::Ttl));
}

#[test]
fn tick_helpers_cover_the_full_grid_and_reject_off_grid_prices() {
    let e = Env::default();
    let c = base_config(&e); // min=SCALE, max=10*SCALE, tick=SCALE -> 10 ticks
    assert_eq!(tick_count(&c), 10);
    assert_eq!(tick_index(&c, SCALE), Ok(0));
    assert_eq!(tick_index(&c, 10 * SCALE), Ok(9));
    assert_eq!(tick_index(&c, SCALE - 1), Err(Error::Price));
    assert_eq!(tick_index(&c, 10 * SCALE + 1), Err(Error::Price));
    assert_eq!(tick_index(&c, SCALE + SCALE / 2), Err(Error::Price));
    assert_eq!(validate_bid(&c, SCALE, 0), Err(Error::Amount));
}

#[test]
fn penalty_floors_in_the_bidders_favor_and_is_zero_at_zero_bps() {
    let e = Env::default();
    assert_eq!(penalty(&e, 1_000, 0).unwrap(), 0);
    assert_eq!(penalty(&e, 1_000, 2_500).unwrap(), 250);
    // 999 * 2500 / 10000 = 249.75 -> floors to 249, never rounds up against
    // the bidder.
    assert_eq!(penalty(&e, 999, 2_500).unwrap(), 249);
    assert_eq!(penalty(&e, 1_000, MAX_NONREVEAL_PENALTY_BPS).unwrap(), 500);
}

#[test]
fn allocation_zeroes_out_fills_whose_payment_rounds_to_zero() {
    let e = Env::default();
    let c = base_config(&e);
    let candidate = Candidate {
        price: SCALE,
        target: 10,
        numerator: 1,
        denominator: 10,
    };
    let excluded = Bid {
        owner: contract_address(&e, 0x33),
        deposit: 100,
        nonce: None,
        commitment: None,
        price: Some(SCALE),
        quantity: Some(10),
        excluded: true,
        resolution: ResolutionValue::None,
    };
    assert_eq!(allocation(&e, &c, &excluded, &candidate).unwrap(), (0, 0));
    let unrevealed = Bid {
        quantity: None,
        ..excluded.clone()
    };
    assert_eq!(allocation(&e, &c, &unrevealed, &candidate).unwrap(), (0, 0));
    // Buyback's `cost` floors (unlike Sale's ceil), so a fill whose nominal
    // quantity is positive but whose price is far below SCALE can floor to a
    // zero payment. allocation() must still report (0, 0), per its
    // documented anti-dust rule, not a positive quantity with zero payment.
    let mut buyback = c.clone();
    buyback.mode = Mode::Buyback;
    let tiny_fraction = Candidate {
        price: 1,
        target: 10,
        numerator: 1,
        denominator: 1,
    };
    let revealed = Bid {
        excluded: false,
        price: Some(0),
        quantity: Some(1),
        ..excluded
    };
    assert_eq!(
        allocation(&e, &buyback, &revealed, &tiny_fraction).unwrap(),
        (0, 0)
    );
}

#[test]
fn independent_commitment_matrix_covers_contract_owners_boundaries_and_every_domain_field() {
    // Expected hashes were generated by the independent Node manual XDR encoder,
    // never by this helper or the Rust SDK. Full typed inputs and encoded
    // preimages live in tests/vectors/commitment-domains.json. Mutated configs
    // are wire conformance inputs and need not satisfy auction admission rules.
    const VECTORS: &[(&str, &str, &str, &str)] = &[
        (
            "legacy_sale_account_owner",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "eb9d5a5209daf34a55afd313cfbb7a8186ce3233ed645b8b4a57d4fb292ca6b6",
        ),
        (
            "buyback_contract_owner",
            "aeec4a1d001a3ff404b7b5d2980e80d3f3a6a09e93b6ff838af2d370a6fee0ac",
            "aeec4a1d001a3ff404b7b5d2980e80d3f3a6a09e93b6ff838af2d370a6fee0ac",
            "1004c17a901f6ee7c41f67cedf3c7e8a8b595574ece5cb04252abad68a7647ae",
        ),
        (
            "amount_high64",
            "e5f0680eddf793f6d61d9137656f2eb189b1518930aa0bc4e00165d31cc991df",
            "e5f0680eddf793f6d61d9137656f2eb189b1518930aa0bc4e00165d31cc991df",
            "e551f86ec8b8daa7677e65c7d6ae6ba547b9ec021ea8d184a05999d60c57e5a1",
        ),
        (
            "amount_i128_max",
            "78ec1b1e007b8ae8b4031a639175403be24a4dd0abb35dd679c938eed74d9fd3",
            "78ec1b1e007b8ae8b4031a639175403be24a4dd0abb35dd679c938eed74d9fd3",
            "3e6138ea255db41f3531118c29815ef424af3f72849e9eb1779faa9e37dbd14a",
        ),
        (
            "config_version",
            "0c6723e0d06a2e045e6ded27ed4ff997d11f71c86298629751d68c41b031c0d1",
            "0c6723e0d06a2e045e6ded27ed4ff997d11f71c86298629751d68c41b031c0d1",
            "ebdeafccb4e0eaa523aa7cb1cdbed35587d1b98ac2e2d82ffeee3a71ff48feb4",
        ),
        (
            "config_rwa_token",
            "dc58d2e0f199d9b1a7bf45f16b57a31055e0a559ea3eb275c4a3a8a8ea6f1614",
            "dc58d2e0f199d9b1a7bf45f16b57a31055e0a559ea3eb275c4a3a8a8ea6f1614",
            "3744d9d7f5879cd0bf592d57442be3fc959c2afeb6b3907b34770b0678f88bed",
        ),
        (
            "config_quote_token",
            "3b901d0f3852c325674c1d1db998eb1930e546d1ab609be9c4ab9d16ae5010ea",
            "3b901d0f3852c325674c1d1db998eb1930e546d1ab609be9c4ab9d16ae5010ea",
            "dc709d3f03bf4366f7a9de8c5bb114c3366b81d8019ede5e786850dc6c6db1de",
        ),
        (
            "config_issuer",
            "16d26213d48b29346d670afd1ef636fdc1496792ce9b23ee4360625bbd8b24c9",
            "16d26213d48b29346d670afd1ef636fdc1496792ce9b23ee4360625bbd8b24c9",
            "48698f92ce497da8accf3fac1d0ecd199625877d15dba11461c8c1a4e1c08b3d",
        ),
        (
            "config_buyback_recipient",
            "f88154fdd7c3f4a8a3d545cc427f99a4de7da260d29eb779052b58de67c7e2fb",
            "f88154fdd7c3f4a8a3d545cc427f99a4de7da260d29eb779052b58de67c7e2fb",
            "33a9e9361af3998630598388bbce1d006b3be7c0693349da7bbf1981ecca664a",
        ),
        (
            "config_mode",
            "aeec4a1d001a3ff404b7b5d2980e80d3f3a6a09e93b6ff838af2d370a6fee0ac",
            "aeec4a1d001a3ff404b7b5d2980e80d3f3a6a09e93b6ff838af2d370a6fee0ac",
            "d7af8bb199e007908c1affd5d1f8c027bfd974958a03697f22e98c65411fdb90",
        ),
        (
            "config_bid_mode",
            "e7b8363caeedf6e7addce08a0e046dddb1c28cea3cd6006e50956cfa570b695d",
            "e7b8363caeedf6e7addce08a0e046dddb1c28cea3cd6006e50956cfa570b695d",
            "0a0b3d5432fbfcb64ac5d2c70231837e59bf5b8aaf4b5984f563b744f237224e",
        ),
        (
            "config_nonreveal_penalty_bps",
            "ee27b882f0f926f473dbada0a33d73973db2b7a35ccd3f2b722cf59456e38352",
            "ee27b882f0f926f473dbada0a33d73973db2b7a35ccd3f2b722cf59456e38352",
            "ae2eb6f2d69488686d707dccd1342196e91013778a5a1ae5eb68df9901325f3a",
        ),
        (
            "config_supply_or_budget",
            "a00be818cc632be6009f91607f8b213276c92a7fac88a4e1271310ca7bf54a0a",
            "a00be818cc632be6009f91607f8b213276c92a7fac88a4e1271310ca7bf54a0a",
            "721d58ebfe48928cba4a0d25e6b5a4bb9d226a3ce3268caea69ebd3351a4f169",
        ),
        (
            "config_min_fill_quantity",
            "ec140fcc7007a86ab69145b75b4cf5876f4d6f7d28a6fd3b1ff274bfc16eb849",
            "ec140fcc7007a86ab69145b75b4cf5876f4d6f7d28a6fd3b1ff274bfc16eb849",
            "a1a38fbec4b57b43c97bea535fd105e555e22c4435897e6d0c6fce5d60505004",
        ),
        (
            "config_min_bid_quantity",
            "49e10b0aa443482ab70f48ae5b8c5ffc994b52f9a693b0191d49bbbf7931ec24",
            "49e10b0aa443482ab70f48ae5b8c5ffc994b52f9a693b0191d49bbbf7931ec24",
            "75bf9c4081a8d9015ae64bb6cd3041299faf6fdb49546250eed41af1675cb795",
        ),
        (
            "config_min_deposit",
            "427a14b61c39d4b342ce304fd12dcab5df544e3a534198306351e943dc63a6f4",
            "427a14b61c39d4b342ce304fd12dcab5df544e3a534198306351e943dc63a6f4",
            "2884ad2a86ce7fc14f443454c8e09020575480e170ea8def725b2a96d6060093",
        ),
        (
            "config_min_price",
            "9a52375f4f84f94bab9eb3285c53d9b3667388f4d76d0ffcbe2673259ed7bb30",
            "9a52375f4f84f94bab9eb3285c53d9b3667388f4d76d0ffcbe2673259ed7bb30",
            "2c695a2cb54bc44ef9b36f9c556d760f642bc1616ecaf6e3f2b5af8363e3f553",
        ),
        (
            "config_max_price",
            "40f1dad2a6b103ccddcef24b5c63bcb7c195d280d078d1f90114a74fc0aca3d1",
            "40f1dad2a6b103ccddcef24b5c63bcb7c195d280d078d1f90114a74fc0aca3d1",
            "17053c3bab3ed7ed2a5a586e9c52b640d027cdce201429fc5f857582ae156963",
        ),
        (
            "config_tick_size",
            "076155e648ea7747cf53ddbf7e7bf1b3ee6c6b9d271a018ab36ecf48beb59140",
            "076155e648ea7747cf53ddbf7e7bf1b3ee6c6b9d271a018ab36ecf48beb59140",
            "6a70f91306e1ab1297da9442157c0ba10c77fc63ad617b01109b379230145e87",
        ),
        (
            "config_max_bids",
            "5f03abd1e2d32728576c0cfb588519615abf28a153dd42c8c0bbb2c29902c8d2",
            "5f03abd1e2d32728576c0cfb588519615abf28a153dd42c8c0bbb2c29902c8d2",
            "9dfb20d356bdd6fea6d6b9e1b223a0b6be08d9b2482e41c65e0eb1bc2496f9f1",
        ),
        (
            "config_max_bids_per_address",
            "16a6d118b9654fe2c0dd15cf9ae349e1c3bf3928ba0946eb5600d5072a20a978",
            "16a6d118b9654fe2c0dd15cf9ae349e1c3bf3928ba0946eb5600d5072a20a978",
            "2c12664b6f96f889f0626d2f5dff8bf12ca5593073d2ba82aa64b68bb15f7605",
        ),
        (
            "config_start_ledger",
            "dd2aee6dbf5330d0aefcee87bfd2a2f8eb3636d46b7831dde786e3f45c7f1c1f",
            "dd2aee6dbf5330d0aefcee87bfd2a2f8eb3636d46b7831dde786e3f45c7f1c1f",
            "5819102d1ea649c62804e70aa5a4b4ec46be7f6b5c0a45144327fd85d705b1f7",
        ),
        (
            "config_end_ledger",
            "38dcc34931bfd6de4927b49451c01c88d32ea8865b7ae0ffdc3ee2682919d968",
            "38dcc34931bfd6de4927b49451c01c88d32ea8865b7ae0ffdc3ee2682919d968",
            "bc3b87525f5935bc46e9855f85b84a1c7de2d3a10185677302b0c43881204599",
        ),
        (
            "config_reveal_end_ledger",
            "242ddf5cafffefc3896f288077dcd61ab09a63d7939309573b12ae6dad0de15c",
            "242ddf5cafffefc3896f288077dcd61ab09a63d7939309573b12ae6dad0de15c",
            "a7d4f328ea4e2b307a2c0866b62ca7840062a4f504cdc994dc0f8932fd7bd342",
        ),
        (
            "config_close_deadline",
            "31c9b34935ccb154290a657ded047cf17187e9b88f743a12c832b18bb938bf1a",
            "31c9b34935ccb154290a657ded047cf17187e9b88f743a12c832b18bb938bf1a",
            "e04799c69481c59a6a8a6e2d4bf5cc332df90c7f6ac84bbd8b677818f5ae7383",
        ),
        (
            "config_delivery_deadline",
            "fee9347b2e5269b0bce9f4ba3f8a68c5e76ba8ae7b430948487dcb71aec5aa1a",
            "fee9347b2e5269b0bce9f4ba3f8a68c5e76ba8ae7b430948487dcb71aec5aa1a",
            "c8bcb0ae8e75bdb2f6eda490344871dc11d5bd1d2bd5b354834fa939408ac7c1",
        ),
        (
            "field_domain",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "ea438abb68efb9c37a6b7a9da30507d09dd74c4e50017acc6232de7830433ba8",
        ),
        (
            "field_network",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "926445e280d263c51231829073fb53cb40efdb2fa297a23df45da5c5f2e49920",
        ),
        (
            "field_auction",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "71781c8ddabc1598130e2dbbd4ed00bf99a93a0f1f758e5504ce0f7d2b3e401f",
        ),
        (
            "field_owner",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "e8f32f5d49752ff21c1ab25029b2cbf0cfb0cb0ef9ee4206c43f009ff9548283",
        ),
        (
            "field_nonce",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "97357dcb5faf7e735ac40e3dbf89411102dd57f494008f61d7548a1afe179cfa",
        ),
        (
            "field_config_hash",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "9999999999999999999999999999999999999999999999999999999999999999",
            "18b3070f02eda649c18532d696770b6e954860123ee1842ca091fdeee02b0959",
        ),
        (
            "field_locked",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "088b1ea8e868e653df8ebe56424895760180256065ba4a3e319d897de727a09b",
        ),
        (
            "field_price",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "6beccba79c6694f8e10e5e95138a1daf903844f94f808f5d46df45ad39b2c94b",
        ),
        (
            "field_quantity",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "1ad40daad1eac3cb087c825f629c73dab720cf2b560c11773d9fe41d5e1500e1",
        ),
        (
            "field_salt",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "5bccc3da092232f563cb23130ddc83d7a512cb67eacecf3142f8467d248fb661",
            "0ff68463daab21ff87d266bb4896880639d37099f5e0b79b9ae9cde4d693e6e7",
        ),
    ];
    fn hex32(value: &str) -> [u8; 32] {
        assert_eq!(value.len(), 64);
        let mut output = [0; 32];
        for (index, byte) in output.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[2 * index..2 * index + 2], 16).unwrap();
        }
        output
    }
    assert_eq!(VECTORS.len(), 36);
    for &(name, expected_config, expected_bound, expected_commitment) in VECTORS {
        let e = Env::default();
        let account = |fill| -> Address {
            xdr::ScAddress::Account(xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(
                xdr::Uint256([fill; 32]),
            )))
            .try_into_val(&e)
            .unwrap()
        };
        let mut owner = account(0x22);
        let mut auction = contract_address(&e, 0x11);
        let mut c = Config {
            version: 1,
            rwa_token: contract_address(&e, 0x77),
            quote_token: contract_address(&e, 0x88),
            issuer: owner.clone(),
            buyback_recipient: owner.clone(),
            mode: Mode::Sale,
            bid_mode: BidMode::Sealed,
            nonreveal_penalty_bps: 2_500,
            supply_or_budget: 3,
            min_fill_quantity: 1,
            min_bid_quantity: 1,
            min_deposit: 1,
            min_price: SCALE,
            max_price: 10 * SCALE,
            tick_size: SCALE,
            max_bids: 16,
            max_bids_per_address: 4,
            start_ledger: 100,
            end_ledger: 110,
            reveal_end_ledger: 115,
            close_deadline: 140,
            delivery_deadline: 160,
        };
        let mut network = [0x33; 32];
        let mut nonce = BytesN::from_array(&e, &[0x55; 32]);
        let mut locked = 12345678901234567890123456789i128;
        let mut price = 2 * SCALE;
        let mut quantity = 1234567i128;
        let mut salt = BytesN::from_array(&e, &[0x66; 32]);
        let mut domain = "CROSSING_BID_V1";
        let mut override_hash = None;
        match name {
            "legacy_sale_account_owner" => {}
            "buyback_contract_owner" => {
                c.mode = Mode::Buyback;
                owner = contract_address(&e, 0x22);
            }
            "amount_high64" => {
                c.supply_or_budget = 1i128 << 64;
                locked = c.supply_or_budget;
                quantity = locked;
                price = SCALE;
            }
            "amount_i128_max" => {
                c.supply_or_budget = i128::MAX;
                c.max_price = SCALE;
                locked = i128::MAX;
                quantity = i128::MAX;
                price = SCALE;
            }
            "field_domain" => domain = "CROSSING_BID_V2",
            "field_network" => network = [0x34; 32],
            "field_auction" => auction = contract_address(&e, 0x12),
            "field_owner" => owner = contract_address(&e, 0x22),
            "field_nonce" => nonce = BytesN::from_array(&e, &[0x56; 32]),
            "field_config_hash" => override_hash = Some(BytesN::from_array(&e, &[0x99; 32])),
            "field_locked" => locked += 1,
            "field_price" => price += 1,
            "field_quantity" => quantity += 1,
            "field_salt" => salt = BytesN::from_array(&e, &[0x67; 32]),
            _ => match name.strip_prefix("config_").unwrap() {
                "version" => c.version += 1,
                "rwa_token" => c.rwa_token = contract_address(&e, 0x78),
                "quote_token" => c.quote_token = contract_address(&e, 0x89),
                "issuer" => c.issuer = account(0x23),
                "buyback_recipient" => c.buyback_recipient = account(0x23),
                "mode" => c.mode = Mode::Buyback,
                "bid_mode" => c.bid_mode = BidMode::Open,
                "nonreveal_penalty_bps" => c.nonreveal_penalty_bps += 1,
                "supply_or_budget" => c.supply_or_budget += 1,
                "min_fill_quantity" => c.min_fill_quantity += 1,
                "min_bid_quantity" => c.min_bid_quantity += 1,
                "min_deposit" => c.min_deposit += 1,
                "min_price" => c.min_price += 1,
                "max_price" => c.max_price += 1,
                "tick_size" => c.tick_size += 1,
                "max_bids" => c.max_bids += 1,
                "max_bids_per_address" => c.max_bids_per_address += 1,
                "start_ledger" => c.start_ledger += 1,
                "end_ledger" => c.end_ledger += 1,
                "reveal_end_ledger" => c.reveal_end_ledger += 1,
                "close_deadline" => c.close_deadline += 1,
                "delivery_deadline" => c.delivery_deadline += 1,
                _ => panic!("unknown independent vector {name}"),
            },
        }
        e.ledger().set_network_id(network);
        let encoded_config_hash = config_hash(&e, &c);
        assert_eq!(
            encoded_config_hash.to_array(),
            hex32(expected_config),
            "{name} config"
        );
        let bound_hash = override_hash.unwrap_or(encoded_config_hash);
        assert_eq!(
            bound_hash.to_array(),
            hex32(expected_bound),
            "{name} bound config"
        );
        let provenance = Provenance {
            factory: contract_address(&e, 0x99),
            registry: contract_address(&e, 0xaa),
            template: BytesN::from_array(&e, &[0; 32]),
            config_hash: bound_hash.clone(),
        };
        e.register_at(&auction, Context, ());
        let actual = e.as_contract(&auction, || {
            commitment(
                &e,
                &provenance,
                &owner,
                &nonce,
                locked,
                price,
                quantity,
                &salt,
            )
        });
        if name == "field_domain" {
            // The production domain is deliberately fixed. Verify the changed
            // domain's independent digest and prove the helper rejects that
            // substitution by continuing to produce the original V1 digest.
            let changed_preimage = (
                Symbol::new(&e, domain),
                BytesN::from_array(&e, &network),
                auction.clone(),
                owner.clone(),
                nonce.clone(),
                bound_hash,
                locked,
                price,
                quantity,
                salt.clone(),
            )
                .to_xdr(&e);
            let changed = e.crypto().sha256(&changed_preimage);
            assert_eq!(changed.to_array(), hex32(expected_commitment), "{name}");
            assert_ne!(actual.to_array(), changed.to_array());
            assert_eq!(actual.to_array(), hex32(VECTORS[0].3));
        } else {
            assert_eq!(actual.to_array(), hex32(expected_commitment), "{name}");
        }
    }
}
