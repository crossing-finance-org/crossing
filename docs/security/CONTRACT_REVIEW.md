# Crossing Protocol — Internal Contract Review

Date: **22 October 2026**
Scope: the current local smart-contract PoC, its compiled WASM, exact authorization
test trees, design models, and [architecture v1.6](../../ARCHITECTURE.md), source `0.1.0`.
This is an internal development review. The reviewed contracts have since
been deployed to and exercised on Stellar Testnet (see
[deployments/](../../deployments/)); no Mainnet deployment or external audit
has occurred. The evidence below does not establish production readiness.

## Implemented behavior

- [Shared types and arithmetic](../../contracts/common/src/lib.rs): checked U256
  intermediate math, bounded configuration, genuine SAC verification, immutable
  config hashing, domain-bound commitments, and allocation rules.
- [Auction](../../contracts/auction/src/lib.rs): dormant creation, separate funding,
  funded open/sealed bids, bounded clearing, preliminary and atomic authorization
  checks, permanent exclusion/restarts, delivery reserves, independent claims,
  same-owner credits, expiry, solvency checks, and typed TTL maintenance.
- [Factory](../../contracts/auction-factory/src/lib.rs): current RWA SAC admin
  authority, immutable constructor deployment, salt binding, provenance, discovery,
  and mutual factory/registry code bindings.
- [Registry](../../contracts/price-registry/src/lib.rs): retriable pull publication,
  factory/template/config verification, pair histories, source-ordered latest
  observations, and separate allocated/actual execution records.
- [OracleAdapter](../../contracts/oracle-adapter/src/lib.rs): fixed issued-SAC
  base, pinned immutable registry, partial raw SEP-40 reads, and TTL maintenance.

The current PoC has no client SDK, UI, privileged rescue, live
upgrade, or arbitrary refund recipient. Future integration architecture does not
mean those components have been implemented.

## Findings resolved during implementation

