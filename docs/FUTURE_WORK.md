# Future work register

This register preserves every unresolved item needed to take Crossing from the
current contract PoC to the full project specified in
[ARCHITECTURE.md](../ARCHITECTURE.md). It does not duplicate the finding
history in [security/FINDINGS_AND_FIXES.md](security/FINDINGS_AND_FIXES.md)
(every finding there already has an applied fix) or the release-gate detail in
[security/DESIGN_AUDIT.md §5](security/DESIGN_AUDIT.md#5-open-implementation-and-deployment-gates)
and [§6](security/DESIGN_AUDIT.md#6-explicit-residual-risks-and-chosen-product-tradeoffs)
(summarized here with a stable ID and cross-reference back to that detail).

Status values: **Planned** (scoped, not started), **Research** (needs a design
decision before it can be scoped), **Blocked** (depends on another item or an
external actor), **Deferred** (explicitly out of scope for the initial auction
product). An item being open here is not a defect in shipped code: every
closed finding is tracked separately in
[FINDINGS_AND_FIXES.md](security/FINDINGS_AND_FIXES.md).

## 1. Core architecture

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-101 | Stand up the application workspace (`sdk/`, `dapp/`, `services/`, `packages/`, `db/`, `infra/`, `releases/`) | High | Planned | Repository structure |
| FW-102 | Published release manifest format and signing | High | Planned | Release governance |

**FW-101.** *Description:* [ARCHITECTURE.md §15.1](../ARCHITECTURE.md#151-target-repository-additions)
defines the target package layout (SDK, dApp, indexer, API, keeper, notification,
onboarding services, database migrations, infrastructure config, immutable
release manifests, end-to-end/fuzz test trees) alongside the existing Rust
workspace. None of these paths exist yet. *Recommended approach:* add them
incrementally per [§15.2](../ARCHITECTURE.md#152-delivery-sequence-and-dependencies)'s
delivery sequence, starting with generated SDK bindings once contract release
qualification (gate G-01, below) is complete. *Dependencies:* FW-102, FW-601.
*Acceptance criteria:* each new package has its own locked toolchain, tests,
and CI job; none weakens the existing Rust workspace's checks.

**FW-102.** *Description:* [ARCHITECTURE.md §25.3](../ARCHITECTURE.md#253-reproducible-releases-and-supply-chain)
specifies a signed, versioned release manifest binding source revision,
lockfiles, exact WASM hashes, generated bindings, and a software bill of
materials. No such manifest has been published; only the working
[deployments/testnet.json](../deployments/testnet.json) record exists.
*Recommended approach:* define the manifest schema, anchor a signing key
independently of its distribution point, and require SDK/dApp startup to
verify it before enabling signing. *Dependencies:* none. *Acceptance criteria:*
a manifest exists, is independently reproducible from source, and is verified
by at least one consumer before that consumer accepts it as a trusted release.

## 2. Auction infrastructure (factory, registry, oracle adapter)

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-201 | Target-network resource/footprint qualification (gate G-05) | High | Planned | Auction, Factory, Registry |
| FW-202 | Target-network archival restoration submission (gate G-06) | High | Planned | All contracts |
| FW-203 | Broader classic-account/custom-account signer matrix (gate G-02) | Medium | Planned | Factory, Auction |
| FW-204 | Curated, governed oracle feed (gate G-10) | Medium | Research | New `contracts/oracle-feed/` |

**FW-201.** *Description:* local resource measurements (CPU, memory, footprint,
bytes) in [CONTRACT_REVIEW.md](security/CONTRACT_REVIEW.md#resource-measurement)
use the pinned SDK's default local-host limiter, not a specific deployed
network's current configuration. *Recommended approach:* re-measure every
resource-heavy branch (maximum Buyback, late exclusion, classic trustlines,
restored entries) against the actual target network's live limits before
enabling real inventory. *Dependencies:* none. *Acceptance criteria:* gate
G-05 in [DESIGN_AUDIT.md §5](security/DESIGN_AUDIT.md#5-open-implementation-and-deployment-gates)
is satisfied in full.

**FW-202.** *Description:* local tests use the pinned test host's recording-mode
automatic restoration; no `RestoreFootprint` transaction has been submitted to
a real network. *Recommended approach:* exercise the documented restoration
flow ([operations runbook](operations/README.md#ttl-and-restoration)) against
Testnet, including combined-resource and explicit-preamble cases.
*Dependencies:* FW-201. *Acceptance criteria:* gate G-06.

**FW-203.** *Description:* local tests cover a classic master-key account, a
classic 2-of-2 multisig account, and a custom Ed25519 account
([CRYPTOGRAPHIC_REVIEW.md](security/CRYPTOGRAPHIC_REVIEW.md)), seeded through
the test host rather than real network account/trustline setup. *Recommended
approach:* extend coverage to additional signer thresholds, sponsored
reserves, and wallet-driven signing before claiming broad account support.
*Dependencies:* FW-301 (wallet integration). *Acceptance criteria:* gate G-02.

**FW-204.** *Description:* [ARCHITECTURE.md §11.3](../ARCHITECTURE.md#113-oracleadapter-a-partial-raw-sep-40-read-surface)
and [§13.19](../ARCHITECTURE.md#1319-price-consumers-and-curated-oracle-boundary)
scope a separately governed, filtered feed (trusted sources, minimum
volume/execution fraction, staleness limits, manipulation policy) as future
work distinct from the already-implemented raw `OracleAdapter`. *Recommended
approach:* this needs a design decision (recorded under
[docs/decisions/](decisions/)) before implementation: source eligibility,
governance model, and whether remaining SEP-40 methods (`assets`, `price`,
`prices`) are in scope. *Dependencies:* none; independent of the auction
product. *Acceptance criteria:* gate G-10.

## 3. Clearing algorithm

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-301 | Broader stateful contract/reference comparison (gate G-07) | Medium | Planned | Auction clearing |
| FW-302 | Reviewed capacity increase beyond 16 bids | Low | Research | Auction, Common |

**FW-301.** *Description:* current evidence is 48 compiled-WASM clearing
fixtures plus 10,000 model-only generated books
([TESTING.md](TESTING.md#independent-models-and-vectors)); the model
comparisons are not guest-executed. *Recommended approach:* grow the
compiled-WASM fixture count and branch/path exploration, prioritizing sparse
Buyback gaps, maximal exclusion sequences, and near-`i128` books.
*Dependencies:* none. *Acceptance criteria:* gate G-07.

**FW-302.** *Description:* [ARCHITECTURE.md §1.1](../ARCHITECTURE.md#11-core-architectural-decisions)
and [§13.22](../ARCHITECTURE.md#1322-scaling-and-contract-evolution) record
that `MAX_BIDS = 16` is a reviewed, benchmarked choice tied to the atomic
finalizer fitting one transaction's resource limits — not an arbitrary
constant. *Recommended approach:* any increase requires its own reviewed
template/release, re-qualifying atomic finalization with classic trustlines,
contract accounts, late exclusions, and target-network limits; if capacity
cannot fit an atomic final check, the architecture requires reviewing a
different compliance model explicitly rather than silently paginating it.
*Dependencies:* FW-201. *Acceptance criteria:* a new, separately reviewed
release; never a change to the deployed `0.1.0` template.

## 4. Open and sealed bidding

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-401 | Automatic/threshold reveal service | Low | Research | New service, Auction (new release) |
| FW-402 | SDK-side encrypted reveal-recovery vault | Medium | Planned | `sdk/core`, `dapp` |

**FW-401.** *Description:* the implemented commit-reveal requires the
bidder's own authorized `reveal_bid` call; Soroban cannot recover bid details
from a hash on its own. A design direction for automatic reveal (encrypted
payload plus a threshold-decryption service) has been discussed but not
specified. *Recommended approach:* this is a research item, not a scoped
task — it needs a selected cryptographic scheme, an operator-threshold model,
payload-availability guarantees, and a ledger-based release policy before it
can be scoped; it would also require a new immutable auction release, since
the current contract only allows the bidder's own authorized reveal. Existing
deployed auctions keep their current authorization rule unconditionally.
*Dependencies:* none. *Acceptance criteria:* a recorded design decision
(see [docs/decisions/](decisions/)) before any implementation begins.

**FW-402.** *Description:* [ARCHITECTURE.md §13.12](../ARCHITECTURE.md#1312-investor-participation-and-reveal-recovery)
specifies a fixed Argon2id/AES-256-GCM encrypted recovery-envelope profile for
sealed-bid secrets. No SDK or dApp implementing it exists yet. *Recommended
approach:* implement the profile exactly as specified (fixed KDF parameters,
context binding, 64 KiB import bound) and get it reviewed before any sealed
bid depends on it for recovery. *Dependencies:* FW-101 (`sdk/core`).
*Acceptance criteria:* the encrypted export/import acceptance cases in
[ARCHITECTURE.md §23.2](../ARCHITECTURE.md#232-sdk-and-wallet-acceptance) pass.

## 5. Escrow and settlement / 6. Refund mechanisms

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-501 | Full classic-account/trustline failure matrix (gate G-04) | Medium | Planned | Auction |
| FW-502 | Additional adversarial-account settlement scenarios | Low | Planned | Auction |

**FW-501.** *Description:* two-leg rollback, reserves, credits, deficits, and
expiry are covered locally with the test host's accounts
([CONTRACT_REVIEW.md](security/CONTRACT_REVIEW.md)); a complete classic
trustline/transfer failure matrix on a real network remains open.
*Recommended approach:* exercise frozen trustlines, closed trustlines mid-flow,
and partial-authorization combinations on Testnet before Mainnet qualification.
*Dependencies:* FW-201. *Acceptance criteria:* gate G-04.

**FW-502.** *Description:* [CONTRACT_REVIEW.md "Remaining evidence and scope"](security/CONTRACT_REVIEW.md#remaining-evidence-and-scope)
calls for additional adversarial accounts beyond the currently tested classic
master-key, 2-of-2 multisig, and custom Ed25519 cases. *Recommended approach:*
add sponsored-reserve accounts and higher-threshold multisig combinations.
*Dependencies:* FW-203. *Acceptance criteria:* gate G-04.

## 7. RWA authorization and compliance

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-701 | Issuer/asset onboarding verification workflow | High | Planned | New `services/onboarding/` |
| FW-702 | Isolated metadata/document fetcher (SSRF-safe) | High | Planned | `services/onboarding/`, `dapp` |

**FW-701.** *Description:* [ARCHITECTURE.md §13.8](../ARCHITECTURE.md#138-issuer-and-asset-onboarding)
specifies the full onboarding workflow (Classic-to-SAC identity verification,
investor verification, asset-authorization requests); none of it is
implemented — the PoC assumes authorization already exists on both assets.
*Recommended approach:* implement as a private service with its own storage
and access boundary, separate from public trading infrastructure.
*Dependencies:* none. *Acceptance criteria:* a verification record exists for
every onboarded issuer/investor before their first on-chain action; sensitive
documents never reach the public API.

**FW-702.** *Description:* [ARCHITECTURE.md §13.10](../ARCHITECTURE.md#1310-dapp-structure-and-data-state)
specifies an isolated HTTPS-only fetcher for issuer-supplied metadata/document
URLs, with destination validation, private/loopback/metadata-address blocking,
and redirect handling, following the
[OWASP SSRF prevention cheat sheet](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html).
Not implemented. *Recommended approach:* build and independently review this
fetcher before any issuer-supplied URL is fetched server-side.
*Dependencies:* FW-701. *Acceptance criteria:* the metadata-redirect and
private-network-destination acceptance cases in
[ARCHITECTURE.md §23.2](../ARCHITECTURE.md#232-sdk-and-wallet-acceptance) pass.

## 8. Oracle and pricing infrastructure

See §2 (FW-204) for the curated feed. No further items beyond the raw
`OracleAdapter`, which is implemented.

## 9. Smart contract security

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-901 | Independent external contract audit | Critical | Blocked | All contracts |
| FW-902 | Fresh dependency/advisory review before public release | Medium | Planned | Cargo workspace |

**FW-901.** *Description:* every review to date (design, contract, auction
logic, cryptographic, and this architecture/documentation pass) is an
internal engineering review. [DESIGN_AUDIT.md gate G-12](security/DESIGN_AUDIT.md#5-open-implementation-and-deployment-gates)
and [GOVERNANCE.md](../GOVERNANCE.md) both require independent external audit
and remediation before valuable or Mainnet inventory is placed at risk.
*Recommended approach:* commission an external audit against a tagged
release once contract release qualification (§2 above) is otherwise complete;
budget time for remediation before any Mainnet deployment. *Dependencies:*
FW-201, FW-202. *Acceptance criteria:* gate G-12; findings resolved per the
release policy in [ARCHITECTURE.md §25.1](../ARCHITECTURE.md#251-project-policies-and-responsibility).

**FW-902.** *Description:* the cached `cargo audit` review in
[CONTRACT_REVIEW.md](security/CONTRACT_REVIEW.md#dependency-review) used a
local RustSec snapshot dated 3 October 2026, not a live online check.
*Recommended approach:* re-run against current advisories immediately before
any public/Mainnet release, and periodically per
[MAINTENANCE.md](../MAINTENANCE.md)'s weekly dependency-review cadence.
*Dependencies:* none. *Acceptance criteria:* a dated, fresh advisory report
with no unresolved known vulnerability in the release dependency tree.

## 10. Integration APIs and SDKs

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-1001 | Generated TypeScript bindings and `@crossing/sdk` | High | Planned | `sdk/generated`, `sdk/core` |
| FW-1002 | Wallet integration and signing acceptance matrix (gate G-09) | High | Planned | `dapp`, `sdk/core` |
| FW-1003 | Indexer, database, and public query API | High | Planned | `services/indexer`, `services/api` |
| FW-1004 | Keeper service | Medium | Planned | `services/keeper` |

**FW-1001.** *Description:* [ARCHITECTURE.md §13.5](../ARCHITECTURE.md#135-sdk-packages-and-exact-numeric-handling)
specifies `@crossing/contracts` (generated bindings) and `@crossing/sdk`
(orchestration: exact amounts, commitments, transaction preparation). Neither
package exists; only independent Python/JavaScript reference scripts do.
*Recommended approach:* generate bindings from the exact released WASM
specifications once FW-102 exists, with CI drift detection against the ABI.
*Dependencies:* FW-102. *Acceptance criteria:* every canonical config/
commitment vector round-trips through the SDK and matches the independent
Rust/reference output byte for byte.

**FW-1002.** *Description:* no wallet integration exists.
[ARCHITECTURE.md §23.2](../ARCHITECTURE.md#232-sdk-and-wallet-acceptance) lists
the required acceptance matrix (wrong network/config rejection, modified
invocation rejection, RPC-timeout reconciliation, multisig/contract-account
support). *Recommended approach:* integrate a maintained wallet-adapter
library against the published compatibility matrix; do not claim support for
an account type that has not been tested through this matrix. *Dependencies:*
FW-1001. *Acceptance criteria:* gate G-09; every row of the §23.2 table passes
for each supported wallet/account combination.

**FW-1003.** *Description:* [ARCHITECTURE.md §13.14–13.17](../ARCHITECTURE.md#1314-indexer-requirement-and-trust-boundary)
specify durable ledger ingestion, PostgreSQL projections, reconciliation, and
a versioned public API. None are implemented; the bounded contract getters
are not a substitute for cross-auction search. *Recommended approach:* build
ingestion first (idempotent, replay-safe, gap-aware) since the API and dApp
both depend on its projections. *Dependencies:* FW-1001 (shared ABI
decoding). *Acceptance criteria:* gate G-07/G-08's indexer-facing parts; the
failure/load acceptance cases in
[ARCHITECTURE.md §23.3](../ARCHITECTURE.md#233-indexer-and-service-acceptance).

**FW-1004.** *Description:* [ARCHITECTURE.md §13.18](../ARCHITECTURE.md#1318-keeper-scheduling-and-service-policy)
specifies a permissionless keeper that calls the already-existing public
contract methods (`advance`, `finalize`, `expire`, `expire_range`,
`record_clearing`, `record_settlement`, `maintain`) on a schedule. Not
implemented; every auction currently depends on a manual or scripted caller.
*Recommended approach:* implement as an independently restartable worker with
durable job leases, since the contracts themselves remain safe without it
(anyone can call these methods manually). *Dependencies:* FW-1001.
*Acceptance criteria:* the keeper-outage and competing-keeper rows in
[ARCHITECTURE.md §13.21](../ARCHITECTURE.md#1321-observability-and-failure-isolation)/[§23.3](../ARCHITECTURE.md#233-indexer-and-service-acceptance).

## 11. Testing and verification

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-1101 | End-to-end wallet-to-ledger acceptance pilot (gate G-09, §23.4) | High | Planned | Full stack |
| FW-1102 | Transitive dependency/license inventory generation | Low | Planned | Release tooling |

**FW-1101.** *Description:* [ARCHITECTURE.md §23.4](../ARCHITECTURE.md#234-integrated-lifecycle-and-launch-gate)
requires running all four mode combinations through real wallet signing on
Testnet before launch, including an oversubscribed Sale, sparse Buyback,
minimum-fill cancellation, non-reveal penalty, and a credit restored after a
service migration. Not yet run; it depends on the SDK/dApp/keeper existing.
*Recommended approach:* schedule as the final pre-launch gate, after FW-1001
through FW-1004. *Dependencies:* FW-1001, FW-1002, FW-1003, FW-1004.
*Acceptance criteria:* every row of §23.4 passes with reconciled balances.

**FW-1102.** *Description:* [ARCHITECTURE_REVIEW.md "Limits and implementation
acceptance"](security/ARCHITECTURE_REVIEW.md#limits-and-implementation-acceptance)
records that a full transitive license inventory was attempted but not
completed (an uncached package needed network access unavailable in that
environment). *Recommended approach:* regenerate in an environment with
network access to the Cargo registry, before any binary redistribution.
*Dependencies:* none. *Acceptance criteria:* a complete, reviewed dependency
license inventory accompanies the next public release artifact.

## 12. Performance and scalability

Covered by FW-201 (resource qualification) and FW-302 (capacity increase).
No additional items.

## 13. Deployment and operations

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-1301 | Private vulnerability reporting (hosting control G1) | Medium | Blocked | GitHub repository settings |
| FW-1302 | Branch protection on `main` (hosting control G2) | Medium | Blocked | GitHub repository settings |
| FW-1303 | Production monitoring/alerting and recovery drills | High | Planned | Operations |

**FW-1301 / FW-1302.** *Description:* tracked in detail in
[SECURITY.md "Planned hosting controls"](../SECURITY.md#planned-hosting-controls).
Both require GitHub organization-admin access that the current maintainer
accounts' tooling does not hold. *Recommended approach:* an account holder
with organization-admin rights enables these directly in repository settings.
*Dependencies:* none (access, not engineering, is the blocker). *Acceptance
criteria:* both rows in SECURITY.md's table flip from open to configured and
can then be removed from that table.

**FW-1303.** *Description:* [ARCHITECTURE.md §26.3](../ARCHITECTURE.md#263-service-schedule-and-recovery-objectives)
specifies measurable recovery objectives (60-minute API recovery, 15-minute
keeper/signer recovery) and monthly restore/replay drills. No hosted service
exists yet to monitor or drill. *Recommended approach:* implement once
FW-1003/FW-1004 exist; do not claim these targets as met before a service
exists to measure them against. *Dependencies:* FW-1003, FW-1004.
*Acceptance criteria:* a documented, dated drill meeting each stated target.

## 14. Documentation

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-1401 | Named accountable roles and endpoints for production services | Medium | Blocked | MAINTAINERS.md, release records |

**FW-1401.** *Description:* [ARCHITECTURE.md §25.1](../ARCHITECTURE.md#251-project-policies-and-responsibility)
requires a primary/backup owner, access policy, and handover instructions for
each production service (contracts/security, SDK, dApp, data services,
operations, release management). [MAINTAINERS.md](../MAINTAINERS.md) names
current maintainers for the repository itself; it does not yet name owners
for services that do not exist yet. *Recommended approach:* assign these
roles as each corresponding service in §10/§13 above is actually built, not
in advance of it. *Dependencies:* the corresponding service item.
*Acceptance criteria:* every row in the eventual production-services table
has a named primary and backup.

## 15. Research and optional extensions

| ID | Title | Priority | Status | Component |
|---|---|---|---|---|
| FW-1501 | Settlement-asset ramps (SEP-24 anchor integration) | Low | Deferred | `dapp`, external anchors |
| FW-1502 | Protocol fee / separate treasury role | Low | Deferred | Contracts (new release) |
| FW-1503 | Discretionary pause / emergency mechanism | Low | Deferred | Contracts (new release) |
| FW-1504 | Arbitrary-token (non-SAC) support | Low | Deferred | Contracts (new release) |

**FW-1501–FW-1504.** *Description:* [ARCHITECTURE.md §1.2](../ARCHITECTURE.md#12-scope)
and [§13.22](../ARCHITECTURE.md#1322-scaling-and-contract-evolution) explicitly
exclude these from the initial auction product: no AMM/bridge/leverage, no
protocol fee, no discretionary pause, and no arbitrary-token support. They
are listed here only so they are not silently forgotten, not because they are
scheduled. *Recommended approach:* each would need its own design decision,
threat model, and — for anything touching the contracts — a new immutable
release; none should be assumed from this register's existence.
*Dependencies:* none currently. *Acceptance criteria:* none currently; a
future decision record in [docs/decisions/](decisions/) would set them.

## Cross-reference index

| Architecture section | Future-work items |
|---|---|
| [§1.1–1.2 Scope and decisions](../ARCHITECTURE.md#1-product-scope-and-decisions) | FW-302, FW-1501–FW-1504 |
| [§11.3 OracleAdapter](../ARCHITECTURE.md#113-oracleadapter-a-partial-raw-sep-40-read-surface) | FW-204 |
| [§13 Full application and Stellar integration](../ARCHITECTURE.md#13-full-application-and-stellar-integration) | FW-101, FW-401, FW-402, FW-701, FW-702, FW-1001–FW-1004 |
| [§15.1 Target repository additions](../ARCHITECTURE.md#151-target-repository-additions) | FW-101 |
| [§22 Adversarial economics and release policy](../ARCHITECTURE.md#22-adversarial-economics-and-release-policy) | FW-901 |
| [§23 Full system acceptance criteria](../ARCHITECTURE.md#23-full-system-acceptance-criteria) | FW-201–FW-203, FW-501, FW-502, FW-1002, FW-1003, FW-1101 |
| [§25 Open-source governance and distribution](../ARCHITECTURE.md#25-open-source-governance-and-distribution) | FW-102, FW-901, FW-902, FW-1102 |
| [§26 Maintenance and project continuity](../ARCHITECTURE.md#26-maintenance-and-project-continuity) | FW-1301–FW-1303, FW-1401 |
| [DESIGN_AUDIT.md gates G-01–G-12](security/DESIGN_AUDIT.md#5-open-implementation-and-deployment-gates) | FW-201–FW-204, FW-301, FW-501, FW-502, FW-901, FW-1002, FW-1003 |

Update this register when an item is scoped into an active plan, completed, or
superseded — do not delete a completed item silently; move it to
[CHANGELOG.md](../CHANGELOG.md) with a reference back to its ID here.
