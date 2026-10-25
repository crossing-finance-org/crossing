# Security policy

Crossing is a smart-contract PoC, deployed to and exercised on Stellar
Testnet (see [deployments/](deployments/)). There is no Mainnet deployment,
supported production release, external contract audit, or bug bounty. The
current source version is `0.1.0`; changes before a reviewed release can break
interfaces and stored types.

## Reporting

For a potential vulnerability, use the repository's private vulnerability report
option in the GitHub Security tab when it is enabled. If it is unavailable, ask
the repository maintainer for a private reporting channel without posting exploit
details publicly. No dedicated security email has been configured in this
repository. Never include private keys, wallet recovery material, or reveal secrets.

Provide affected source revision/contracts, preconditions, a minimal reproduction,
expected versus observed effects, and the affected assets/invariants. Prefer a
local failing test over interaction with live assets. There is no promised response
SLA or reward program.

## Review boundaries

- [Consolidated findings and fixes](docs/security/FINDINGS_AND_FIXES.md) — every
  finding across all review passes, in one place, in plain language
- [Internal contract review](docs/security/CONTRACT_REVIEW.md)
- [Auction economic logic and settlement review](docs/security/AUCTION_LOGIC_REVIEW.md)
- [Cryptographic encoding and authorization review](docs/security/CRYPTOGRAPHIC_REVIEW.md)
- [Design findings, corrections, and release gates](docs/security/DESIGN_AUDIT.md)
- [Technical architecture and threat model](ARCHITECTURE.md)
- [Measured testing/coverage boundaries](docs/TESTING.md)

The 20 October 2026 review covered all three contracts and shared code then
implemented. Confirmed issues in zero-rounded marginal-demand authorization and
financial TTL handling were corrected and regression-tested. Factory
deployment, issuer authority, salt isolation, and registration rollback have
direct runtime tests. The 21 October 2026 addendum reviewed two further
additions — a fourth, read-only `OracleAdapter` contract and an optional,
bounded non-reveal penalty — against the same standard; see the
[contract review addendum](docs/security/CONTRACT_REVIEW.md#addendum--21-october-2026-oracleadapter-and-the-optional-non-reveal-penalty).
This records known fixes, not a guarantee that every vulnerability has been
discovered.

The 22 October 2026 re-review fixes Sealed Sale hidden-quantity capacity overflow,
adapter retention/maintenance, and unsupported native-SAC adapter bases. It adds
real classic/custom-account Ed25519 authorization and re-entry rollback tests,
36 independent commitment vectors, expired snapshot ownership recovery, maximum
grid exclusions, penalty precedence, and registry pair/outage isolation. These
are local-host checks; snapshot automatic restoration and signed authorization
entries do not establish network restoration or transaction submission.

The follow-up adds classic-current-admin and 2-of-2 multisig signatures,
maximum Buyback/late-exclusion resource and byte measurements, and C-18 coverage
evidence hardening. Empty/duplicate records, missing function maps, invalid
regions/counters, and mismatched LLVM diagnostics fail the unchanged 95% gates;
old reports/merged profiles cannot supply a passing failed-run result. This
correction protects test evidence and changes no deployed contract behavior.

A 24 October 2026 independent re-audit found and fixed one Medium-severity
timing bypass of the non-reveal penalty (`resolve()` now classifies a
never-revealed bid ahead of the timing-derived expiry reason on any auction
that cleared, so the penalty cannot be dodged by simply letting the delivery
deadline pass unresolved). Every other re-audited area was confirmed correct
as-is. See the
[consolidated findings report](docs/security/FINDINGS_AND_FIXES.md#phase-3--contract-implementation-review)
for the full explanation (finding C-19) and the
[contract review addendum](docs/security/CONTRACT_REVIEW.md#addendum--24-october-2026-independent-re-audit-penalty-timing-bypass)
for the exact code change.

Fully funded bids remove unfunded-order risk. SAC administrators can still freeze
or claw back assets. Refund credits preserve ownership but cannot force a forbidden
transfer or replace missing collateral. Raw auction observations can be stale or
manipulated and are not automatically suitable collateral prices.

Production qualification requires current dependency review, target-network
resource and restoration tests, independent review/remediation, and verification
of deployment manifests and asset controls. Repository workflows do not deploy.
