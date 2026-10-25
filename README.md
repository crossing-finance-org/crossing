# Crossing Protocol

**Funded, single-price auctions for real-world assets on Stellar.**

Crossing lets an RWA issuer sell escrowed inventory or buy back tokens against an
escrowed settlement-asset budget. Authorized participants submit funded open or
sealed bids. The contracts rank eligible demand/supply, determine one clearing
price, and resolve each allocation independently. A public registry records the
clearing result and separate actual execution totals.

**Current status:** working smart-contract PoC, internally reviewed, deployed
to and exercised on Stellar Testnet with 100+ real on-chain transactions
across both modes and both bid modes (see
[deployments/](deployments/)). There is no Mainnet deployment and no
independent external contract audit. Documentation matches contract baseline
`0.1.0` and architecture v1.6. The [documentation index](docs/README.md)
tracks review dates and document ownership. The PoC implements the contracts and
their local and Testnet demonstration. The technical architecture covers the full product, including
the future client SDK, dApp, indexer, keepers, and curated price integration.

[Run the demo](docs/POC_DEMO.md) · [Architecture](ARCHITECTURE.md) ·
[Contract API](docs/reference/CONTRACT_API.md) ·
[Security review](docs/security/CONTRACT_REVIEW.md) ·
[Findings and fixes](docs/security/FINDINGS_AND_FIXES.md) ·
[Documentation index](docs/README.md)

## Why batch auctions

Private credit, property interests, and structured assets often trade only
occasionally. A fixed auction window collects funded interest without requiring
buyers and sellers to be online together. A uniform clearing result gives
participants a common execution price and leaves a public historical observation.

The registry provides verifiable auction outcomes. It does not continuously
revalue an asset or certify fair value. Collateral integrations need their own
source, execution-volume, freshness, and manipulation policy.

## Features and scope

| Component or feature | Current status |
|---|---|
| Issuer-authorized factory and immutable auction instances | Implemented |
| Sales with escrowed RWA inventory | Implemented |
| Buybacks with escrowed quote budget | Implemented |
| Funded open bids and public price-tick aggregates | Implemented |
| Funded sealed commitments, reveal verification, non-reveal refunds | Implemented |
| Uniform-price clearing and pro-rata marginal allocation | Implemented |
| SAC compliance at admission, atomic finalization of every price contributor, and delivery | Implemented |
| Independent delivery, same-owner credits, withdrawals, and deadline expiry | Implemented |
| Verified public clearing and actual-execution registry | Implemented |
| Optional bounded non-reveal penalty on Sealed auctions | Implemented; default zero, issuer-configured up to 50% |
| Read-only, partial SEP-40 price adapter (`base`/`decimals`/`resolution`/`lastprice`) | Implemented |
| Local compiled-WASM demonstration and regression suite | Implemented |
| Client SDK, dApp, indexer/API, hosted keeper services | Architecture specified; not implemented |
| Separately reviewed curated oracle feed (source/manipulation governance) and lending/vault integrations | Architecture boundary specified; separate implementation/review required |

The Rust Soroban SDK is the contract framework dependency. No application/client
SDK is included in the PoC. There are no protocol trading fees, live upgrades,
privileged rescue functions, arbitrary refund recipients, AMM pools, or bridges.

## Quick start

Prerequisites:

| Tool | Version / purpose |
|---|---|
| Rust through Rustup | **1.94.1**, selected by `rust-toolchain.toml` |
| Rust target | `wasm32v1-none` |
| Rust components | Rustfmt and Clippy |
| Soroban SDK | **25.3.2**, pinned in `Cargo.toml` and resolved in `Cargo.lock` |
| Python | 3.10+ for design models and repository checks |
| Node.js | 18+ for independent BigInt/XDR checks; CI selects Node 22 |

From the repository root:

```bash
rustup show
rustup component add rustfmt clippy
rustup target add wasm32v1-none
./scripts/demo.sh
./scripts/check.sh
```

A fresh build needs dependency access. Cached environments can set
`CARGO_NET_OFFLINE=true`. The local demonstration does not need Stellar CLI, a
wallet, an RPC endpoint, testnet funds, or deployment credentials. Native host
compilation is the largest first-build step; Cargo jobs are limited to one in
`.cargo/config.toml` to reduce peak memory.

## Demonstration

```bash
./scripts/demo.sh
```

The script builds all four WASM contracts and runs three asserted scenarios
against genuine Stellar Asset Contracts in Soroban's local test host:

