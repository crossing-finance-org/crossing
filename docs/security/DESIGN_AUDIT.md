# Crossing Protocol — Design Audit and Second Review

Date: **22 October 2026**
Target: [architecture v1.6](../../ARCHITECTURE.md), contract baseline `0.1.0`
Scope: repository documentation, auction economics, trust boundaries, contract
interfaces, storage, compliance, accounting, publication, and executable design
models. This is an internal design review, not an independent audit of deployed
or compiled contracts. Core contracts were subsequently implemented locally and
deployed to Stellar Testnet; [the contract review](CONTRACT_REVIEW.md) records
that separate phase, and [deployments/](../../deployments/) records the
Testnet evidence. No Mainnet deployment or external audit has occurred.

## Result and meaning of closure

The original specification was not ready to implement safely. Its buyback
arithmetic, refund assumptions, compliance snapshot, and issuer withdrawals could
produce incorrect prices, failed refunds, or insufficient backing. The revised
design specifies corrections for every finding below. The second review adds
exact constructor/ABI contracts, mutation ordering, accounting deltas, recovery
paths, byte-size gates, and remaining external risks.

**“Design corrected” means a concrete normative correction is in the architecture.**
It does not mean the Rust implementation is proven correct. Model evidence
is distinguished from contract/runtime evidence. No mainnet readiness, formal
verification or independent approval is claimed. Subsequent source-coverage
measurements are recorded separately in the contract review and testing guide.

All 35 original findings have design corrections. Subsequent contract findings
C-01–C-18, their fixes, and runtime regressions are recorded in the
[contract review](CONTRACT_REVIEW.md) and [auction logic review](AUCTION_LOGIC_REVIEW.md).
Broader contract qualification, resource sizing, and application/oracle work remain open gates
in §5. External asset powers and economic manipulation remain explicit risks in
§6 rather than being mislabeled eliminated vulnerabilities.

## 1. Reviewed inputs and method

Reviewed `README.md`, the original `ARCHITECTURE.md`, and historical funding
material held outside the published repository. The revised architecture governs
technical behavior.
Competitor, market-size, legal, funding-eligibility, and partner claims are not
certified by this technical review.

Review work included manual adversarial examples, per-asset liability equations,
state/authority/ownership tracing, sparse-price buyback cases, post-clearing
revocation/default analysis, asynchronous sweep races, integer granularity,
public-observation integrity, and primary Stellar documentation verification.
A second pass specifically checked implementability and details absent from the
first high-level description.

Artifacts used for executable evidence:

- [Python mathematical/accounting model](../../tools/design/auction_model.py)
- [Independent JavaScript/BigInt arithmetic reference](../../tools/design/reference.mjs)
- [Design regression and generated tests](../../tests/design/test_auction_model.py)

The design model intentionally abstracts signatures, actual SAC calls, canonical
XDR, TTL, network limits, and rollback. It cannot establish those properties.
Subsequent compiled-WASM integration tests and a separate XDR vector provide
partial runtime evidence, documented in the contract review; the release gates
below still require their full acceptance criteria.

## 2. First-review findings and corrections

All entries in this table are **design corrected**. Severity describes the
potential impact of implementing the original design literally.

