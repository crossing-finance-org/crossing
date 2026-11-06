# Maintenance and support policy

Maintenance covers the completed Crossing system: contract releases, SDK,
dApp, public data services, keepers, deployment configuration and documentation.
This policy defines production responsibilities, support boundaries, recurring
work and recovery requirements. Operators implement and validate them before
claiming a supported deployment. Implementation status remains separate: the
current repository contains a contract PoC, not running production services.

## Component maintenance

| Area | Required maintenance task | Verification |
|---|---|---|
| Contracts and shared types | Review correctness, bounds, auth, accounting and compatible clients | Compiled-WASM regressions and independent vectors |
| Build and test tools | Keep locked builds, artifact verification and coverage evidence trustworthy | Tooling tests, build manifests and unchanged gates |
| Dependencies and toolchain | Review update proposals and advisories without silently changing pins | Applicable tests, artifact/hash comparison and compatibility note |
| Documentation | Reconcile architecture/API/runbooks with current source and evidence | Links, anchors, code fences and source comparison |
| Testnet records | Retain original observations and distinguish them from current network state | Dated manifests and public transaction references |
| SDK and wallets | Keep exact bindings, amount parsing, recovery format and signing compatibility | Cross-language vectors and supported-wallet integration tests |
| dApp | Preserve user intent, supported browser crypto, old-release exits and accessibility | End-to-end signing/recovery and dependency checks |
| Indexer/API | Maintain schemas, event decoders, complete replay and honest freshness | Raw-ledger replay, reconciliation and query/load tests |
| Keepers and signers | Maintain deadlines, retries, budgets, TTL and credential separation | Restart/failover drills and reconciled transaction journals |
| Community | Triage scoped reports and explain accepted/declined changes | Issues, review history and release notes |
| License and distribution | Preserve license/notice and inspect incorporated third-party material | Package contents and distribution review |

Dependabot proposes Cargo and workflow updates weekly. A proposed update does
not imply an advisory scan, acceptance or a new deployment. Review dependency
behavior, license, feature and compiler changes. SDK/host changes need renewed
resource, authorization, encoding and TTL evidence; passing compilation alone
is insufficient. Production owners review the scheduled queue and advisories,
record disposition and escalate urgent exposure. The absence of an update
proposal is not proof that dependencies are free of known issues.

## Change and release procedure

1. Identify the issue, affected source/release and whether it involves confidential
   security information. Use the private reporting path when appropriate.
2. Propose the smallest correction with a failing regression for behavior/tooling
   defects. Record compatibility changes and update owning documentation.
3. Run the applicable checks. Contract changes use `scripts/check.sh` and required
   coverage/resource evidence; documentation-only changes use documentation
   checks. Packaging changes need exact-content and stale-artifact tests.
4. Review the final diff and generated fixtures. Do not weaken gates, replace
   measured evidence or present an unrun check as passed.
5. For a release, build and verify exact WASMs, collect review evidence and
   package checksums, manifests, LICENSE, NOTICE and required upstream notices.
   Retain source/lockfiles and tool versions. CI's 14-day artifact retention is
   temporary test evidence, not a permanent public release archive.
6. Publish a new immutable tag/artifact set and release notes through an authorized
   repository operator. Never overwrite an existing release with different bytes.
7. If deployed, use the separately reviewed deployment procedure and record the
   new addresses. Existing auctions stay under their original code and terms.

The current PoC does not promise stable `0.x` interfaces. Call out changed
configuration, domains, storage, errors/events and client requirements explicitly.
A patch version cannot change the identity of already deployed code.

## Independent operator responsibilities

Anyone operating the software selects their own network, assets, fee/rent
budget, providers, signing policy, backup policy and support arrangements. The
license does not appoint the original repository maintainer as that operator.
Before valuable use, assign accountable people to these responsibilities:

| Responsibility | Required access | Must be isolated from |
|---|---|---|
| Release review and publication | Source/releases and manifest signing as configured | Investor private keys and asset-admin credentials |
| Ingestion and reconciliation | Network reads and projection writes | User signing and asset-admin operations |
| Query service | Read-only public projections | Private onboarding documents and keeper signing |
| Keeper and retention | Public-method fee signer and job journal | Owner-authorized funding, reveal and withdrawals |
| Asset onboarding | Provider integration and approved asset-admin workflow | General-purpose public API inputs |
| Incident coordination | Alerts, affected-release status and runbooks | Any assumed escrow override |