1. **Open sale:** an issuer escrows 10 RWA. Alice bids 6 at 9 and Bob bids 8 at 7.
   The clearing price is 7; Alice receives 6 RWA for 42 quote, Bob receives 4 for
   28, and they withdraw unused deposits. The registry exposes the price and
   actual 10-RWA / 70-quote execution.
2. **Sealed buyback:** the issuer escrows 100 quote. Valid reveals of 4 RWA at 8
   and 6 at 10 both execute at 10. An altered reveal is rejected. A non-revealer
   withdraws its complete 3-RWA deposit.
3. **Compliance:** a winner revoked after the preliminary sweep is excluded by
   the atomic finalizer. Clearing restarts without its demand, and its full
   deposit remains recoverable.

The displayed sale/buyback amounts use whole tokens encoded with seven decimals.
The [demo guide](docs/POC_DEMO.md) explains assertions and local-host boundaries.
These are executable contract demonstrations, not signed public-network trades.

## Protocol lifecycle

1. **Create.** The current RWA SAC administrator authorizes an immutable auction
   configuration through `AuctionFactory`. Creation deploys the escrow but moves
   no assets.
2. **Authorize and fund.** Asset administrators authorize the escrow address on
   both SACs. Before the start ledger, the issuer deposits the exact sale inventory
   or buyback budget. Funding verifies deployment provenance and current authority.
3. **Bid.** Every bidder needs authorization on both assets. Open sale bids lock
   their maximum quote obligation; buyback bids lock RWA quantity. Sealed bidders
   lock a declared maximum and submit a domain-bound commitment.
4. **Reveal and close.** Sealed bids reveal inside their own window. Permissionless
   bounded calls compute the candidate and sweep price-contributing bids. Finalization checks
   every price-contributing bid atomically, including marginal bids rounded to
   zero; definite revocations permanently exclude bids and restart clearing before
   the closing deadline.
5. **Deliver or refund.** Claims check authorization again. Successful trades
   deliver assets; undelivered trades retain full bidder refund backing. Sale
   proceeds become issuer credit only after the corresponding RWA delivery.
6. **Withdraw and publish.** Credit owners withdraw their own original asset.
   Anyone can publish verified immutable clearing/execution observations. After
   deadlines, permissionless expiry preserves refunds rather than forfeiting them.

Creation and funding are separate because an `AUTH_REQUIRED` asset cannot be sent
to a new escrow until its asset administrator authorizes that address.

## Economics and compliance

Amounts are token base units, prices are fixed-point values with `S = 10^7`, and
both supported assets have seven decimals. Intermediate multiplication/division
uses U256 with checked i128 results.

- Sales rank bids from highest price downward. Oversubscribed books ration the
  marginal price tick; undersubscribed books fill at the reserve if actual integer
  volume meets the minimum. Deposits/payments use `ceil(quantity × price / S)`.
- Buybacks rank asks from lowest upward and reprice the entire accepted prefix at
  the uniform price. Payments use `floor(quantity × price / S)`. Total quote paid
  cannot exceed budget; sparse asks and rounding can leave unused funds.
- Equal-price marginal allocations round down pro rata. Report actual filled
  volume separately from the nominal candidate; dust and zero-value fills can
  cause a minimum-fill cancellation. The nominal marginal price can survive zero
  individual marginal fills; every contributing bid must still be authorized.
- Three compliance checks cover admission, atomic clearing, and delivery. An
  unavailable authorization query aborts; it is not treated as definite revocation.