| ID | Severity | Defect / concrete failure | Correction and evidence |
|---|---|---|---|
| D-01 | Critical | Buyback sums offers at their own ask costs even though all accepted quantities must be paid the final price. Uniform repricing can exceed the budget. | §7.2 uses capacity `floor(B*S/p)` for the entire accepted prefix; floor quote payments. Sparse-gap and repricing regressions plus independent reference. |
| D-02 | Critical | Issuer can claim proceeds before a later winner revokes/defaults; a full bidder refund may then be underfunded. | §9/§19 retain every reversible trade reserve. Sale proceeds accrue only after delivery; Buyback reserves exact promised payments. Issuer-first withdrawal tests. |
| D-03 | High | Revoked SAC holders are assumed able to receive immediate token refunds. SAC transfer restrictions also apply to refunds. | §9/§20 create same-owner, same-asset credits; failed withdrawal preserves them. Frozen-refund isolation and expiry tests. |
| D-04 | High | Multi-transaction winner sweep is treated as a single current-authorization snapshot. An early checked winner can revoke before the last page. | §8.2 requires final atomic checks of all price contributors, including marginal bids rounded to zero, and a disclosed 16-bid cap. Model and compiled-WASM revocation-after-prepare regressions cover the rule; broader target-network resource fit remains a gate. |
| D-05 | High | Original placement compliance exempts sale investors and confuses which side supplies RWA. | §3/§6/§8 require every bidder's RWA and quote authorization in both modes. Sale-investor rejection regression. |
| D-06 | High | Calling a token's `authorized` method is treated as proof it is a SAC; arbitrary WASM can lie or implement malicious transfer behavior. | §2 requires built-in SAC executable verification; native XLM/custom tokens excluded in V1. Genuine/native/fake-token compiled-WASM regressions now cover this boundary. |
| D-07 | High | “Issuer opens auction” has no concrete enforceable creation authority. | §3/§4 require current RWA SAC admin equality/signature at create and fund. Runtime regressions reject wrong/missing/mutated authorization and transferred admins; broader classic/custom-account flows remain open. |
| D-08 | High | Deployment/initialization is raceable or assumes the new escrow can receive AUTH_REQUIRED assets before its address is authorized. | §4 uses constructor deployment, dormant create, SAC authorization, then one-time funding. Real bootstrap/pin failures, same-salt rollback/retry, constructor reinvocation, funding, and unfunded expiry are tested locally. |
| D-09 | High | Sealed commitment is only `hash(price,amount,salt)`, permitting ambiguous encoding and missing owner/network/auction/terms/deposit binding. | §6.2 specifies canonical ScVal XDR with domain, owner, nonce, config hash, and deposit binding. Rust/Node golden vectors and runtime mutation/copy/replay/underfunding tests pass; all 22 Config fields and all ten commitment components have independent fixed vectors; runtime network/config/deposit mutations pass. Future client conformance remains a gate. |
| D-10 | High | Non-revealed and excess sealed collateral lack complete exactly-once liability/return rules. | §6/§9/§19 track every full deposit, consume slots at commit, retain excess until resolution, refund original asset. Sealed excess/non-reveal/replay tests. |
| D-11 | High | Re-clearing is “bounded” by revocations in a call, but the algorithm may restart without a sound persisted work/termination measure. | §7.4/§8.3 specify epoch resets, complete accumulators, permanent exclusions, finite passes, deadlines. Local WASM covers 65-tick continuation, multiple work limits, revocation restarts, and expiry; 4096-tick/all-16-exclusion continuation now passes in 2,200 calls. Network cost qualification remains open. |
| D-12 | High | `i128` multiplication, cumulative counters, and ceiling additions may overflow before division. | §3/§7 wide intermediates, checked casts/sums, quotient/remainder ceiling. Overflow and maximum-i128 reference vectors. |
| D-13 | High | Conservation adds token and stablecoin amounts, mixes dimensions, and assumes equality despite unsolicited transfers. | §9.4/§19 use separate asset liabilities, credits, pending escrow and reserves; surplus is not issuer credit. Generated lifecycle and donation tests. |
| D-14 | High | Token administrators can revoke escrow or claw it back, contradicting an unconditional “no settlement risk” promise. | §2/§9/§20 state asset powers, preserve liabilities, and block insolvent outflows. Genuine SAC clawback, suspension, bookkeeping, and replenishment are covered by runtime tests; external asset powers remain. |
| D-15 | High | Indefinite closing or missing claimants can strand issuer backing and deposits without a terminal path. | §5/§9 separate closing and delivery deadlines; permissionless cancellation and bid expiry preserve credits forever. Boundary/default/global-freeze tests. |
| D-16 | Medium | Sale undersubscription assigns cumulative demand as marginal quantity, potentially double-counting better ticks; reserve and `limit_price` can conflict. | §3 removes redundant limit; §7.1 full ratio at reserve for partial sales. Partial-sale regression. |
| D-17 | Medium | Pro-rata target is used as actual volume/minimum although integer allocation can be smaller. | §7.3 computes actual F after rounding/zero-value filter before minimum; owns all dust explicitly. Actual-minimum/dust regressions. |
| D-18 | Medium | Bid amount changes meaning between modes; price scale/asset decimals and payment rounding are not fully defined. | §3 defines RWA quantity always, seven decimals, quote scale, deposits, and asymmetric payment rounding. Fixed-point and generated-book tests. |
| D-19 | Medium | TTL is an absolute-ledger promise without max-horizon, relative semantics, code/instance maintenance, or restoration policy. | §12/§21 define excluding-current TTL, forced initial instance/code horizon, bounded maintenance, and persistent ownership. Actual live-until and aged shared-code regressions pass after C-07/C-08; real archival restoration remains open. |
| D-20 | Medium | Global maps, growing bidder vectors, and an unspecified winner index can exceed storage/transaction bounds. | §10/§17 use separate persistent entries, dense bid IDs, bounded counters, and ID pagination. Serialized-size and native storage tests remain required. |
| D-21 | High | Registry provenance is underspecified and price writes can be forged if a caller supplies claimed metadata. | §11/§17 pin factory/template executables and pull config/results, cross-check registration, deny caller-supplied price. Runtime tests reject fake publications and unregistered clones. |
| D-22 | Medium | Keying price only by RWA mixes different settlement assets; delayed publication can replace newer marks and manufacture freshness. | §11 keys full pair, orders by source ledger/raw contract ID, separates source/publication time. Runtime tests cover delayed and duplicate records, same-ledger ties in both orders, zero execution, and reversed-pair reads; distinct quote-pair isolation now passes; network dependency restoration remains open. |
| D-23 | Medium | Inline registry failure could revert closing and block all claims. | §4/§11 use independent retriable publication from immutable state. Injected publication failure leaves both quote-pair deliveries usable; publication retries without changing source time. Network dependency restoration remains open. |
| D-24 | High | Allocated price/volume is treated as executed oracle data even after post-clearing refunds or related-account demand. | §11 separates clearing/execution, records zero execution explicitly, forbids raw feed as automatic collateral input. Curated adapter is separately scoped/reviewed. |
| D-25 | Medium | “Sealed” overstates confidentiality and non-reveal incentives; “SEP-compatible” is claimed without the actual interface. | §6 states metadata/withholding risks and optional bounded penalty precedence; §11 documents an implemented partial raw SEP-40 adapter. Client and curated-oracle acceptance remain separate. |
| D-26 | Medium | Claim effects and two-leg Buyback failures have no precise atomic/rollback semantics. | §9/§18/§19 specify actual delivery before earned proceeds, fixed recipients, effects/rollback, and typed outcomes. Genuine SAC second-leg failure rolls back the first leg/accounting; retry and issuer-first claim-order regressions pass. |

