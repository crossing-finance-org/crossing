# Architecture and documentation audit

Date: **5 November 2026**. Target: full-project architecture **v2.1**, the current
`0.1.0` contract interface, published repository documentation and release tooling.
Corrections are incorporated directly in their owning documents. This separate
report records scope, findings, evidence and remaining implementation gates.

The architecture specifies the completed production project. The existing code
is its contract PoC; it does not limit the design's application, indexing,
governance, maintenance or release requirements. This is an internal technical
documentation/design audit, not an independent external contract audit or a
production deployment certification.

## Scope and method

Review the published Markdown documents, LICENSE/NOTICE, deployment manifest,
contract source/types, test definitions, build/packaging scripts and configured
CI. Private ignored archives are outside the published-project scope. Existing
dated measurements remain historical evidence and are not relabeled as new runs.

| Document group | Reviewed subjects |
|---|---|
| Architecture, project README and documentation index | Full-system scope, component boundaries, economics, authority, statuses and document ownership |
| Contract API and five contract/shared source files | Current methods/types, errors, Config, commitment domains, phases, bounds, accounting and publication |
| Operations, maintenance and support | Deployment, fee/rent funding, retention/restoration, deadlines, recovery, support and retirement |
| Governance, contribution, conduct, decision format and pull-request template | Authority, independent production review, contribution rights, community and continuity |
| Licensing, LICENSE/NOTICE and packaging | Declared license, attribution, distribution contents and third-party review boundary |
| Testing, demonstration and security-review records | Model/guest/native distinction, real versus mocked authorization, historical evidence and open gates |
| Security policy | Private reporting, incident authority, disclosure and accuracy of audit claims |
| Changelog and Testnet records | Historical chronology, source/build identity, transaction references and dated balances |

Trace asset ownership from escrow to allocation, resolution and withdrawal.
Compare every public method and error with the API reference, configuration
with its source schema, and recorded source-input hashes with current files.
Check all published local Markdown links and heading references. Compare relevant
Stellar semantics against primary documentation: SAC identity, authorization,
archival, simulation, ledger/event access and bounded history.

## Previously identified corrections retained

| ID | Severity | Finding | Applied correction |
|---|---|---|---|
| A-01 | Medium | Bidder authority implied exclusive control over delivery timing | Architecture actor table and claim flow state that any caller can resolve a bid to fixed recipients |
| A-02 | Medium | Passphrase/network ID alone did not isolate Testnet resets | Off-chain network-instance/checkpoint identity scopes databases, jobs, caches, API and recovery context without changing the commitment preimage |
| A-03 | Medium | Event ingestion completion could be mistaken for historical financial reconciliation | Separate ingestion/reconciliation watermarks, transaction-wide event ordinals, observation ledgers and explicit degraded status |
| A-04 | Medium | Verified WASM bundles omitted the project's license | Package exact LICENSE/NOTICE bytes and checksums; packaging regression verifies their content and checksum identities |
| A-05 | Medium | Open-source decision/release and distribution rules were incomplete | Governance, contribution, conduct, support, licensing and decision policies define roles and boundaries |
| A-06 | Medium | Maintenance lacked a full component schedule, recovery and retirement model | Architecture §26 and MAINTENANCE define owners/backups, budgets, retention, journals, recovery objectives, support and handover |
| A-07 | Medium | Repository implementation status was narrowing the full-project specification | The architecture explicitly specifies the completed project, with implementation status tracked separately |
| A-08 | Low | Wording requested removal of a configuration field already absent from source | Describe the actual single grid/reserve/ceiling schema |

## Findings from the documentation-wide pass

Severity measures the consequence of following inaccurate documentation or
implementing an underspecified design. It is not a vulnerability rating for
deployed contract code.