| ID | Finding | Implemented correction and evidence |
|---|---|---|
| C-01 | The original 32-bid finalizer exceeds the pinned SDK's default transaction footprint guard: 103 entries against a limit of 100 in the tested layout. CPU alone does not establish executability. | Lower the hard/configuration cap to 16, including non-reveals. Synchronize models, vectors, and architecture. The compiled-WASM maximum-winner test passes the enabled default invocation resource limiter and explicit CPU/memory/footprint assertions. |
| C-02 | Returning raw token errors through the protocol error type can mislabel a SAC error as an unrelated protocol variant. A two-leg failure also needs real rollback evidence. | Normalize failed token transfers to `Error::Transfer` and propagate failure from the root invocation. A genuine SAC second-leg balance-overflow failure rolls back the first RWA transfer, resolution, and accounting; retry succeeds after capacity is restored. |
| C-03 | The pinned SDK's native-test conversion for `Option<custom contract type>` prevented the same persisted types from compiling consistently across test and guest builds. | Use explicit `CandidateValue`, `ReasonValue`, and `ResolutionValue` enums in both builds. Generated WASM contract specifications describe the encoding; tests use those same artifacts. This is an implementation compatibility issue, not a demonstrated loss-of-funds exploit. |
| C-04 | Reconciliation needs cumulative successful withdrawals and direct getters for published design fields. | Track per-asset withdrawal totals atomically with credits, balances, and liabilities. Expose tick totals, cancellation reason, and pair history count. The sale demonstration withdraws bidder refunds and earned issuer proceeds, then reconciles token balances and liabilities. |
| C-05 | Medium: validation accepted native XLM as the quote SAC because executable identity and seven decimals also describe native XLM. This violated V1’s issued-asset boundary. | Require the issued-SAC admin interface on the quote asset as well. A genuine native-SAC regression fails before the fix (quote side accepted) and passes after it; both RWA and quote positions reject native XLM with no factory registration. |
| C-06 | Assurance gap: most generated economic checks ran against models rather than deployed guest code; commitment mutation, overlapping expiry, and delayed registry publication had limited runtime evidence. | Add 48 independently generated BigInt clearing fixtures executed against compiled WASM, mutation/copy/replay/underfunding tests, overlapping expiry batches, and source-ordered/zero-execution registry tests. This narrows test gaps rather than proving exhaustive coverage. |
| C-07 | Medium: a fresh auction instance could reuse an older shared WASM code entry whose TTL exceeded the half-retention extension threshold but ended before the newly accepted delivery/recovery horizon. A compiled-WASM reproduction showed instance TTL 518,400 but code TTL 300,000 for an auction with delivery 350,000 ledgers away and a required recovery horizon of 470,960. | Force both instance and shared-code retention to the complete financial horizon during construction, using `threshold = extend_to`. Preserve half-horizon rolling extensions afterward. An aged-code regression creates and funds a second auction and checks actual instance/code live-until values against its absolute financial target. Funds were not deleted; the defect introduced an unexpected restoration requirement before delivery. |
| C-08 | Low: `financial_ttl` added one ledger to a relative horizon even though SDK 25.3.2 already defines TTL as live-until minus the current ledger. The exact supported maximum was rejected. | Remove the extra ledger. A runtime regression accepts the exact maximum, verifies actual instance/code TTL after creation/funding, and rejects maximum plus one. This is a scheduling correctness issue, with no demonstrated financial exposure. |
| C-09 | Assurance gap: factory coverage did not directly establish all bootstrap pin failures, signed-argument binding, create-time admin changes, namespace isolation, or rollback after a successful child deployment followed by a factory error. | Add real `deploy_v2` bootstrap tests for all four pin mismatches with same-salt retry and constructor reinvocation rejection; exact-auth/config/salt failures; transferred-admin rejection; issuer/network/factory salt separation; and post-deployment counter-fault rollback. The counter fault is injected test state, not a realistically reachable exploit. Add registry same-ledger ordering checks in both publication orders. |
| C-10 | Medium: marginal bids whose quantity or payment rounds to zero can still influence the uniform price, but the original sweep/finalizer skipped their authorization checks. Revoked marginal demand could therefore reprice authorized winners or prevent a deliverable book from clearing. | Check both SAC authorizations for every revealed, nonexcluded bid at or better than the candidate price, in both sweep and atomic finalization. Definite revocation subtracts its full quantity and restarts; query failures still abort. Sale/buyback regressions revoke zero-fill marginal owners before sweep and after `Ready`, assert exclusion/repricing/full refunds, and cover a zero-payment buyback becoming deliverable after exclusion. The original regression fails against the previous compiled WASM. |

These corrections supplement the [35 design findings](DESIGN_AUDIT.md); they are
not a claim that all possible contract defects have been discovered. Frozen
credits, asset clawback, participant concentration, and stale/manipulated auction
prices remain disclosed trust/economic limits, not eliminated vulnerabilities.
All C-01–C-10 corrections above are implemented; there are no outstanding
confirmed code findings from this review pass. Remaining qualification work is
listed separately below and in the design release-gate matrix.

### Additional review by contract scope

Separate review passes examined the factory, auction, and registry/shared library
from source and test behavior. The factory pass covered bootstrap authority,
immutable code pins, issuer authorization, salt construction, registration
atomicity, discovery, and retention. It found no additional exploitable factory
defect. The auction and shared-library passes identified C-07 and C-08 above;
additional economic review identified C-10. The registry pass found no
forged-publication or ordering defect; its same-ledger
tie rule now has direct runtime evidence. These remain internal development
reviews, not an independent external audit or certification.

Factory and registry retention is intentionally independent of an auction's
financial retention. Long-lived source metadata/history may require maintenance
or restoration before funding or publication. Claims on an already funded
auction do not call the factory or registry. The auction's initial instance/code
horizon guarantee is distinct from these operational dependencies; C-07 fixes
the former and does not claim to remove the latter.

## Reproducible local evidence

```bash
./scripts/demo.sh
./scripts/check.sh
```