## 3. Second-review findings and corrections

These additional findings are also **design corrected** in v1.1.

| ID | Severity | Detail missed / failure | Correction and evidence |
|---|---|---|---|
| S-01 | High | Buyback recipient can equal escrow. A self-transfer could be counted as delivery while acquired RWA remains inaccessible as surplus. | §3.3 rejects auction/factory/registry recipients and protocol-address bidders; verify actual deployed address at funding. Model recipient/address regression. |
| S-02 | High | Budget and low price individually fit i128 but maximum affordable quantity does not; a funded auction may fail every clearing attempt. | §3.3 bounds `floor(B*S/min_price)` at validation. Overflow-capacity regression and maximum-i128 books. |
| S-03 | Medium | All nominal marginal fills may round to zero individually. Describing price as an individually delivered last bid becomes false. | §7.3 explicitly uses nominal aggregate marginal price before indivisible rounding; actual F/C reported separately. Both-mode zero-marginal-delivery regressions. |
| S-04 | High | Fixed entry counts do not bound serialized response size; returning full pages of configs/bids can exceed metadata limits. | §17.2 returns bounded IDs/addresses and one-record getters; §21 benchmarks key/entry/return/event/envelope bytes. Runtime byte evidence remains required. |
| S-05 | High | “Pin factory/registry hashes” leaves constructor dependency/circular deployment verification unspecified. | §17.1 defines hash arguments, precomputed factory, mutual bindings, registered funding, and salt preimage. Real local deployment, all four pin mismatches, atomic retry, and namespace tests pass; network release acceptance remains open. |
| S-06 | Medium | A time-derived phase may be mistaken for completed cancellation/refund accounting, and exactly-once initialization is not explicit enough. | §17.4 separates persisted stage, effective bidding phase, and expiry flags; §19 defines terminal-init and per-bid guards. Deadline/replay model tests. |
| S-07 | Medium | Public methods lack exact bounded results, persistent outcome fields, delta equations, and auth trees, leaving independent implementations inconsistent. | §17–§19 and the current API reference match implemented types, storage, outcomes, and auth. Runtime/encoding tests cover current wire records and scoped calls; complete external-client conformance remains future work. |
| S-08 | Medium | Generic “fail closed” might permanently exclude a valid bidder after a query/restore/resource error, or an exclusion error might roll back its intended progress. | §8/§18 distinguish definite false, transient error, and successful `Excluded` progress. Fault-injected SAC/host tests required. |
| S-09 | Medium | Capacity/resource tradeoffs and the cost of maximum repeated clearing are hidden behind O(ticks) language. | §8/§12/§21 disclose O(bids) validation, 16-slot baseline and maximum 2,227 continuation calls; no throughput/fee guarantee without benchmarks. |

