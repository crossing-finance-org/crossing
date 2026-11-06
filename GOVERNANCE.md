# Project governance

Crossing's governance covers the complete open-source project: contracts, SDK,
dApp, data services, operating software and documentation. It defines how source,
architecture and releases are maintained. Repository governance does not control
existing auction escrow, replace asset administrators or create on-chain voting.

Role definitions and review requirements apply to the completed system. Actual
maintainer identities and access membership are operational records, not invented
architecture values; current maintainers are recorded in
[MAINTAINERS.md](MAINTAINERS.md), kept in agreement with
[.github/CODEOWNERS](.github/CODEOWNERS). The repository's current contract PoC
can use a smaller contributor group, but production release requires the
ownership and review coverage defined below. [MAINTENANCE.md](MAINTENANCE.md)
defines ongoing duties.

## Roles and authority

| Role | Responsibility | Boundary |
|---|---|---|
| Contributor | Propose changes, provide evidence and respond to review | No merge or release authority follows from submitting a change |
| Reviewer | Assess behavior, compatibility, tests and documentation | Review approval is not an independent security audit |
| Maintainer | Triage, accept scoped changes, maintain documentation and prepare releases | Repository access cannot change deployed immutable code |
| Release operator | Verify and publish a specific artifact bundle and manifest | Deployment identity is separate from asset administrator identity |
| Asset administrator | Manage the underlying asset's authorization and permitted controls | A token role, not a repository-governance role |
| Independent operator | Run a fork, client, indexer or public keeper | Must identify its own deployment and service policies |

Every production component has a primary maintainer and backup. Critical
review, release verification and publication cannot be self-approved by the
author alone. Maintainer appointments require demonstrated contribution,
review competence and recorded approval by existing project leadership. Record
access, responsibilities, backup coverage and eventual removal/handover. An
independent contract review remains a separate production gate.

## How decisions are made

Ordinary fixes use a focused issue or pull request with a reproducible problem,
final behavior and validation. Review comments remain attached to the change.
Maintainers may accept, request changes or decline with a technical explanation.
Contributors can propose a narrower change or a documented alternative.

Changes to economics, authority, custody, configuration, storage, commitments,
events, public APIs, capacity or supported assets need a written design decision
before implementation. Record the decision in `docs/decisions/` using the
[decision template](docs/decisions/README.md). Include alternatives, trust changes,
compatibility, resource implications, migration and acceptance evidence. A private
security fix may publish its decision after coordinated disclosure.

For disputed design changes, summarize the disagreement and evidence publicly,
allow substantive review, and keep the current behavior until a maintainer
records a decision. A comment count is not a vote or a substitute for a safety
argument. Urgent private security decisions are recorded and disclosed when
appropriate; ordinary discussion is not a substitute for the release gates.

## Review and merge policy

All changes should use pull requests and pass the checks applicable to their
scope. Contract, authentication, clearing, accounting, storage and release-tooling
changes require particular attention to regressions and compatibility. A qualified
reviewer other than the author is required for production-sensitive changes,
followed by separate release verification. If the current contributor group
cannot provide that coverage, record the limitation and do not label the change
production-approved. A PoC development merge can preserve that explicit boundary.

Do not bypass failing checks by weakening coverage, deleting regressions or
regenerating expected values without an explained behavior change. Dependency
updates receive the same review as handwritten changes, including build and
license effects. Documentation changes preserve the distinction between current
implementation, measured evidence and planned architecture.

Branch protection, required status checks, protected tags, publishing permissions
and private vulnerability reporting are hosting settings. Files in this repository
do not prove that those settings are enabled. Before publishing a supported
production release, verify those controls and record their configuration.
[GitHub branch protection](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches).
Branch protection and private vulnerability reporting are currently tracked
as open, named future actions in [SECURITY.md](SECURITY.md#planned-hosting-controls),
not assumed enabled.

## Releases and compatibility

The source is currently `0.1.0` and pre-stable. Describe breaking changes explicitly
even before `1.0.0`. Documentation versions are independent of software versions;
an architecture update does not create a new contract deployment.

Use immutable release tags and checksummed artifacts. A released correction gets
a new version and tag; it does not replace bytes under an old version. Record
source, lockfiles, compiler, specification and WASM hashes, review evidence,
license/notice files and any deployment manifest. The detailed procedure is in
[operations](docs/operations/README.md).

After stable public interfaces exist, use semantic versioning for SDK/API changes.
Any contract bytecode change still needs a new deployment identity, even when
the source change is called a patch. Configuration/commitment compatibility is
determined by exact encoding and domain, not only a version label.
[Semantic Versioning](https://semver.org/).

## Security and emergencies

Follow [SECURITY.md](SECURITY.md) for confidential reports. Keep exploit details
out of public issues until a coordinated disclosure decision. Repository
maintainers may restrict new hosted entry points or publish affected release
information; they cannot pause immutable escrow or override SAC permissions.

An incident record identifies affected versions/hashes, impact, mitigation,
replacement release if any, evidence and unresolved limitations. Never claim
existing funds were migrated by a source release. Asset-admin action and
permissionless expiry/withdrawal remain subject to the deployed contract rules.

## Continuity and forks

Maintainers should document handovers, remove obsolete access and retain release
history. If active maintenance stops, publish the status and available recovery
instructions. The production support/deprecation window is defined in MAINTENANCE and
published with concrete release dates. Publish changes prospectively and retain
exit/provenance information for outstanding positions.

Anyone may fork under [Apache-2.0](LICENSE), subject to its terms. A fork identifies
its own operators, addresses, manifests, review evidence and service endpoints.
It cannot represent a different deployment as the original release or move
existing escrow merely by reusing the source. The license does not confer project
trademark rights. See [LICENSING.md](LICENSING.md).