Both build the current WASM before the integration tests embed it. Toolchain:
Rust 1.94.1, Soroban SDK 25.3.2, `wasm32v1-none`, locked Cargo dependencies.

| Check | Evidence |
|---|---|
| WASM build | All four deployable contracts compile with the release profile and overflow checks. Artifact hashes and sizes must be taken from the built files; see the operations runbook. |
| Contract tests | **60 integration tests** execute compiled WASM against genuine built-in SACs. **9 shared unit tests** check wide math/overflow, configuration/TTL/grid bounds, penalty and anti-dust allocation rules, canonical config/commitment vectors, and public wire-type equivalence. |
| Core demonstration | Three passing scenarios assert open-sale funding, public tick totals, single-price fills, actual token delivery/refund withdrawals, verified public registry records, sealed buyback reveal validation, non-reveal refunds, and late revocation exclusion. |
| Adversarial runtime cases | Two-leg rollback/retry, post-clearing revocation and full refund after issuer withdrawal, frozen-credit failure preservation, deadline refunds, clawback deficit suspension/replenishment, dormant/closing expiry, failed authorization, fake tokens/publications, unregistered clones, and SAC escrow authorization before funding. |
| Contract/reference equivalence | 48 deterministic seed `0xA0D17` fixtures, regenerated/checked against the independent BigInt reference, execute both modes with fractional prices, 65-tick persisted walks, one-price books, varying work limits, minimum failures, reverse claims, and complete credit withdrawal. All final asset liabilities drain to zero. |
| Encoding | Rust and a separate Node canonical ScVal-XDR encoder agree on the legacy fixture and a 36-case fixed matrix covering all 22 Config fields, all ten commitment components, contract owners, Buyback, and 2^64/i128 maximum amounts. |
| Design suite | **50 tests**, including 10,000 generated books plus 6 extreme-i128 books compared with an independent BigInt reference, and 1,000 generated accounting lifecycles. These are model comparisons, not 10,000 contract invocations. |
| Source coverage | A separate native Soroban/LLVM runner enforces at least 95% production executable-line coverage in every contract source, including shared code. Raw LLVM totals are retained. The manifest identifies SDK-generated host-test-only ScVal bridge attribute regions separately; no handwritten production code is omitted. See [testing methodology](../TESTING.md), including the native deployment adapter. |
| Static/build checks | Rustfmt check and Clippy across workspace/all targets with warnings denied. |

Focused evidence is in [lifecycle.rs](../../contracts/auction/tests/lifecycle.rs):

| Area | Regression entry points |
|---|---|
| Factory bootstrap and registration | `factory_real_bootstrap_rejects_mismatched_pins_and_retries_atomically`, `factory_post_deployment_counter_fault_rolls_back_contract_and_registration` |
| Factory authority and namespace | `factory_creation_requires_authorization_of_exact_config_and_salt`, `factory_creation_rechecks_asset_admin_and_failed_attempt_preserves_salt`, `factory_salts_are_scoped_to_issuer_network_and_factory` |
| Financial TTL | `financial_horizon_accepts_exact_maximum_ttl_and_rejects_one_beyond`, `reused_auction_wasm_covers_each_new_auctions_financial_horizon` |
| Price-contributor compliance | `revoked_zero_fill_marginal_demand_is_removed_before_price_is_fixed`, `revoked_zero_payment_bid_cannot_force_an_otherwise_deliverable_book_to_cancel` |
| Wide arithmetic in guest settlement | `wide_amounts_execute_fractional_price_prorata_and_drain_both_liabilities` |
| Registry tie ordering | `registry_same_ledger_ties_use_raw_contract_id_in_both_publication_orders` |

The [testing guide](../TESTING.md) provides filtered reproduction commands,
raw/production coverage totals, and the native-adapter boundary. The
[auction logic review](AUCTION_LOGIC_REVIEW.md) explains financial invariants.

