# Licensing and distribution

Crossing's original code, tests, scripts and repository documentation use the
Apache License, Version 2.0 unless a file explicitly identifies another license.
The complete terms are in [LICENSE](LICENSE); the SPDX identifier is
`Apache-2.0`. All five Rust workspace crates inherit that declaration from
`Cargo.toml`. They currently set `publish = false`; no published package or
production release is implied.

## Reuse and contribution terms

Apache-2.0 permits use, modification and redistribution, including commercial
use, subject to its conditions. Distribution preserves the license, applicable
notices and attribution, and identifies modified files. The license includes
patent terms, warranty limitations and no general trademark permission. The
license text governs these terms. [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0).

Contributors retain their copyrights. Intentionally submitted contributions
are accepted under the project's existing license as described in LICENSE §5,
unless separately agreed. No copyright assignment, separate contributor agreement
or mandatory commit sign-off is introduced by this PoC. Submit only material
you have the right to contribute and identify external material and its license
in the pull request. See [CONTRIBUTING.md](CONTRIBUTING.md).

The bracketed example copyright line in LICENSE's standard appendix is part of
the canonical license text. It is not an instruction to invent a copyright
owner or rewrite that license. Do not replace a contributor's existing notice
with a project-wide ownership claim.

## Source and binary distributions

The repository includes a project [NOTICE](NOTICE). Carry LICENSE and NOTICE
with source archives, downloadable WASM bundles and future SDK/dApp packages.
NOTICE is attribution information and does not add license restrictions.

Before making a distributable artifact available:

1. Identify its exact source revision, dependency lockfiles, target and enabled
   features. Separate runtime/linked code from development-only tools.
2. Inventory incorporated dependencies, vendored code, fonts, images and other
   assets. Record each origin, version, license expression and notice requirements.
3. Preserve required third-party texts and notices in that artifact. A root
   Apache-2.0 declaration does not relicense dependencies or third-party assets.
4. Include applicable project notices and a record of modifications. For future
   packages, set an SPDX license field and verify packaged file lists before
   publishing; do not assume the source checkout and package contain the same files.
5. Assemble the distribution in a clean staging directory and verify its exact
   allowlisted file set before upload. A reused build output directory can contain
   stale files that the packaging helper did not create or verify.
6. Retain the inventory and checksums with release evidence. Review missing,
   unknown or incompatible terms before including the material.

The current artifact helper includes the project's license and notice. It does
not produce a complete dependency license inventory or certify the contents of
linked third-party code. That inventory and any required upstream notices remain
part of a public binary release review. Do not label an incomplete inventory as
an approved software bill of materials.

## File identification and external material

New original source files should use the appropriate language's short SPDX
comment where practical, such as `SPDX-License-Identifier: Apache-2.0`.
Preserve third-party identifiers. JSON fixtures without comments can be described
by their containing directory and distribution manifest. Do not modify deployed
contract source merely to add cosmetic headers without rebuilding and verifying
the resulting artifacts. [SPDX identifiers](https://spdx.dev/learn/handling-license-info/).

Screenshots, issuer documents, logos, asset metadata and generated client code
must have an identified origin and license. A token issuer's permission to trade
does not confer rights to redistribute its documents or branding. Secrets and
private verification data never belong in a source distribution.

## Independent operation

The software license does not supply a hosted service, audit certification,
asset backing, legal permission to issue an instrument or an entitlement to
the original project's domains and accounts. Operators publish their own
deployment identity, support policy and required asset permissions. Preserve
provenance and make changes clear when distributing a fork.