No assertion is made that an internal second pass finds every possible future
vulnerability. All identified defects have explicit design treatment; remaining
unknown implementation behavior is represented honestly by the gates below.

## 4. Executable verification

Run from the repository root with Python 3.10+ and Node.js 18+:

```bash
python3 -m unittest discover -s tests/design -v
```

Current suite: **50 tests**. It includes:

- **10,000 deterministic generated books**, seed `0xC20551`, exact price,
  allocation, and payment equality against the separate BigInt reference.
- **6 extreme i128 books** against the same independent reference, exercising
  products that cannot be evaluated safely in i128 before division.
- **1,000 generated lifecycle sequences**, seed `0x51E771E`, with revocation,
  issuer-first withdrawals, shuffled claims, and deadline refunds; both asset
  balances and liabilities reconcile and all owned funds can be drained after
  authorization recovery in the no-clawback/no-donation cases.
- Regression vectors for sparse Buyback gaps, uniform repricing, ceiling/floor
  asymmetry, nominal versus actual minimum, indivisible marginal rounding,
  replay, capacity, non-reveal, excess collateral, frozen credits, failed trade
  legs, expiry boundaries, unsolicited surplus, and deficit detection.

Node is intentionally required for the comparison; the test does not silently
skip the independent reference when it is unavailable. Generated mismatches report
seed, case index, and complete input book for reproduction. Python uses exact
integers as a numerical oracle, not fixed-width machine behavior.

Model limitations: no cryptographic signature verification, canonical ScVal/XDR,
SAC executable query, native account/trustline rules, actual host rollback,
re-entry, real continuation storage, price-registry execution, TTL, CPU or byte
measurement. Abstract model success does not close those implementation gates.

## 5. Open implementation and deployment gates

These are release acceptance gates, not a list of wholly unimplemented tests.
Local evidence partially satisfies G-01–G-08. No row is fully closed by the
current PoC; planned application/oracle work applies when those components ship.
See [testing](../TESTING.md) for measured source coverage and execution boundaries.

