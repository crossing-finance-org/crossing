# Getting help

Crossing is a best-effort open-source PoC, not a hosted production service.
There is no promised response time, asset recovery service or maintenance term.

Start with [README](README.md), the [demonstration](docs/POC_DEMO.md),
[Contract API](docs/reference/CONTRACT_API.md) and
[operations guide](docs/operations/README.md). The
[architecture](ARCHITECTURE.md) describes the full target system and distinguishes
it from components implemented today.

Use a repository issue for a reproducible non-security bug, documentation
question or scoped feature proposal. Include revision, toolchain, command,
expected/observed result and a minimal local example. Label Testnet observations
with network, ledger and public transaction hash. Avoid duplicate reports when
an existing issue already covers the problem.

Do not include keys, recovery phrases, unrevealed bid material or private
verification data. Potential vulnerabilities use [SECURITY.md](SECURITY.md);
conduct concerns use [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Repository maintainers cannot reverse completed transfers, redirect owned
credits, restore a lost reveal secret or override asset administrators.
Permissionless expiry and credit withdrawal still require valid transactions,
asset permission and sufficient backing. A contract fix is a new release, not
an upgrade of existing escrow.

For contribution/review procedure see [CONTRIBUTING.md](CONTRIBUTING.md) and
[GOVERNANCE.md](GOVERNANCE.md). Support scope, maintenance procedures and service
retirement are defined in [MAINTENANCE.md](MAINTENANCE.md). Current maintainers
are listed in [MAINTAINERS.md](MAINTAINERS.md).