Most integration authorization uses specific `MockAuth` invocation/sub-invocation
trees, with unapproved calls rejected and no blanket `mock_all_auths` approval.
Three additional regressions construct real signed Soroban authorization entries:
classic-account master-key credentials with actual account/trustline entries,
custom-account Ed25519 credentials, and an authenticated callback re-entry
attempt with rollback/retry. A fourth checks runtime commitment-domain mutations
using exact mock trees. These establish local host behavior, not wallet UX,
exhaustive multisig coverage, signed transaction envelopes, or public-network submission.
See the [cryptographic review](CRYPTOGRAPHIC_REVIEW.md).

### Resource measurement

The historical 22 October local release build produced Auction 92,348 bytes, Factory
39,854 bytes, PriceRegistry 46,343 bytes, and OracleAdapter 21,374 bytes.
The post-C-19 Auction artifact is 92,325 bytes; the other three sizes are unchanged.
These are local release WASM sizes before any separate optimization step;
independent reproduction and deployed release identities remain separate checks. The operations runbook
shows how to calculate hashes from the actual files.

`bounded_finalizer_fits_measured_resource_limits` creates 16 distinct funded
winners and invokes the compiled-WASM finalizer after preparing the candidate.
For the current sale fixture, the SDK invocation estimator reports:

| Resource | Measured | Explicit test ceiling |
|---|---:|---:|
| Modeled instructions | 18,354,195 | less than 80,000,000 |
| Modeled memory bytes | 2,436,426 | less than 32,000,000 |
| Ledger footprint entries | 55 | at most 100 |

The SDK's default per-invocation resource limiter remains enabled, including
write/read, footprint, entry-size, and event limits. The global test budget is
unlimited because it also meters authorization observation; the test asserts
the actual invocation's separate resource measurements. It also checks instance
and shared-code live-until coverage through the configured recovery horizon.

These are local modeled costs for one branch, using the limits bundled with the
pinned SDK. They are not current network configuration, an envelope/fee estimate,
or a worst-case proof across classic trustlines, revocation, restoration, and all
closing/delivery branches. No network capacity or throughput guarantee is made.

Additional compiled-WASM measurements use 16 distinct contract-address bidders:

| Finalizer branch | Modeled instructions | Memory bytes | Conservative footprint entries |
|---|---:|---:|---:|
| Prorated Buyback, budget 33, actual payment 32, issuer remainder credit 1 | 18,371,672 | 2,455,028 | 58 |
| Sale, revoke bid 15 after the sweep; full scan then successful exclusion/restart | 18,398,742 | 2,471,908 | 58 |

| Branch | Written ledger bytes | Event bytes | Return ScVal XDR bytes | Maximum sampled key / entry bytes |
|---|---:|---:|---:|---:|
| Prorated Buyback | 3,176 | 460 | 400 | 152 / 2,992 |
| Late exclusion | 2,908 | 116 | 44 | 120 / 2,424 |

The host estimator measures invocation writes/events; the typed return is
serialized separately because that estimator excludes return size. Maximum key
and entry sizes are canonical LedgerKey/LedgerEntry encodings sampled from the
auction and both SACs' contract-data state, a superset rather than an exact
transaction footprint. Both fixtures report zero disk-read bytes in this
preloaded local host; this is not a network disk-read estimate. All assertions
retain the default invocation limiter, CPU below 80 million, memory below
32 million, and conservative footprint at most 100. Both tests complete claims,
owned withdrawals, and zero liabilities after the measured call. Transaction
envelopes, classic-account finalizer resources, restoration, and target-network
configuration remain separate qualification work.

### Dependency review

A cached `cargo audit --no-fetch` review was run against the
available local RustSec snapshot dated **3 October 2026** (`ef6173cb`). It reported
no known vulnerabilities and two unmaintained-package warnings in the resolved
host/test dependency tree: `derivative` 2.2.0 (`RUSTSEC-2024-0388`) and `paste` 1.0.15
(`RUSTSEC-2024-0436`). This is cached evidence, not a fresh online advisory check.
Review current upstream advisories and guest/host dependency reachability before
a public release; do not treat an unmaintained warning as a patched vulnerability.