See [clearing mathematics](ARCHITECTURE.md#7-clearing-mathematics) and
[settlement accounting](ARCHITECTURE.md#9-settlement-refunds-and-conservation)
for exact rules and per-asset liability equations.

## Checks and test evidence

```bash
./scripts/check.sh
```

The check script runs formatting, locked release WASM builds, Rust tests, Clippy
with warnings denied, design tests, independent XDR and clearing-vector checks,
and local documentation-link checks. Before individual integration runs, build
the WASMs and their source/artifact manifest. Tests load and verify those files
from Cargo's configured target directory; `CARGO_TARGET_DIR` is supported.

```bash
python3 scripts/wasm_artifacts.py build
cargo test --workspace --locked -- --test-threads=1
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -m unittest discover -s tests/design -v
node tools/design/check_commitment.mjs
python3 tools/design/generate_contract_vectors.py --check
```

| Evidence | Coverage |
|---|---|
| 60 compiled-WASM integration tests | Genuine SAC transfers, scoped authorization, sale/buyback, commit/reveal, the optional non-reveal penalty, price-contributor compliance, wide amounts, rollback, credits, deficits, expiry, factory bootstrap/namespace/registration, financial TTL, registry ordering, and the OracleAdapter's SEP-40 read surface |
| 48 contract clearing vectors within that suite | Independent BigInt expected values, both modes, one-price and 65-tick grids, fractional prices, varied chunk sizes, minimum failures, reverse claims, and complete liability withdrawal |
| 9 shared Rust unit tests | Wide arithmetic, configuration/TTL/grid bounds, penalty/allocation rules, canonical XDR vectors, and public wire-type equivalence |
| 50 design tests | 10,000 generated books, 6 extreme-i128 books, 1,000 generated accounting lifecycles, and targeted regressions |
| Independent encoders/reference | Node BigInt economic reference and 36 fixed canonical ScVal-XDR commitment vectors covering all 22 Config fields and all commitment components |
| Resource regressions | Maximum Sale/Buyback and late-exclusion finalizers fit local invocation limits, with ledger/event/return/key/entry byte measurements; network qualification remains open |
| 15 tooling tests | Coverage report integrity, exclusions, raw-contract thresholds, stale diagnostics, failure cleanup, WASM identity/source checks, custom targets, and artifact packaging |

The 10,000-book comparison is model evidence, not 10,000 contract executions.
Most authorization tests use exact mock invocation trees. Cryptographic regressions
exercise real Ed25519 classic-account/custom-account credentials, signed-call
mutation, nonce/expiry, replay, and authentication callback re-entry rollback.
Two further cases verify real classic-admin creation/funding and 2-of-2 multisig
weight, membership, sorting, duplication, and rollback rules.
These run in the local host and do not submit signed network transactions. See
the [cryptographic review](docs/security/CRYPTOGRAPHIC_REVIEW.md).
The separate [coverage workflow](docs/TESTING.md) measures
native executable source lines and enforces at least 95% production coverage in
every contract source. Raw LLVM totals and the separate SDK test-helper policy
remain visible. No branch/path completeness, formal proof, or independent audit
is claimed. [The contract review](docs/security/CONTRACT_REVIEW.md) records findings,
measured resources, dependency review, and remaining evidence. Review fixes
include hidden-quantity overflow reservation before Sealed Sale deposits,
oracle instance/code maintenance, issued-SAC base validation, and a 24 October
2026 fix for a non-reveal-penalty timing bypass (the penalty could previously
be dodged by simply letting the delivery deadline pass unresolved). Penalty
reason precedence, 4096-tick repeated exclusions, expired snapshot recovery,
registry pair isolation, and publication failure isolation have direct
regressions. The coverage runner also rejects incomplete/mismatched reports
and prevents reuse of older passing summaries after a failed run. The
[consolidated findings and fixes report](docs/security/FINDINGS_AND_FIXES.md)
gathers every finding from every review pass, with a plain-language
explanation of each defect and its fix, in one place.

The [auction logic review](docs/security/AUCTION_LOGIC_REVIEW.md) explains clearing
priority, budget/collateral bounds, rounding, price-contributor authorization,
and per-asset settlement conservation.

## Source coverage

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
./scripts/coverage.sh
```

The gate checks **each contract and the shared implementation separately** against
95% production executable source-line coverage. Test fixtures and precisely
identified SDK-generated host-only bridge regions are outside that production
metric; raw LLVM totals and every such region are retained. Reports are written
to `target/coverage/`; GitHub CI retains HTML/JSON as `contract-coverage`.
Native instrumentation complements the mandatory all-WASM suite and does not
measure guest instruction coverage. [Testing methodology](docs/TESTING.md) explains
the native deployment adapter, exclusions, and remaining test boundaries.

Measured raw LLVM line coverage: **Auction 97.13%**, **Factory 97.53%**,
**Registry 98.76%**, and **OracleAdapter 97.83%** (100% of production lines).
Each deployable contract must independently remain at or above 95%. The shared
library reaches 99.62% for production lines; its raw total is 92.25% including
SDK test-only bridge attributes, disclosed in the testing guide.

## Limits and security

- **16 total bids per auction**, including non-reveals. The earlier 32-bid draft
  exceeded the pinned SDK's default footprint guard. Larger capacity requires
  reviewed changes and measurements.
- **Issued SAC assets only.** Native XLM and arbitrary custom tokens are excluded
  on both sides. Delegated RWA SAC administration is supported.
- **Asset-admin powers remain.** Freezing can delay delivery/refund withdrawals;
  clawback can create a deficit. The contracts preserve liabilities and block
  outflows while insolvent, but cannot replace missing assets.
- **Credits are owned funds.** A credited refund is not a completed transfer. It
  stays with the same owner and asset, and has no protocol forfeiture deadline.
  Keeping/restoring archived records still requires someone to pay storage costs.
- **Financial retention has a defined horizon.** Auction construction protects
  instance and reused code through delivery plus 120,960 recovery ledgers; rolling
  maintenance targets 518,400 ledgers within runtime limits. Factory/registry
  dependencies have separate maintenance/restoration requirements.
- **Sealed metadata is public.** Deposits, identities, timing, and counts can leak
  information. By default non-reveal carries no penalty; an issuer may instead
  configure a bounded penalty (up to 50%) on never-revealed bids specifically —
  losing a salt still excludes the bid from clearing either way.
  Auction cancellation refunds in full; on a cleared auction, the `NonReveal`
  penalty applies before and after delivery expiry, including when the owner
  loses authorization. Revealed expired bids refund fully. See architecture §6.3.
- **Historical prices need interpretation.** Allocation can exceed actual delivery,
  related accounts can influence a price, and observations can become stale.
- **Permissionless work still needs a payer.** Closing, expiry, maintenance,
  restoration, and publication do not happen without transactions.

Report vulnerabilities through [SECURITY.md](SECURITY.md). Production work still
includes multisig/wallet and additional account adversaries, network archival
restoration, broader stateful comparisons, full resource qualification, and
independent review. [Release gates](docs/security/DESIGN_AUDIT.md#5-open-implementation-and-deployment-gates)
track the broader requirements.

## Repository layout

```text
.github/                   # CI, dependency updates, issue/PR templates
.cargo/                    # build settings and cargo wasm alias
contracts/
  common/                  # shared wire types, checked math, SAC/commitment helpers
    src/test.rs            # direct unit tests for validation/arithmetic/commitment helpers
  auction/                 # escrow, clearing, compliance, delivery, credits
    tests/lifecycle.rs     # integration-test module registration and shared imports
    tests/scenarios/       # behavior, authorization, demo, and resource regressions
    tests/support/         # shared fixture, verified WASM loader, bootstrap/coverage adapters
  auction-factory/         # issuer-authorized deployment and discovery
  price-registry/          # verified clearing and execution observations
  oracle-adapter/          # read-only, partial SEP-40 surface over the registry
deployments/
  testnet.json             # current Testnet contract IDs, WASM hashes, and deploy salt
  TESTNET_TRANSACTIONS.md  # dated log of every on-chain Testnet transaction, with hashes
docs/
  README.md                # documentation index
  POC_DEMO.md              # reproducible local demonstration
  TESTING.md               # suite layers, counts, and coverage methodology
  reference/               # current contract API
  operations/              # build, release, maintenance, incident runbooks
  security/                # design/contract/auction-logic/cryptographic reviews,
                            # release gates, and FINDINGS_AND_FIXES.md (every
                            # finding across all passes, in plain language)
scripts/                   # checks, demo, coverage, verified WASM build/packaging
tests/
  design/                  # economic/accounting model tests
  tooling/                 # adversarial coverage-gate and WASM artifact checks
  vectors/                 # canonical commitment/domain and independent clearing fixtures
tools/design/              # independent models, encoders, fixture generation
ARCHITECTURE.md             # full product technical architecture
Cargo.toml / Cargo.lock    # pinned contract workspace and dependencies
rust-toolchain.toml         # compiler and WASM target
LICENSE                    # Apache License, Version 2.0
CONTRIBUTING.md
SECURITY.md
CHANGELOG.md
```

`contracts/auction/tests/scenarios/` groups the integration suite by behavior:
clearing, accounting, authorization, commitments, lifecycle, factory, registry,
demonstration, and independent vectors, alongside focused cryptographic, classic
authority, oracle, penalty, resource, and recovery regressions. Shared setup and
artifact loading live in `support/`. The native coverage adapter remains an
explicit test-only helper in that directory. All scenarios run through the same
`lifecycle` test binary; existing test-name filters remain usable.

Planned SDK/application packages are documented in the architecture; empty package
scaffolds are intentionally absent. Funding proposals and Word documents are
excluded from this repository. Local editor/tool state directories are excluded
in `.gitignore`.

## Contributing and license

Read [CONTRIBUTING.md](CONTRIBUTING.md) before changing economic rules, authority,
wire types, commitments, or storage. Include regression evidence and synchronize
the architecture with behavior. The GitHub workflow runs the same local check
script; it does not deploy contracts.

Licensed under the [Apache License, Version 2.0](LICENSE). Crossing is open
infrastructure: a permissive license with an explicit patent grant lets
wallets, exchanges, indexers, and other protocols integrate or build on the
contracts without a separate grant from the authors, and the patent clause
specifically protects that integration work. Every crate in the workspace
declares `license.workspace = true`, resolving to the same Apache-2.0 term.
