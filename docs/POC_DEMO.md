# Run the smart-contract demonstration

The demonstration executes the compiled AuctionFactory, Auction, and PriceRegistry
WASM in Soroban's local test host against genuine Stellar Asset Contracts (SACs).
Each printed result is backed by assertions on contract state and token balances.
No wallet, client SDK, UI, network deployment, or external service is needed.

From the repository root, with Rustup, Python, and Node installed:

```bash
rustup component add rustfmt clippy
rustup target add wasm32v1-none
./scripts/demo.sh
```

The repository selects Rust 1.94.1 and Soroban SDK 25.3.2. A fresh build needs
access to the dependencies in `Cargo.lock`. A cached environment can use
`CARGO_NET_OFFLINE=true ./scripts/demo.sh`. The script builds WASM before running
the demonstrations, so the tests execute the current contract artifacts.

## What the demonstration proves locally

| Scenario | Funded input | Asserted result |
|---|---|---|
| Open sale | Issuer deposits 10 RWA. Alice bids 6 at 9 and locks 54 quote; Bob bids 8 at 7 and locks 56 quote. | Public tick totals match the bids. Both fills use price 7: Alice gets 6 RWA for 42; Bob gets 4 for 28. They withdraw refunds of 12 and 28; the issuer withdraws 70 after delivery. |
| Public price | The sale has finished delivering. | Registry verifies factory provenance and exposes clearing price 7, allocated volume 10 RWA, and actual execution of 10 RWA / 70 quote through separate records. |
| Sealed buyback | Issuer deposits 100 quote. Three bidders commit and deposit 4, 6, and 3 RWA. | Bid price/quantity fields remain absent before reveal. An altered reveal is rejected. Valid asks of 4 at 8 and 6 at 10 execute at price 10, paying 40 and 60. The non-revealer withdraws all 3 RWA. Both liabilities finish at zero. |
| Revoked winner | A sale bidder loses RWA authorization after the preliminary sweep, before atomic finalization. | The finalizer excludes it and restarts clearing. Its demand cannot set the final price. Reauthorization does not undo exclusion; the bidder withdraws its entire deposit. |

The sale and buyback examples use whole token amounts, encoded internally with
seven decimal places. The compliance regression uses small raw-unit amounts.
The fixtures create accounts/assets and advance ledger numbers locally. Specific
authorization invocation trees stand in for signatures; these are not signed
transactions on a public network. Deterministic demo salts are test fixtures,
not a production secret-generation method. Sealed deposits and commitment
metadata remain public and can reveal information about bids.

## Review the checks and code

```bash
./scripts/check.sh
```

This also runs all contract tests, formatting, Clippy, 50 design tests, generated
model comparisons, and the independent canonical-XDR vector check. The integration
suite has 60 compiled-WASM integration tests and 9 shared unit tests. Beyond the
three displayed scenarios, it covers real factory bootstrap/pin failures, exact
create authorization, salt isolation, deployment rollback, post-clearing refunds,
two-leg rollback, frozen credits, deficits, expiry, registry ordering, the
OracleAdapter's partial SEP-40 read surface, the optional non-reveal penalty, and
very large fractional-price amounts. Dedicated regressions exclude revoked
marginal bids even when their own quantity/payment rounds to zero, and verify
exact-maximum TTL plus reused auction-code retention.

- [Demonstration assertions and runtime tests](../contracts/auction/tests/lifecycle.rs)
- [Architecture](../ARCHITECTURE.md)
- [Contract review and limits of the evidence](security/CONTRACT_REVIEW.md)
- [Auction clearing and settlement logic](security/AUCTION_LOGIC_REVIEW.md)
- [Coverage methodology and measured results](TESTING.md)
- [Design findings and corrections](security/DESIGN_AUDIT.md)

This is a working smart-contract PoC, exercised locally and on Stellar
Testnet (see [deployments/](../deployments/)). No external audit or Mainnet
deployment is claimed. The tested capacity is 16 bids per auction; production
qualification and broader adversarial/runtime testing remain separate work.