| Gate | Current local evidence | Still required for full acceptance |
|---|---|---|
| G-01 Toolchain/build identity | Rust 1.94.1, SDK 25.3.2, WASM target/lockfile pinned; four release artifacts; cached dependency review disclosed. | Fresh advisories, independent artifact reproduction, tested deployment CLI/network manifest. |
| G-02 SAC identity/authority | Genuine issued SACs; native/fake assets rejected; exact create/fund authority and bootstrap pins; real classic current-admin create/fund in both modes, master-key and 2-of-2 multisig/trustline credentials, and custom Ed25519 credentials. | Network account/trustline setup, broader signer/transfer limits, and wallet/custom-account authentication on the target network. |
| G-03 Canonical commitments | Rust/Node legacy fixture and 36 independent fixed cases cover every Config and commitment component, exact i128/address encoding; runtime network/config/deposit mutation and replay regressions. | Future SDK conformance, independent release-artifact review and client secret-management acceptance. Local encoding/domain regression gaps are closed for the current schema. |
| G-04 Runtime financial atomicity | SAC/WASM two-leg rollback, reserves, credits, deficits, expiry, exact auth trees; authenticated custom-account re-entry rollback/retry, classic multisig child-failure nonce rollback; penalty precedence and wide-value conservation. | Additional adversarial accounts and complete classic trustline/transfer failure matrix on the target network. |
| G-05 Resources/capacity | Local 16-owner Sale/Buyback finalizer and late-exclusion measurements, canonical bytes, and default invocation limiter; 4096-tick/all-16-exclusion case finishes in 2,200 advance calls against a 2,227 bound. | All heavy branches/account types, serialized bytes/envelopes, restoration/rent and margins under target-network limits. See the contract review for measured counters. |
| G-06 TTL/restoration | Exact maximum TTL, actual instance/code retention, aged auction/adapter code and permissionless maintenance; expired snapshot preserves credits/nonces/metadata/duplicate guards and permits draining refunds. | Target-network restoration submission, RPC/footprint/rent/dependency acceptance. Local recording-mode auto-restoration does not establish these. |
| G-07 Stateful contract equivalence | 48 guest clearing vectors, wide amounts and 60 integration tests; 9 shared unit tests; 15 tooling tests validate evidence integrity. 10,000 books/1,000 lifecycles are model comparisons. Per-contract source coverage is documented separately. | 10,000+ actual guest/stateful cases, broader sequences and branch/path exploration. Maximum-grid repeated exclusions now have a direct regression. |
| G-08 Registry | Forgery/clones rejected; ordering/ties/source time, duplicate/zero execution, two quote-pair isolation, publication failure with continued delivery, and snapshot duplicate-guard recovery. | Broader network outages and restored history/pointer/provenance dependency combinations. |
| G-09 Application safety | Full SDK/dApp/indexer/keeper architecture; no application implementation. | Implement/test BigInt/XDR/signing, manifest verification, encrypted recovery, credited-versus-paid UI and offline access. |
| G-10 Curated oracle | Raw partial SEP-40 adapter implemented; source-time/zero-execution/base/TTL regressions. Governed feed is separately specified. | Curated source/volume/freshness/manipulation policy, governance/security review, any required remaining SEP-40 methods and consumer integration. |
| G-11 Release governance | Build/deployment/recovery runbooks and known limits documented; software license selected ([Apache-2.0](../../LICENSE)). | Verified release manifest, pilot asset controls and appropriate transferability review. |
| G-12 External review | Separate internal scope reviews; confirmed fixes retested; cryptographic evidence documented. | External contract audit/remediation, testnet pilot and signed deployment verification before valuable/mainnet inventory. |

## 6. Explicit residual risks and chosen product tradeoffs

- **External asset controls:** issuer/quote admins can freeze or claw back.
  Credits preserve ownership but cannot force a forbidden transfer or replace a
  lost asset. Deficits suspend outflows; external replenishment is required.
- **Initial capacity:** 16 total slots, including sealed non-reveals. This supports
  the atomic closing guarantee, not unrestricted thousands of independent bidders.
  Authorized slot occupation and Sybil participation remain possible.
- **Manipulation:** issuer authority and related bidders can manufacture a price.
  The raw registry certifies execution provenance, not fair value or independence.
- **Privacy/withholding:** deposits remain public. Non-reveal is unpenalized by
  default; an optional penalty up to 50% applies only to a cleared auction's
  `NonReveal` resolution before or after delivery expiry. Auction cancellation
  takes precedence and refunds in full; delivery expiry alone does not waive it.
  Lost secrets prevent participation; refunds follow the configured resolution rule.
- **Discrete economics:** gaps and per-bid integer rounding leave unused budget or
  inventory. Nominal marginal price need not equal an individual delivered ask.
- **Liveness cost:** someone must pay continuation, expiry, storage and restoration
  fees. The worst-case recomputation bound is finite but potentially expensive.
- **Delivery option:** a winner can delay or abandon delivery until its deadline;
  no penalty is imposed, and actual execution may be below allocated volume.
- **Immutability:** no privileged rescue or live upgrade exists. Bugs in eventual
  deployed code cannot be fixed by changing this document.

These constraints must appear in product/integration material where relevant.
They cannot be honestly marked fixed by a smart-contract implementation alone.
