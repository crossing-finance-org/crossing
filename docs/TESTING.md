# Smart-contract testing and coverage

The project uses three complementary layers: compiled-WASM behavior/resource
tests, instrumented native Soroban source coverage, and independent economic
models. Each serves a different purpose; no single percentage replaces the other
evidence or independent security review.

## Run the suites

```bash
./scripts/check.sh
./scripts/demo.sh

# Install these once for source coverage:
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
./scripts/coverage.sh
```

For focused Cargo runs, first use `python3 scripts/wasm_artifacts.py build`.
It builds release WASMs into Cargo's configured target directory and records
their hashes, sizes, current contract source/build-input hashes, compiler versions,
source revision, and whether the working tree is dirty. The integration loader
rejects missing manifests, changed build inputs, or changed WASM bytes. A plain
`cargo wasm` still builds contracts, but does not refresh the test manifest.
Use the verified build command after contract changes.

`CARGO_TARGET_DIR` is supported, including paths with spaces. The scripts set
`CROSSING_WASM_DIR` to that release directory before native coverage changes its
own build target. Direct Cargo runs resolve the configured target automatically;
an explicit `CROSSING_WASM_DIR` can select an already verified artifact set.
Artifacts are loaded once per test process, with the loaded bytes hashed again.
The manifest is a local consistency check, not a signed release attestation.

The coverage script produces raw LLVM `target/coverage/coverage.json`, an HTML
report under `target/coverage/html/`, and `production-summary.json`. Its gate
requires **95% production executable source-line coverage per file**. The auction,
factory, registry, oracle adapter, and shared implementation are assessed separately; a high
workspace average cannot conceal a low-coverage contract. Missing source records
also fail. Integration fixtures and `common/src/test.rs` are excluded.
Empty counters, duplicate source records, missing production function mappings,
invalid source regions, and LLVM profile/source mismatch diagnostics also fail.
The runner removes previous reports before building, retains export/HTML stderr,
and removes merged profiles in reuse mode so a run without fresh raw profiles
cannot fall back to an earlier passing result. Instrumented output is confined
to `target/llvm-cov-target/`.

The pinned SDK additionally generates **host-test-only `TryFrom<&Record> for
ScVal` bridges** at `#[contracttype]` attributes. They are not compiled into release
WASM. LLVM can list their attribute locations as zero-counter regions even when
the public wire-encoding tests call the bridges. The gate separates these from
production lines while retaining the unmodified raw report. An exclusion must
match the generated ScVal bridge symbol, occupy only a `#[contracttype]` line,
have a zero counter, and overlap no production function region. No handwritten
logic, error branch, guest converter, or complete production source file is
omitted. The policy is tied to SDK 25.3.2 and fails for an unreviewed SDK pin change.

`production-summary.json` lists every omitted attribute location and generated
symbol alongside raw and production totals. The four deployable contracts must
also exceed 95% in the **unadjusted raw LLVM report** as a second mandatory gate.
The shared library's raw total includes twenty-one SDK test-only attribute regions.
Review both metrics.

Coverage means LLVM source-line coverage. It is not a claim of 95% branch/path
coverage, exhaustive input exploration, or a proof that exercised assertions
detect every financial or authorization defect. Read the JSON/HTML reports to
inspect uncovered defensive/error paths and region/function statistics.

For a local iteration with the same cached instrumentation/toolchain,
`./scripts/coverage.sh --reuse-build` clears prior profiles and reruns both modes
without deleting compiled dependencies. The default command cleans the workspace
coverage build first. Use the default clean run for final evidence after source,
test, or toolchain changes: retained objects with older source maps can produce
LLVM mismatched-function warnings and invalid line denominators. Discard those
percentages and regenerate cleanly; do not relax the gate to accept them.
Both commands apply the same raw/production gates and neither replaces the
default all-WASM behavioral run.

### Measured results — 24 October 2026

| Implementation | Raw LLVM lines | Raw coverage | Production coverage |
|---|---:|---:|---:|
| Auction | 946 / 974 | **97.13%** | 97.33% |
| AuctionFactory | 158 / 162 | **97.53%** | 98.14% |
| PriceRegistry | 239 / 242 | **98.76%** | 99.17% |
| OracleAdapter | 45 / 46 | **97.83%** | **100.00%** |
| Shared library | 262 / 284 | 92.25% | **99.62%** |

The shared library is not a deployable contract. Its production denominator is
263 lines after the twenty-one SDK test-only attribute regions above. The same
policy identifies two such regions in Auction and one each in
Factory/Registry/OracleAdapter; their unadjusted totals independently satisfy
the 95% contract requirement. Generated reports are the authority when sources
or toolchains change.

