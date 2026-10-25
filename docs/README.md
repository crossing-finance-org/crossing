# Documentation

Current baseline: source `0.1.0`, architecture **v1.6**. Evidence/model review:
**25 October 2026**; repository structure follow-up: **25 October 2026**.
Four deployable smart contracts and the shared Rust library are implemented.

| Document | Purpose |
|---|---|
| [Project README](../README.md) | Product, implemented scope, setup, demonstration, checks, and repository map |
| [Technical architecture](../ARCHITECTURE.md) | Full system: contract economics/ABI, client SDK, dApp, data services, operations, and security |
| [Contract API](reference/CONTRACT_API.md) | Current entry points, authorization, errors, and types |
| [Testing and coverage](TESTING.md) | Suite layers, independent fixtures, native measurement, and the per-file 95% coverage gate |
| [Working demonstration](POC_DEMO.md) | Reproducible local scenarios with asserted outcomes |
| [Findings and fixes](security/FINDINGS_AND_FIXES.md) | Every review finding across all passes, in one place, with a plain-language explanation of each defect and its fix |
| [Contract review](security/CONTRACT_REVIEW.md) | Implementation findings, runtime evidence, and remaining qualification work |
| [Auction logic review](security/AUCTION_LOGIC_REVIEW.md) | Clearing invariants, price-contributor compliance, escrow conservation, rounding, and sealed-bid limits |
| [Cryptographic review](security/CRYPTOGRAPHIC_REVIEW.md) | Canonical commitments, independent vectors, real Ed25519 credentials, replay/expiry, and authentication callback rollback |
| [Design review](security/DESIGN_AUDIT.md) | Design findings, corrections, invariants, and release gates |
| [Operations](operations/README.md) | Build identity, deployment sequence, maintenance, and incident response |

The architecture describes the full intended product. Contracts and their local
demonstration are implemented; client SDK, dApp, indexer, keepers, and curated
oracle integration are planned. The generated WASM specification and the source
define current wire types. Resolve inconsistencies through source review and a
documented compatibility change before release.

Document ownership: the Contract API owns current wire behavior and authorization;
the architecture owns economic rules and the planned full system; Testing owns
suite counts, coverage methods, and dated measurements; Operations owns build,
release, and recovery procedures. Findings and Fixes owns finding status and
links to supporting reviews. The deployment manifest owns recorded deployment
identity, and the transaction log owns dated on-chain activity. README is an
overview and links to these owners rather than duplicating detailed policy.
Historical measurements keep their original dates. A fix present in the working
tree is described as applied; that does not assert it has been committed or merged.

For a first review, read the project README, run the demonstration, then use the
contract API and auction logic review to inspect the implementation. The testing
guide owns the detailed coverage methodology and measured totals. The design
review separates local evidence from outstanding release qualification. Funding
proposals and Word documents are excluded from the published repository.