| ID | Severity | Finding | Applied correction and verification |
|---|---|---|---|
| D-01 | Medium | The residual-risk list still said a winner could delay/abandon delivery, contradicting public `claim_bid` | DESIGN_AUDIT now distinguishes permissionless claims from undelivered allocations caused by absent calls or blocked transfers; checked against the actual resolution path |
| D-02 | Medium | The consolidated report described all four cryptographic regressions as signed-auth tests | Identify three genuine-signature paths and one commitment-domain test using exact mocks; retain the two additional classic-authority cases and local-host boundary |
| D-03 | Medium | Security/review wording could imply external independence or complete correctness | Security policy and contract-review narrative identify separate internal passes and finite checked cases without an external certification claim |
| D-04 | Medium | TTL and credit descriptions used unconditional statements about absence of financial risk | Distinguish preserved ownership from deadline availability, restoration cost, permissions and backing; deployment balance statements are explicitly dated |
| D-05 | Low | Setup instructions recommended Node.js 18+, including unsupported releases | Use Node.js 22 LTS consistently with the configured CI; do not change historical test measurements |
| D-06 | Low | The operations restoration recipe could imply every archive access requires a separate transaction | Describe supported inline restoration and conditional separate restore/resimulation, preserving deadline behavior |
| D-07 | Low | The testing artifact list omitted LICENSE/NOTICE and publication could reuse a directory with stale files | Align the artifact list/checksums and require a clean staging directory with an exact reviewed file set before public upload |
| D-08 | Medium | Recovery import had no fixed KDF/cipher profile or pre-derivation resource limits | Specify the target Argon2id/AES-GCM profile, authenticated context, 64-KiB single-bid bound, strict profile/field validation and acceptance cases |
| D-09 | Medium | External metadata/document retrieval lacked an explicit network-access boundary | Specify an isolated HTTPS fetcher, destination/redirect validation, internal-address blocking, resource limits and independently configured signing/onboarding origins |

These documentation corrections are applied. D-08/D-09 define requirements for
application components that do not yet exist; they do not claim a vault or
fetcher implementation has passed security tests.

## Evidence and validation

The 106 recorded Testnet references have contiguous IDs, unique 64-character
transaction hashes and matching displayed prefixes. All 14 recorded contract
build-input hashes match current files. Source contains 60 integration-test
definitions and nine shared-unit-test definitions; these counts are not a fresh
execution of those Rust suites.

Reproducible checks for this documentation pass:

```bash
python3 scripts/check_docs.py
python3 -m unittest discover -s tests/design -q
python3 -m unittest discover -s tests/tooling -q
node tools/design/check_commitment.mjs
python3 tools/design/generate_contract_vectors.py --check
git diff --check
```

Additional source checks passed for all 51 exported contract signatures
(including reserved constructors), 22 error codes, 22 Config fields and their
documented declaration order, internal heading references and deployment-record
identities. No contract source, canonical license text or deployment identity
is changed by this pass.

| Check | Result on this pass |
|---|---|
| Python design suite | 50 tests passed, including generated-book/reference comparisons |
| Python tooling suite | 15 tests passed, including exact license/notice bundle checks |
| Commitment vectors | Legacy vector and 36 domain/configuration cases passed |
| Clearing fixtures | All 48 fixtures matched the independent BigInt reference |
| Published documentation | 24 Markdown files inspected; all 38 internal heading links resolved |
| Repository link/fence checker | Passed for 26 local Markdown files; this walk also includes two ignored archive files outside the published audit scope |
| Source and recorded build inputs | 14 recorded input hashes matched; API/schema/error checks passed |
| Transaction-log integrity | 106 contiguous IDs and unique, well-formed hashes; displayed prefixes matched |
| Whitespace | `git diff --check` passed |

The host initially selected Node.js 12.22.9, which could not execute the modern
JavaScript checks. Reruns used an official Node.js 22.23.4 binary in temporary
storage after verifying its archive against the published SHA-256. All model/
vector checks then passed without changing the reference algorithms or fixtures.
The supported Node.js 22 setup instructions now match CI; the
[Node.js release table](https://nodejs.org/en/about/previous-releases) identifies
the supported lifecycle. The failed initial runtime attempts are not counted
as passing tests or contract defects.

## Limits and implementation acceptance

Live Testnet transactions/balances are not re-queried in this pass. Recorded
deployment evidence remains dated. New network-resource/archival acceptance,
fresh native coverage, wallet integration, guest execution and an external
audit are not inferred from passing documentation/model/tooling checks.

The production implementation still needs its generated SDK/dApp, indexer/API,
keeper services, independently reviewed recovery and fetcher implementations,
real account/wallet matrix, provider/network qualification and demonstrated
operational recovery targets. Production releases also need accountable role
coverage, verified hosting controls, signed provenance and complete target-
specific dependency/notice inventories. These are implementation/release gates;
they do not narrow the architecture's completed-project scope.

The full transitive license inventory was not regenerated: the earlier local
metadata attempt required an uncached package in a read-only Cargo cache.
Project license packaging is verified separately from upstream license review.
No updated dependency-advisory result is claimed.

The main architecture retains the final requirements without embedding this
report's finding history. The [documentation index](../README.md) links this
report separately.

