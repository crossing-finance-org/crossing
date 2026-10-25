# Contributing

The current implementation scope is the Soroban contracts, their demonstration,
tests, and review documentation. The [architecture](ARCHITECTURE.md) also specifies
future client/application components. Keep changes scoped to an issue or concrete
behavior, and discuss contract economic or interface changes before implementation.

## Development

Use the pinned toolchain in `rust-toolchain.toml` and the checked-in `Cargo.lock`.
Install Rustfmt, Clippy, the `wasm32v1-none` target, Python 3.10+, and Node.js 18+.
The [README](README.md#quick-start) provides the complete setup, and the
[testing guide](docs/TESTING.md) defines the per-source-file 95% coverage gate.
It also requires at least 95% unadjusted raw LLVM line coverage for each deployable
contract; shared-library SDK test-only attribute regions are separately disclosed.

```bash
./scripts/demo.sh
./scripts/check.sh
# With cargo-llvm-cov 0.9.1 and llvm-tools-preview installed:
./scripts/coverage.sh
```

Run `python3 scripts/wasm_artifacts.py build` before individual Rust integration
tests. The loader verifies current contract build inputs and the four WASM hashes
before executing scenarios. The provided scripts enforce this order and honor
`CARGO_TARGET_DIR`. Keep shared fixtures in `tests/support/` and behavior tests
in `tests/scenarios/` within the auction crate. Do not disable default invocation
resource limits to make an oversized contract call pass.

## Contract changes

For behavioral fixes, include a regression that fails before the fix. Check both
asset liabilities independently, preserve refund ownership, and verify the full
invocation rolls back on failure. Keep work bounded and authorization scoped.
Tests of arithmetic alone do not replace genuine SAC/compiled-WASM evidence.
Clearing authorization must cover every price-contributing revealed bid, including
rounded-zero quantities/payments, in both sweep and finalization. Construction
must protect reused code and the instance independently through the accepted
financial horizon. Factory changes need real deployment, exact signed arguments,
pin mismatch, and rollback regressions; the native auction-installation adapter
alone cannot establish factory deployment behavior.
Sealed Sale admission must reserve the aggregate hidden-quantity upper bound
before accepting collateral, including excess collateral. Cover failed admission
rollback and both near-limit reveal orders when changing those bounds.

Changes to `Config`, commitment domains, stored contract types, constructor pins,
errors, or events affect compatibility. Update [architecture](ARCHITECTURE.md),
[contract API](docs/reference/CONTRACT_API.md), and canonical fixtures together.
Never silently regenerate expected values to accept a failing implementation.
The independent commitment matrix covers every Config and commitment component;
keep its Node encoder and fixed Rust expectations in agreement without deriving
expected hashes from the production Rust helper. Include real signed credentials
for changes to authentication, nonce/expiry handling, or invocation arguments;
mock invocation trees alone cannot establish signature verification.
The contract clearing fixtures are derived from the independent BigInt reference:

```bash
python3 tools/design/generate_contract_vectors.py --check
# Only after reviewing an intended change:
python3 tools/design/generate_contract_vectors.py
```

Keep the 16-bid cap unless a separate reviewed resource measurement supports a
change. Record the fixture, account types, artifact identity, branch measured,
and applicable limits alongside any performance claim.

## Pull requests and security

Explain the problem, final behavior, validation, and material limitations. Include
reproducible seeds/books for economic failures. Run the complete check script and
keep generated artifacts, snapshots, credentials, and reveal secrets out of Git.

Follow [SECURITY.md](SECURITY.md) for potential vulnerabilities. Current internal
reviews are not external audits. The project is licensed under
[Apache-2.0](LICENSE); a contribution is accepted under that same license by
default (see [LICENSE](LICENSE) §5), with no separate contributor agreement.

Use the [documentation ownership map](docs/README.md) when updating behavior,
operations, or review records. Keep policy details in their owning document and
link to them from summaries. Describe fixes as applied until a commit/merge is
actually verified. Update measured test counts/coverage only from completed runs,
retain raw coverage evidence, and distinguish local results from hosted CI or deployment.
Keep funding proposals and Word documents outside published source history;
the repository's technical documentation must remain usable without them.

Changes to coverage tooling require the negative report tests in `tests/tooling/`.
Keep nonempty source/function evidence, raw deployable-contract thresholds, and
precise SDK exclusions enforced. Synthetic policy fixtures are separate from
contract coverage; never replace actual LLVM profiles with them.
