use super::*;

#[derive(serde::Deserialize)]
struct ClearingVectors {
    cases: std::vec::Vec<ClearingCase>,
}
#[derive(serde::Deserialize)]
struct ClearingCase {
    name: std::string::String,
    config: ClearingConfig,
    max_price: std::string::String,
    tick_size: std::string::String,
    min_bid: std::string::String,
    bids: std::vec::Vec<ClearingBid>,
    expected: ClearingExpected,
}
#[derive(serde::Deserialize)]
struct ClearingConfig {
    mode: std::string::String,
    supply: std::string::String,
    min_price: std::string::String,
    min_fill: std::string::String,
}
#[derive(serde::Deserialize)]
struct ClearingBid {
    price: std::string::String,
    quantity: std::string::String,
}
#[derive(serde::Deserialize)]
struct ClearingExpected {
    price: Option<std::string::String>,
    allocations: std::vec::Vec<std::string::String>,
    payments: std::vec::Vec<std::string::String>,
}
fn integer(value: &str) -> i128 {
    value.parse().unwrap()
}

#[test]
fn compiled_wasm_matches_48_independent_clearing_vectors_and_drains_liabilities() {
    let vectors: ClearingVectors = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/vectors/clearing-v1.json"
    )))
    .unwrap();
    assert_eq!(vectors.cases.len(), 48);
    for (case_index, case) in vectors.cases.into_iter().enumerate() {
        let f = Fixture::new();
        let mode = if case.config.mode == "sale" {
            Mode::Sale
        } else {
            Mode::Buyback
        };
        let mut config = f.config(mode, false, integer(&case.config.supply));
        config.min_price = integer(&case.config.min_price);
        config.max_price = integer(&case.max_price);
        config.tick_size = integer(&case.tick_size);
        config.min_bid_quantity = integer(&case.min_bid);
        config.min_fill_quantity = integer(&case.config.min_fill);
        let a = f.auction(&config, 40);
        let client = AuctionClient::new(&f.e, &a);
        f.e.ledger().set_sequence_number(100);
        let users: std::vec::Vec<_> = case
            .bids
            .iter()
            .map(|b| {
                let user = f.user();
                f.place_raw(&a, &user, integer(&b.price), integer(&b.quantity));
                user
            })
            .collect();
        f.e.ledger().set_sequence_number(110);
        // Exercise persisted cursors across 65 ticks and both sweep chunk sizes.
        let work = if case_index % 2 == 0 { 1 } else { 32 };
        for _ in 0..256 {
            match client.state().stage {
                Stage::Ready | Stage::Cancelled | Stage::Refunded => break,
                _ => {
                    client.advance(&work);
                }
            }
        }
        if client.state().stage == Stage::Ready {
            client.finalize();
        }
        assert_eq!(
            client.clearing().map(|c| c.candidate.price),
            case.expected.price.as_deref().map(integer),
            "{}",
            case.name
        );
        // Reverse order checks that claimed reserves do not change later fills.
        for id in (0..users.len()).rev() {
            let result = client.claim_bid(&(id as u32));
            assert_eq!(
                result.rwa,
                integer(&case.expected.allocations[id]),
                "{} bid {id}",
                case.name
            );
            assert_eq!(
                result.quote,
                integer(&case.expected.payments[id]),
                "{} bid {id}",
                case.name
            );
            f.check(&a);
        }
        for owner in users.iter().chain(std::iter::once(&f.issuer)) {
            for asset in [&f.rwa, &f.quote] {
                let amount = client.credit(owner, asset);
                if amount > 0 {
                    f.withdraw(&a, owner, asset, amount);
                }
            }
        }
        assert_eq!(client.accounting().liability_rwa, 0, "{}", case.name);
        assert_eq!(client.accounting().liability_quote, 0, "{}", case.name);
        f.check(&a);
    }
}