The suites contain **60 integration tests**, executed with all four WASM
contracts and repeated for native source measurement, plus **9 shared unit tests**
and **50 design tests**. Repeating a test in multiple execution modes is not
counted as a new independent test. Missing-contract, under-covered-contract, and
unclassified-region negative checks confirm that the coverage gate rejects them.
There are also **15 tooling tests**: nine for malformed/missing/duplicate coverage,
SDK exclusion boundaries, missing function maps, stale diagnostics, and failed
runner cleanup, and six for custom artifact targets, stale source/lockfile inputs,
altered/missing WASMs, failed builds, and artifact packaging. Their synthetic
reports and isolated fake Cargo failure path test the gate; they do not measure
contract execution or add contract coverage.
The OracleAdapter is registered as a native contract in `services`/`auction`
coverage modes, the same way the factory and registry are, so its source is
measured like every other deployable contract rather than only exercised as an
opaque compiled-WASM call.

## Compiled-WASM evidence

`scripts/check.sh` builds the current four release artifacts before executing
the integration suite. In the default test mode, all four contracts run as WASM
against genuine built-in issued SACs. Tests cover exact authorization trees,
deposits, clearing, real token delivery, claim order, refunds/withdrawals,
revocation, rollback, deficits, deadlines, provenance, registry ordering, and
bounded maintenance/pagination. Resource measurements run only in this mode.

Factory regressions deploy registry and factory through real `deploy_v2` host
calls, reject all four mismatched bootstrap pins, block constructor reinvocation,
and retry the original deployment salts after failure. Creation tests require
authorization of the exact configuration/salt, recheck current issuer authority,
and verify issuer/network/factory salt isolation. A test-only counter overflow
injected after deployment proves that the child auction and registration roll
back together; it is not a realistically reachable counter-exhaustion scenario.
Financial TTL regressions exercise the exact supported maximum and a reused
auction WASM entry aged above the rolling extension threshold but below a new
auction's delivery horizon. Registry same-ledger ties are checked against raw
contract-ID bytes in both publication orders.

Three cryptographic regressions use real Ed25519 signatures for classic-account
master-key and custom-account credentials against exact root/SAC invocation
trees. They reject forged signatures, changed arguments/subcalls, mutated
network/nonce/expiration, expired credentials, and replay; successful retries
prove failed authorization does not consume the nonce or change escrow.
An authentication callback attempts re-entry, aborts, and then reuses the same
valid credential successfully after disarming. See the
[cryptographic review](security/CRYPTOGRAPHIC_REVIEW.md) for scope and limits.
A fourth regression tests runtime commitment-domain mutations with exact mock
authorization trees; those mocks do not verify signatures.

Two further real-signature regressions exercise a classic current SAC
administrator creating and funding both modes with exact Config/salt/transfer
authorization, plus genuine 2-of-2 classic multisig bidding and withdrawal.
Insufficient/disabled master-key weight, empty/duplicate/unsorted/outsider
signatures, expiry, replay, and failed child authorization cannot change escrow.
Transferring the admin after creation blocks funding; restoring it permits the
same unconsumed signed credential to retry. Accounts, trustlines, and signer
weights are seeded in the local host; network setup and wallet UX remain separate.

Additional regressions cover conservative Sealed Sale reveal-capacity
reservation, penalty rounding/precedence in both modes, failed withdrawals,
genuine adapter bootstrap failures and aged-code maintenance, and raw oracle
source time before/after zero execution. A 4096-tick, 16-exclusion stress case
uses 2,200 successful advance calls against the 2,227 bound and drains refunds.
Two quote assets for one RWA retain isolated histories/latest pointers, while
an injected registry publication failure leaves both deliveries usable.

An expired snapshot test reconstructs the environment after all saved entries'
live-until ledgers and the delivery/recovery horizon. Credits, bid nonces,
factory metadata, resolution guards, and registry duplicate guards survive;
expiry, publication retry, and owner withdrawals still drain liabilities. This
uses the pinned host's recording-mode automatic persistent restoration, not
network restoration submission, RPC acceptance, rent, or footprint measurement.

Auction logic regressions revoke marginal contributors whose individual
quantity/payment rounds to zero, before sweep and after `Ready`, and require
exclusion, recomputed prices, and full original refunds. A zero-payment buyback
case becomes deliverable after exclusion. Very large fractional-price books in
both modes exercise U256 intermediates beyond i128, pro-rata floors, reverse
claims, and complete credit withdrawal. See the
[auction logic review](security/AUCTION_LOGIC_REVIEW.md) for invariants and limits.

Focused reproduction commands, after `python3 scripts/wasm_artifacts.py build`:

```bash
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked factory_ -- --test-threads=1
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked revoked_zero -- --test-threads=1
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked financial_horizon -- --test-threads=1
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked reused_auction_wasm -- --test-threads=1
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked wide_amounts -- --test-threads=1
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked cryptographic_tests -- --test-threads=1
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked runtime_gap_tests -- --test-threads=1
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked classic_authority_tests -- --test-threads=1
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked resource_audit_tests -- --nocapture --test-threads=1
python3 -m unittest discover -s tests/tooling -v
```

