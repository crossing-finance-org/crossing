//! Local compiled-WASM resource regressions. The pinned host measures ledger
//! entry and event XDR, but does not simulate a network transaction envelope.
use super::*;
use soroban_sdk::xdr::{LedgerKey, Limits, ScAddress, WriteXdr};

fn uses_wasm_resource_measurement() -> bool {
    std::env::var("CROSSING_TEST_MODE")
        .unwrap_or_default()
        .is_empty()
}

fn assert_local_resources(f: &Fixture, a: &Address, result: &Finalization, label: &str) {
    // Capture the invocation before serializing its return or inspecting state.
    let resources = f.e.cost_estimate().resources();
    // This deliberately conservative count also counts writable entries in the
    // read set. It matches the existing finalizer regression's ceiling.
    let footprint =
        resources.disk_read_entries + resources.memory_read_entries + resources.write_entries;
    let return_bytes = result.clone().to_xdr(&f.e).len();

    // Measure actual canonical LedgerKey/LedgerEntry encodings over the auction
    // and both SACs. This is a state-size superset, not a reconstructed footprint
    // or a transaction-size estimate. The host independently enforces entry
    // limits on the precise invocation footprint with its default limiter.
    let contracts = [a, &f.rwa, &f.quote].map(ScAddress::from);
    let mut max_key_bytes = 0usize;
    let mut max_entry_bytes = 0usize;
    for (key, (entry, _)) in f.e.to_ledger_snapshot().ledger_entries {
        if let LedgerKey::ContractData(data) = key.as_ref() {
            if contracts.contains(&data.contract) {
                max_key_bytes = max_key_bytes.max(
                    WriteXdr::to_xdr(key.as_ref(), Limits::none())
                        .unwrap()
                        .len(),
                );
                max_entry_bytes = max_entry_bytes.max(
                    WriteXdr::to_xdr(entry.as_ref(), Limits::none())
                        .unwrap()
                        .len(),
                );
            }
        }
    }
    println!(
        "compiled-WASM {label}: cpu={}, memory={}, conservative_footprint={footprint}, \
         disk_read_bytes={}, write_bytes={}, event_bytes={}, return_bytes={return_bytes}, \
         state_max_key_bytes={max_key_bytes}, state_max_entry_bytes={max_entry_bytes}",
        resources.instructions,
        resources.mem_bytes,
        resources.disk_read_bytes,
        resources.write_bytes,
        resources.contract_events_size_bytes,
    );
    // These are local regression ceilings. Network configuration and complete
    // transaction simulation remain separate deployment checks.
    assert!(resources.instructions < 80_000_000);
    assert!(resources.mem_bytes < 32_000_000);
    assert!(footprint <= 100);
    assert!(resources.disk_read_bytes <= 200_000);
    assert!(resources.write_bytes <= 132_096);
    assert!(resources.contract_events_size_bytes <= 16_384);
    assert!(return_bytes > 0 && return_bytes <= 4_096);
    assert!(max_key_bytes > 0 && max_key_bytes <= 250);
    assert!(max_entry_bytes > 0 && max_entry_bytes <= 65_536);
}

#[test]
fn maximum_buyback_finalizer_fits_local_resources_with_proration_and_remainder() {
    if !uses_wasm_resource_measurement() {
        return;
    }
    let f = Fixture::new();
    let c = f.config(Mode::Buyback, false, 33);
    let a = f.auction(&c, 0xc0);
    let owners: std::vec::Vec<_> = (0..cc::MAX_BIDS).map(|_| f.user()).collect();
    f.e.ledger().set_sequence_number(100);
    for owner in &owners {
        f.place(&a, owner, 2, 2);
    }
    f.e.ledger().set_sequence_number(110);
    f.ready(&a);
    let client = AuctionClient::new(&f.e, &a);
    f.e.cost_estimate().budget().reset_unlimited();
    f.e.cost_estimate().budget().reset_tracker();
    let result = client.finalize();
    assert_local_resources(&f, &a, &result, "16-owner prorated buyback finalization");
    let Finalization::Cleared(clearing) = result else {
        panic!("funded buyback must clear");
    };
    assert_eq!(clearing.winners, cc::MAX_BIDS);
    assert_eq!(clearing.filled, 16);
    assert_eq!(clearing.quote, 32);
    assert_eq!(client.credit(&f.issuer, &f.quote), 1);
    assert_eq!(client.accounting().reserve_quote, 32);
    for (id, owner) in owners.iter().enumerate() {
        let resolution = client.claim_bid(&(id as u32));
        assert!(resolution.delivered);
        assert_eq!(resolution.refund, 1);
        assert_eq!(resolution.rwa, 1);
        assert_eq!(resolution.quote, 2);
        assert_eq!(token::Client::new(&f.e, &f.quote).balance(owner), 1_000_002);
        f.withdraw(&a, owner, &f.rwa, 1);
    }
    f.withdraw(&a, &f.issuer, &f.quote, 1);
    assert_eq!(client.accounting().liability_rwa, 0);
    assert_eq!(client.accounting().liability_quote, 0);
    f.check(&a);
}