## Addendum — 21 October 2026: OracleAdapter and the optional non-reveal penalty

Two scope additions reviewed after the pass above, against the same local
evidence standard. Both are implemented, tested, and covered by the same 95%
per-contract coverage gate; neither changes any finding C-01–C-10 above.

| ID | Addition | Review considerations and evidence |
|---|---|---|
| C-11 | Fourth deployable `OracleAdapter` (architecture §11.3). | Raw `base`/`decimals`/`resolution`/`lastprice` surface; holds no funds, changes no auction accounting, and has no freshness/manipulation policy. Constructor fixes bindings and pins the registry once, relying on the pinned registry's absence of upgrades. Explicitly partial SEP-40: `assets`, `price`, and `prices` are absent. Read, bootstrap, source-time, and retention regressions cover the adapter in WASM and native measurement modes. C-15/C-16 below strengthen retention and base validation; `maintain()` now renews TTL. |
| C-12 | Optional non-reveal penalty (`Config.nonreveal_penalty_bps`, architecture §6.3). | Zero by default, at most 50%, forced zero in Open mode. Only a cleared auction's pre-deadline `NonReveal` resolution splits the original deposit into bidder/issuer credits; cancellation and deadline expiry refund it fully. An unrevealed owner's revocation does not change `NonReveal`; a revealed bid resolved `Revoked` refunds fully. Collection depends on permissionless claims occurring before expiry. Both-mode reason-precedence, floor/maximum/wide-value arithmetic, failed withdrawals, and interleaved issuer/bidder withdrawals preserve separate asset liabilities. The new 22nd Config field changes canonical encoding even at zero; older client payloads are incompatible. |

**Historical timing policy:** the C-12 row records the 21 October review. C-19
below superseded its expiry rule on 24 October: on a cleared auction, a
`NonReveal` penalty applies before and after delivery expiry. Auction
cancellation still refunds fully. The 25 October follow-up aligned the Python
lifecycle model, its regression expectations, and active policy summaries with
that existing contract behavior; no deployed contract code changed.

The penalty addition required updating the shared `Config`'s canonical XDR encoding
(`nonreveal_penalty_bps` is a new field). The Rust/Node canonical-XDR golden
vectors (`tests/vectors/commitment-v1.json`, `common/src/test.rs`) were
regenerated from the independently computed, cross-checked hash — not hand-edited
— and `canonical_xdr_agrees_with_independent_javascript_encoder` continues to
pass, so Rust and the independent JavaScript encoder still agree byte for byte
on the new schema.

### C-13 — Resource optimization: redundant `Config` reads in `store`/`leave`