Tests and [bootstrap helper](../contracts/auction/tests/support/factory_bootstrap.rs)
are registered by the [integration suite](../contracts/auction/tests/lifecycle.rs).
Behavior tests live in `contracts/auction/tests/scenarios/`; shared fixtures,
verified artifact loading, and deployment/coverage adapters live in `support/`.
This organization retains one test binary and the existing assertions and names.
Setting
an empty test mode explicitly selects release WASM, even if the shell previously
used a native coverage mode.

Forty-eight deterministic books, seed `0xA0D17`, are generated independently of the
contracts' clearing code. Expected results come from the JavaScript BigInt
reference. These execute against the contracts, including fractional prices,
65-tick continuation, multiple work limits, minimum failures, reverse claims, and
draining every liability through the correct owner/asset. `--check` regeneration
rejects stale or edited expected fixtures.

## Native measurement and its boundary

LLVM source instrumentation cannot count Rust source execution inside the WASM
guest. The coverage runner therefore runs the same integration assertions twice
in native Soroban mode:

1. `CROSSING_TEST_MODE=services`: production factory and registry Rust execute
   natively; the real factory deploys the actual auction WASM through its normal
   production deployment path. This covers constructor, deployment, registration,
   discovery, and publication source behavior.
2. `CROSSING_TEST_MODE=auction`: production auction Rust and registry execute
   natively. A test-only factory adapter performs the native auction installation
   step, because the SDK's on-chain WASM deployer cannot install a native function
   table. Other factory calls delegate to production code. This isolates source
   measurement of auction logic while keeping its actual validation, authority,
   liabilities, transfer, and error behavior intact.

Native SDK contracts have a special executable marker (`SHA256(empty)`) rather
than distinct release WASM hashes. Native tests cannot establish real artifact
identity or guest instruction cost. That is why the default all-WASM suite remains
mandatory and source coverage never disables production provenance checks.
The adapter exists only in `contracts/auction/tests/support/`; it is absent from
release WASM. Test modes do not introduce test-only paths into contract source.

Bootstrap pin tests always execute real factory/registry WASM. Shared-auction
code aging and the post-deployment counter fault run in default and `services`
modes; they skip the native-auction adapter because its synthetic executable and
installation path cannot establish those properties. The resource regression
also remains specific to the default all-WASM execution mode.
Adapter invalid-base deployment is checked through real WASM `deploy_v2` in
every mode. A paired, direct native constructor-validation call measures the
`Asset` rejection branch without pretending a reserved constructor is publicly
callable; the same regression verifies public constructor reinvocation is denied.
Expired snapshot recovery always uses actual WASM because snapshots cannot
serialize native test function tables.

## Independent models and vectors

The 50 Python design tests compare 10,000 seeded books and 6 extreme-i128 books
against a separate BigInt algorithm and run 1,000 accounting lifecycles. This
validates economic rules independently of contract implementation, but does not
execute those 10,000 books on-chain. The canonical-XDR fixture is checked by a
manual Node encoder and the Rust Soroban encoder. The legacy fixture plus a
36-case fixed matrix cover Buyback and contract owners, 2^64/i128 maximum
amounts, all 22 Config fields, and all ten commitment components. Expected
hashes originate in the independent Node encoder, not the production Rust code.

The resource follow-up measures a maximum prorated Buyback and an exclusion of
the final bid after the preliminary sweep. Both stay within local CPU/memory/
footprint ceilings and complete delivery/refunds with zero remaining liabilities.
Their write/event bytes come from the pinned estimator; return XDR is serialized
separately, and maximum ledger key/entry sizes sample auction/SAC contract data.
See the [resource tables](security/CONTRACT_REVIEW.md#resource-measurement).
These are not serialized network envelopes, restoration costs, or classic-account
maximum-book measurements.

## Continuous integration

GitHub Actions is configured to run checks, the working demonstration, and the 95% coverage gate.
It retains coverage JSON/HTML as `contract-coverage`. The same scripts are usable
offline after dependencies/tools are cached. Hosted workflow execution is
separate from local verification; the repository does not deploy contracts.

After all checks pass, CI also uploads `tested-contracts-<commit>` for 14 days:
the four verified WASMs, `SHA256SUMS`, `test-wasm-manifest.json`, `Cargo.lock`,
the workspace manifest, and the Rust toolchain file. Local packaging uses
`python3 scripts/wasm_artifacts.py package`; it verifies inputs/hashes again
and writes `<Cargo target directory>/verified-contracts/`.

The [contract review](security/CONTRACT_REVIEW.md) records measured results and
remaining qualification gaps such as archival restoration, broader classic
trustline/custom-account scenarios, and target-network resource limits.