#[test]
fn last_bid_revocation_finalizer_fits_local_resources_and_recovers_without_delivery() {
    if !uses_wasm_resource_measurement() {
        return;
    }
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, cc::MAX_BIDS as i128);
    let a = f.auction(&c, 0xc1);
    let owners: std::vec::Vec<_> = (0..cc::MAX_BIDS).map(|_| f.user()).collect();
    f.e.ledger().set_sequence_number(100);
    for owner in &owners {
        f.place(&a, owner, 1, 1);
    }
    f.e.ledger().set_sequence_number(110);
    f.ready(&a);
    let client = AuctionClient::new(&f.e, &a);
    let old_epoch = client.state().epoch;
    let last = cc::MAX_BIDS - 1;
    // Revoke after the sweep so finalize must scan all earlier valid bids
    // before taking its exclusion, tick update and continuation-reset branch.
    f.set_auth(&owners[last as usize], &f.rwa, false);
    f.e.cost_estimate().budget().reset_unlimited();
    f.e.cost_estimate().budget().reset_tracker();
    let result = client.finalize();
    assert_local_resources(&f, &a, &result, "16th-owner late-revocation exclusion");
    assert_eq!(result, Finalization::Excluded(last, old_epoch + 1));
    assert!(client.get_bid(&last).excluded);
    assert_eq!(client.state().stage, Stage::Closing);
    assert_eq!(client.tick_total(&0), cc::MAX_BIDS as i128 - 1);
    assert!(client.clearing().is_none());
    // Reauthorization cannot restore excluded demand. The remaining funded
    // book clears, and every participant can recover their owned assets.
    f.set_auth(&owners[last as usize], &f.rwa, true);
    f.close(&a);
    assert_eq!(client.clearing().unwrap().winners, last);
    for id in 0..cc::MAX_BIDS {
        let resolution = client.claim_bid(&id);
        assert_eq!(resolution.delivered, id != last);
        assert_eq!(resolution.refund, if id == last { 1 } else { 0 });
    }
    f.withdraw(&a, &owners[last as usize], &f.quote, 1);
    f.withdraw(&a, &f.issuer, &f.rwa, 1);
    f.withdraw(&a, &f.issuer, &f.quote, last as i128);
    assert_eq!(client.accounting().liability_rwa, 0);
    assert_eq!(client.accounting().liability_quote, 0);
    f.check(&a);
}

#[test]
fn bounded_finalizer_fits_measured_resource_limits() {
    if !std::env::var("CROSSING_TEST_MODE")
        .unwrap_or_default()
        .is_empty()
    {
        return; // Native line coverage is not a WASM resource measurement.
    }
    let f = Fixture::new();
    let c = f.config(Mode::Sale, false, cc::MAX_BIDS as i128);
    let a = f.auction(&c, 12);
    let client = AuctionClient::new(&f.e, &a);
    f.e.ledger().set_sequence_number(100);
    for _ in 0..cc::MAX_BIDS {
        let u = f.user();
        f.place(&a, &u, 1, 1);
    }
    f.e.ledger().set_sequence_number(110);
    f.ready(&a);
    // Keep the SDK's default per-invocation resource limiter enabled. The
    // global test budget also meters authorization observation; measure the
    // actual invocation separately rather than limiting that test machinery.
    f.e.cost_estimate().budget().reset_unlimited();
    f.e.cost_estimate().budget().reset_tracker();
    let result = client.finalize();
    let resources = f.e.cost_estimate().resources();
    let cpu = resources.instructions;
    let mem = resources.mem_bytes;
    let footprint =
        resources.disk_read_entries + resources.memory_read_entries + resources.write_entries;
    assert!(matches!(result, Finalization::Cleared(_)));

    println!(
        "compiled-WASM {}-winner finalization: cpu={cpu}, memory={mem}, footprint={footprint}",
        cc::MAX_BIDS
    );

    assert!(cpu < 80_000_000 && mem < 32_000_000 && footprint <= 100);
    assert_eq!(client.clearing().unwrap().winners, cc::MAX_BIDS);
    let ttl = f.e.deployer().get_contract_instance_ttl(&a);
    assert!(110 + ttl >= c.delivery_deadline + cc::RECOVERY);
    assert!(f.e.deployer().get_contract_code_ttl(&a) >= ttl);
}