A production operator records actual ownership, primary/backup coverage and
key access for each role. Separate critical review/publication and custodial
credentials even when some service duties are combined. Identities are maintained
in operator records; architectural role definitions do not require personal
names in the technical specification.

## Production service maintenance schedule

These procedures are required for the completed hosted system. Release evidence
records who performs them and the measured results; their presence in a document
does not assert that services have already been deployed.

| Trigger or cadence | Action | Completion evidence |
|---|---|---|
| Every observed closed ledger | Check eligible closing/expiry jobs, state changes, ingestion progress and imminent deadlines | Watermark/job observations and reconciled outcomes |
| Continuous alerting | Detect provider outage, queue delay, unknown attempts, deficits and rent/fee exhaustion | Incident record and acknowledged recovery action |
| Daily while active auctions exist | Review unresolved claims, fee/rent runway, source history headroom and backup health | Operator checklist with network and ledger |
| Weekly | Review dependency/advisory proposals, recurring failures and restoration demand | Accepted/deferred decisions with reasons |
| Monthly | Restore a backup to an isolated environment and replay representative ledgers | State/projection comparison and recovery timing |
| Before provider/protocol/toolchain changes | Run compatibility, resource, signing and archival drills | Exact version/network matrix and results |
| Before every release | Check artifacts, licensing, ABI/domain/event changes, review findings and rollback | Immutable release evidence |
| Before service retirement | Drain work queues, publish exit instructions and export public recovery data | Announced status, manifests and verified recovery paths |

Thresholds are configured from observed ledger cadence and action deadlines,
not assumed wall-clock block times. An alert must leave enough time for multiple
failed submissions, operator response and restoration. Public keepers do not
guarantee inclusion when the network halts or fee policy is exhausted.

## Retention inventory and ownership recovery

Build a per-network-instance inventory from verified factory registration,
auction config/provenance, admitted bid IDs and owner indexes. Include all
financial keys, replay guards and dependencies. A database is an inventory aid;
chain state remains authoritative.

| Record group | Maintenance path | Recovery dependency |
|---|---|---|
| Auction instance and shared WASM | Mutation retention and supported code/instance TTL operations | Original executable, config, state and financial totals |
| Bid, tick, owner index and nonce | Typed auction `maintain`, at most eight keys per call | Consistent original records and replay guards |
| Owner/issuer credit | Typed credit maintenance and instance/code retention | Same owner, same asset and unwithdrawn amount |
| Factory metadata/discovery | Factory bounded maintenance | Registration and authentic release bindings |
| Registry observations/history/pointers | Registry bounded maintenance | Source auction, pair history and pointed observation/execution |
| OracleAdapter | Adapter `maintain` | Independently maintained/restored registry |
| SAC contract balance/authorization data | Asset-aware simulation and supported ledger operations | Actual asset permissions and original ledger entries |

At construction, the auction's financial horizon is at least delivery deadline
plus 120,960 ledgers, with a rolling target of 518,400 capped by current maximum
TTL. These are ledger counts, not guaranteed days. Construction protects instance
and reused code independently; it does not renew every external SAC or registry
dependency. Credit ownership continues after this horizon but someone must pay
retention or restoration costs.

Use a configurable safety threshold before expiry. Simulate maintenance,
account for both shared code and instance, submit with a bounded fee and confirm
actual live-until ledgers. Keep a renewal journal so repeated failures trigger
alerts instead of an infinite fee loop. A read/simulation cannot extend on-chain
retention. Temporary storage is unsuitable for ownership or replay records.

For archived state, use the target network's supported invocation restoration
or an explicit restoration transaction. Preserve original values. If separate
restoration is used, confirm it and resimulate the intended operation. Restoring
an old credit does not recreate a bid window or bypass permission/solvency checks.
[Stellar archival](https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival).

## Financial and data reconciliation

For each asset independently verify `pending + reserve + credits = liability`
and `actual balance >= liability`. Enumerate the issuer and admitted bid owners
for credit reconciliation. Do not sum RWA and quote units or infer withdrawals
from an event feed alone. Inspect SAC changes even without Crossing events.

Maintain separate ingestion and reconciliation watermarks. Complete event
ingestion does not prove that late getter-based enrichment is a historical
snapshot. Mark gaps/mismatches degraded, preserve raw evidence and replay or
refresh current observations before clearing an alert.