Low severity, no behavior change. The auction contract's internal `store` and
`leave` helpers each independently called `cfg(env)` — a full instance-storage
read and deserialization of the 22-field `Config`, including four `Address`
host-object lookups — even though every call site already held a `Config`
reference from earlier in the same invocation. `admit` alone (one call per
`place_bid`/`commit_bid`) calls `store` up to four times, so a single bid
placement paid for that redundant read three or four times over, and every
mutating entrypoint paid for it at least once more via `leave`. Both helpers
now take the caller's `&Config` directly instead of re-reading it. This is a
pure refactor — identical TTL arithmetic, identical storage keys, identical
error conditions. The current full-suite and coverage evidence includes this
refactor. It matters more than a
typical micro-optimization because this contract already operates close to
Soroban's per-transaction resource ceiling — that ceiling is the reason
`MAX_BIDS` is 16 rather than higher — so avoiding wasted CPU in the hottest
paths (every bid placement, every mutating call's exit) is directly relevant
to that headroom, not merely cosmetic.

## Addendum — 22 October 2026: defensive re-review and cryptographic evidence

Separate source reviews covered auction/commitments, factory/adapter, and
registry/accounting. All confirmed findings below have local corrections and
regressions. No external audit or network qualification is claimed.

| ID | Finding | Correction and evidence |
|---|---|---|
| C-14 | Medium: Sealed Sale collateral totals could fit i128 while hidden quantities overflowed the admitted-quantity sum. An otherwise valid, funded reveal could then fail and later incur the optional non-reveal penalty. | Before accepting collateral, bound `floor(total_admitted_deposits*SCALE/min_price)` to i128 using U256. Every valid revealed quantity is bounded by its deposit/price, so the sum fits regardless of reveal order. The conservative bound counts excess collateral. The old compiled WASM accepted the unsafe second commitment; the regression now rejects it before transfer/nonce/slot writes, retries the same nonce with supported collateral, and reveals near-limit quantities in both orders. |
| C-15 | Low: the raw adapter had no TTL-maintenance entry point and used a rolling threshold on construction, allowing reused code to expire earlier than a fresh instance. | Force instance/code to the supported retention target at construction; add permissionless `maintain()` for rolling renewal. Real aged shared-WASM and native/WASM maintenance regressions assert actual TTLs and unchanged bindings. Registry retention remains separate. |
| C-16 | Low: adapter construction accepted native XLM as a base although this release's auctions exclude it, leaving a valid-looking adapter for an unsupported quote asset. | Require a genuine seven-decimal issued SAC with a working admin interface. Wrong registry pins, fake bases, and genuine native SAC bases fail atomic deployment; retrying the same salt with a supported base succeeds. |
| C-17 | Informational: the registry tie-break comment implied factory provenance made raw-address ordering ungameable. Issuers can predict/grind salts. | Clarify that raw contract-ID order is a deterministic arbitrary tie-break with no economic-quality or independence guarantee. Ordering behavior is unchanged. |

Further completed regressions establish penalty reason precedence, wide-value
flooring and frozen-credit preservation in both modes; real classic/custom
Ed25519 credentials and auth-callback rollback; a 36-case independent canonical
commitment matrix; 2,200 bounded calls for 4096 ticks/all 16 exclusions; expired
snapshot ownership/replay recovery; and quote-pair/publication-failure isolation.
The [cryptographic review](CRYPTOGRAPHIC_REVIEW.md) and
[testing guide](../TESTING.md) define the precise evidence boundaries.

### Follow-up — classic authorities, heavy branches, and evidence integrity

Two additional real-auth regressions verify classic current-SAC-admin create and
fund in both modes, including exact Config/salt/transfer binding and retry after
admin restoration; and 2-of-2 classic bidding/refund withdrawal with master-key
weight, membership, ordering, duplicates, expiration, replay, and child-failure
nonce rollback. Ledger accounts/trustlines/signers are seeded locally. This does
not establish network account setup, wallet signing, every multisig arrangement,
or resource fit of classic trustlines in a maximum-size finalizer.

| ID | Finding | Correction and evidence |
|---|---|---|
| C-18 | Assurance integrity: the coverage checker accepted a source with zero lines and no function map as 100%, and allowed a later duplicate file record to overwrite an undercovered one. Missing production function maps could also pass positive counters. Cached merged profiles could be reused if a reuse run produced no raw profiles. | Require positive integer counters bounded by source size, unique source records, and production function mappings; reject invalid regions and LLVM mismatch diagnostics. Clear prior reports before starting and merged data in reuse mode, retain reporter diagnostics, and keep both 95% gates unchanged. Nine tooling regressions cover acceptance/rejection and isolated failed-run cleanup. Real complete LLVM exports continue to pass. This affects test evidence, not deployed contract execution or the previously validated complete coverage measurements. |

The two additional resource regressions above cover a full Buyback finalizer and
the late-exclusion branch, with actual canonical bytes and complete recovery.
The total at this review pass was 60 integration tests, 9 shared Rust unit tests, 50 design
tests, and 9 tooling tests; execution modes do not count as additional tests.

## Addendum — 24 October 2026: independent re-audit, penalty timing bypass

A fresh three-agent re-audit (auction/common, the newer test modules, and
factory/registry/oracle-adapter plus documentation consistency) ran against
the state above, deliberately not trusting the prior passes' conclusions. It
confirmed every prior correction still holds and found one new, real issue.

| ID | Finding | Correction and evidence |
|---|---|---|
| C-19 | Medium: the non-reveal penalty could be deterministically bypassed by timing. `resolve()` classified the caller-supplied `expired` flag as `DeliveryExpired` ahead of `NonReveal`, so a sealed bid that never revealed only needed to go unresolved past `delivery_deadline` — forcing resolution through the permissionless `expire_bid`/`expire_range` path instead of `claim_bid` — to receive a full refund with no penalty applied, regardless of who called the resolving entrypoint or why. A rational non-revealing bidder's best strategy was simply to wait; the existing test `nonreveal_penalty_reason_precedence_matches_terminal_refund_policy` and `ARCHITECTURE.md` §6.3 had both already encoded this as intended behavior ("the penalty is not guaranteed to be collected"), which is precisely why an independent re-audit — not reasoning from the existing tests/docs as ground truth — was needed to catch it. | Reclassify so `NonReveal` is checked ahead of the timing-derived `expired` label on any auction that actually `Cleared`: an unrevealed bid is `NonReveal` whether resolved via `claim_bid` before the deadline or `expire_bid`/`expire_range` after it. `Stage::Cancelled` still takes precedence and still refunds in full — an auction that never cleared never had its price distorted by the non-reveal, so there is nothing to deter after the fact. The existing test was updated to assert the corrected precedence (penalty now applies identically whether resolved before or after the deadline); `ARCHITECTURE.md` §6.3 no longer describes the penalty as uncollectible by waiting. Money conservation is unaffected — this changes which `Reason` and refund split apply, not whether `refund + issuer credit == deposit`. |

This is the one finding from this pass that changed production behavior, not
just documentation or test coverage; every other re-audited area (the
`store`/`leave` `Config`-parameter refactor, the `fund()` window guard, the
50% penalty cap, provenance pinning across all three smaller contracts, and
the "real Ed25519/classic-multisig, not mocked" claims in the cryptographic
test modules) was independently re-verified and confirmed correct as-is.

## Remaining evidence and scope

The [design release gates G-01–G-12](DESIGN_AUDIT.md#5-open-implementation-and-deployment-gates)
remain a broader qualification checklist. Local evidence partially addresses
G-01 through G-08. That matrix distinguishes completed local checks from the
remaining full acceptance criteria. Outstanding contract work includes:

- Network setup and wallet integration for classic issuers/multisig, broader
  signer/trustline/transfer arrangements, and additional custom-account adversaries.
- Actual network archival/restoration submission and fees/footprints. Expired
  local snapshots preserve credits, nonces, metadata, and duplicate guards using
  recording-mode automatic restoration, which does not establish network acceptance.
- All resource-heavy branches and serialized transaction/return/event sizes on
  the target network, including buyback, exclusions, and restored entries.
- Broad stateful contract/reference comparisons, additional branch/path exploration, independent
  review, and public-network pilot/deployment verification.
- Broader registry dependency-restoration/network outage scenarios. Local tests
  cover separate quote pairs, injected publication failure with continued claims,
  same-ledger ties, delayed/duplicate publication, zero execution, and source time.

Client/application safety (G-09) applies to future components; it is outside
the current smart-contract PoC. Curated-oracle design (G-10) remains future
work for the separately governed feed in architecture §11.3 specifically — the
partial, raw `OracleAdapter` surface (C-11 above) is implemented, but adds no
source/manipulation governance and should not be mistaken for that curated
layer. The project is licensed under [Apache-2.0](../../LICENSE). Release
documentation, external asset controls, and independent audit requirements
still apply before a production release.

The [demo guide](../POC_DEMO.md) provides the review entry point. The architecture
states the intended invariants; this review records what has actually been tested.