Never make financial state match a database by changing owners, inventing
credits, deleting replay guards or paying early claimants from insolvent escrow.
Deficits suspend outflows under the contract; expiry bookkeeping can still
resolve liabilities into credits. Recovery requires actual replenishment or
asset-admin action where applicable.

## Backups and recovery objectives

For a full service, back up raw ledger evidence/checksums, release manifests,
projection schemas/data, checkpoints, job/attempt journals and encrypted service
configuration. Do not copy wallet seeds or user reveal bundles into server
backups. Keys have their own restricted backup and rotation policy.

The reference production requirements are:

| Component | Recovery time objective | Recovery point and verification |
|---|---|---|
| Read API | 60 minutes | Rebuild from verified projections; freshness/degraded status remains accurate |
| Keeper and signer | 15 minutes, or tighter if auction headroom requires | No loss of durably acknowledged signed-attempt/hash/sequence journal |
| Projection database | 60 minutes | At most 15 minutes of snapshot lag, closed by durable raw-ledger replay |
| Raw ledger archive | Available from replica within 60 minutes | No loss of acknowledged complete ledgers within the defined storage fault model |
| Release manifests and artifacts | Available from independent mirror within 60 minutes | Exact immutable checksums and provenance retained |

Validate these targets under loss of one process, node or availability zone;
simultaneous destruction of all replicas/backups is outside that fault model.
Acknowledge operator journal/raw-evidence writes only after the configured durable
replication requirement is satisfied, including a separate failure domain.
Persist signed-attempt identity before broadcast. Archive deterministic ledger
objects with checksum validation and retain independent backup/mirror coverage.
User-owned browser reveal secrets are outside server recovery objectives.

Closing-window policy must reserve enough headroom for the stricter of measured
workload/retries and operator recovery time. A 15-minute recovery target cannot
justify a five-minute closing window. Drills verify actual recovery times and
journal/sequence reconciliation before any replacement transaction is built.

A restore drill uses an isolated environment with outbound signing disabled:
verify network incarnation/checkpoint hashes, restore data, replay from the
last complete ledger, compare representative bids/credits/balances against
chain, reconcile known transaction hashes and only then enable public workers.
Reusing an old Testnet database across a reset is prohibited.

## Incidents and emergency fixes

| Condition | Immediate action | Completion condition |
|---|---|---|
| Unknown transaction outcome | Preserve attempt/hash; query verified providers and state | Inclusion or proven expiration/nonexecution reconciled |
| Frozen recipient or full trustline | Preserve credit/outcome and show restriction | Holder/admin repairs capacity or authorization |
| Escrow deficit | Alert, retain evidence and rely on blocked outflows | Actual backing restored and independently reconciled |
| Ingestion gap/provider disagreement | Quarantine affected projections/range | Consistent verified replay and no missing ledger |
| Compromised service credential | Disable access, rotate and inspect signed-attempt history | Access restored with known affected scope |
| Suspected contract defect | Stop new hosted starts for affected release and follow security process | Disclosure, reviewed new release and accurate old-release guidance |

Operational switches cannot pause or upgrade existing auctions. Release/deploy
keys cannot redirect credits. Do not promise that a new deployment repairs
old escrow. For emergencies, record what was reviewed, which checks ran and
which remain incomplete; urgency does not turn missing evidence into a pass.

## Deprecation and end of service

The target stable-release policy supports the current SDK major plus its
previous major for 12 months after replacement. Announce API-major retirement
at least six months in advance and publish concrete dates. Security backports
within a supported line follow compatibility policy; contract corrections deploy
a new immutable release rather than rewriting old instances. Preserve public
contract provenance/specifications and exit tooling for outstanding positions.

These are requirements of the completed project; the current pre-stable PoC has
no published stable package or running service SLA. Distinguish unsupported
clients, disabled creation routes, retired read services and immutable contract
state; they are different events.

For service retirement, stop creating new obligations first. Publish
affected networks/releases, outstanding delivery/expiry work, direct RPC/CLI
instructions, release manifests and available public data exports. Retain old
ABI/commitment vectors and source tags so another operator can build a client.
Continue already-promised service only within the operator's stated policy;
identify who will pay any remaining TTL/expiry costs without implying eternity.

Outstanding credits never become maintainer or issuer property because a service
ends. Archival may add restoration cost. Users can still submit compatible
transactions through another provider while network and asset conditions permit.
Fork operators publish their own identity and support policy under
[GOVERNANCE.md](GOVERNANCE.md) and [LICENSING.md](LICENSING.md).

