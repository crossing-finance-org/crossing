# Changelog

## Unreleased — 0.1.0

- Split the integration harness into shared support and behavior scenario modules.
  Build and verify test WASMs against current source inputs in Cargo's configured
  target directory, rejecting stale or altered artifacts. Preserve the four
  tested WASMs, checksums, and build metadata together in CI. Define documentation
  ownership and distinguish applied working-tree corrections from merged fixes.

- Align the Python lifecycle model and active documentation with the deployed
  C-19 policy: cleared non-reveals retain their penalty after delivery expiry;
  cancelled auctions refund fully. Strengthen the existing both-mode regression
  to cover cancelled auctions resolved after expiry as well.
- Correct seven-decimal Testnet amounts, recover transaction 100's missing hash
  from ledger 5368459, and record the 25 October read-only verification. Bind the
  Testnet manifest to the immutable contract source revision, lockfile, build
  inputs, network identity, and observed deployment transactions. These changes
  leave all four deployed contract WASM hashes unchanged.

- Select the Apache License, Version 2.0 for the project (see
  [LICENSE](LICENSE)); add `license.workspace = true` to every crate. A
  permissive license with an explicit patent grant matches the project's
  stated goal of being open infrastructure other wallets, exchanges, and
  protocols can integrate or build on.
- Deploy the post-C-19-fix contracts to Stellar Testnet and exercise them
  through 100+ real on-chain transactions covering both auction modes (Sale,
  Buyback), both bid modes (Open, Sealed — the latter using independently
  computed commitment hashes), a live non-reveal-penalty resolution after the
  delivery deadline (directly exercising the C-19 fix), a live mid-auction
  authorization revocation and clearing restart, missed-funding-window and
  missed-close-deadline recovery via `expire`, permissionless `maintain()`
  TTL renewal on all four contract types, and full registry/oracle
  publication. See [deployments/testnet.json](deployments/testnet.json) and
  [deployments/TESTNET_TRANSACTIONS.md](deployments/TESTNET_TRANSACTIONS.md)
  for addresses, hashes, and every transaction's hash.
- Add a single consolidated [findings and fixes report](docs/security/FINDINGS_AND_FIXES.md)
  gathering every finding from every review pass (design, second design,
  contract, cryptographic) with a plain-language explanation of each defect
  and its fix, so a reader does not need to open all four source reports.
- Fix a Medium-severity timing bypass of the non-reveal penalty found by a
  fresh, independent three-agent re-audit: `resolve()` now classifies
  `NonReveal` ahead of the timing-derived `DeliveryExpired` reason on any
  auction that cleared, so the penalty applies identically whether a bid is
  resolved via `claim_bid` before the delivery deadline or `expire_bid`/
  `expire_range` after it. Previously, simply letting the deadline pass
  unresolved defeated the penalty entirely. A cancelled auction (which never
  cleared) still refunds in full. Updated the test that had encoded the old,
  bypassable behavior as intended, and `ARCHITECTURE.md` §6.3 accordingly.

- Add real classic-current-admin create/fund authorization in both auction modes
  and genuine 2-of-2 multisig rejection/nonce-rollback/delivery/refund coverage.
- Measure maximum Buyback and late-exclusion finalizers with canonical ledger,
  event, return, key, and entry bytes, followed by complete settlement/recovery.
- Harden the 95% coverage gate against empty or duplicate source records, missing
  function mappings, invalid counters/regions, and stale LLVM diagnostics. Remove
  earlier reports before a run and merged profiles in reuse mode; add 9 tooling
  regressions. Current totals: 60 integration, 9 shared unit, 50 design, 9 tooling.

- Reserve Sealed Sale hidden-quantity capacity before collateral transfer, so
  every accepted valid reveal fits aggregate i128 bounds in either order.
- Force OracleAdapter instance/reused-code retention; add permissionless
  `maintain()` and reject native XLM or unsupported bases at construction.
- Add real classic-account and custom-account Ed25519 credentials, signed-call
  mutation/nonce/expiry/replay tests, authenticated callback re-entry rollback,
  and a 36-case independently encoded commitment matrix.
- Exercise penalty precedence, rounding and wide values in both modes; expired
  snapshot recovery; quote-pair and publication-failure isolation; and all 16
  revocations on a 4096-tick grid in 2,200 bounded continuation calls.
- Align architecture v1.6, API, operations, security reports, README, and test
  evidence with the 22 October 2026 review. Current totals are 56 integration
  tests, 9 shared unit tests, and 50 design tests; execution modes are not counted
  as additional independent tests. Network acceptance and external review remain
  separate release requirements.

- Add a fourth deployable contract, `OracleAdapter`: a read-only, per-base-asset
  partial SEP-40 surface (`base`, `decimals`, `resolution`, `lastprice`) over the
  existing price registry. Explicitly does not implement `assets`, `price`, or
  `prices`; consumers needing historical point-in-time queries should read the
  registry directly.
- Add an optional, bounded non-reveal penalty for Sealed auctions
  (`Config.nonreveal_penalty_bps`, capped at 50%, zero by default and forced to
  zero in Open mode). Applies only to bids resolved with reason `NonReveal`;
  every other refund path is unaffected. The penalty is credited to the issuer
  through the existing credit mechanism, preserving conservation by
  construction.
- Regenerate the canonical config/commitment XDR vectors for the new `Config`
  field and reconfirm Rust/JavaScript agreement byte for byte.
- Optimize the auction contract's internal `store`/`leave` helpers to take the
  caller's already-fetched `Config` instead of re-reading instance storage on
  every call; `admit` alone previously repeated this read up to four times per
  bid placement. Pure refactor, no behavior change, full suite and coverage
  gate re-verified.
- Four-agent independent re-audit (common, auction, factory/registry/adapter,
  tests) found zero Critical/High findings. Addressed every Low/Medium
  finding raised: an explicit `now < start_ledger` guard in `fund()` (removes
  a fragile implicit dependency on `validate_config`'s reuse); documentation
  of previously-undocumented preconditions/trade-offs in `tick_count`,
  `allocation`, `validate_config`'s deadline spacing, the registry's
  same-ledger tie-break, and the oracle adapter's cross-contract immutability
  assumption; a new integration test for a full 16-bid sealed book with mixed
  reveal/non-reveal outcomes swept across multiple `advance()` batches; a new
  regression test locking in that `withdraw`'s solvency check is intentionally
  global, not per-asset; a new case covering `maintain()` batch atomicity when
  a valid key is mixed with an invalid one; and new direct unit tests in
  `common` for `validate_config`'s bound checks, `financial_ttl`'s boundary,
  `tick_count`/`tick_index`, `penalty`, and `allocation`'s anti-dust rule.

- Implement immutable Soroban auction factory, per-auction escrow/clearing, and
  verified public clearing/execution registry.
- Support issuer sales and budgeted buybacks, open and sealed funded bids,
  bounded closing, three-stage SAC compliance, delivery, credits, and expiry.
- Provide a local compiled-WASM demonstration, genuine SAC integration tests,
  independent economic reference models, and canonical commitment vectors.
- Document protocol invariants, implementation review, full application
  architecture, and the boundary between local evidence and production readiness.
- Correct initial shared-auction code retention and exact-maximum financial TTL
  validation; add factory bootstrap, authorization, namespace, atomic deployment
  rollback, and registry same-ledger ordering regressions.
- Include zero-rounded marginal bids in clearing authorization checks, preserving
  price integrity after revocation; cover zero-payment and large-value books.
- Align architecture v1.5 and the full documentation set with source `0.1.0`:
  exact wire/storage names, factory bootstrap and retention, registry publication
  eligibility, current review evidence, and separate outstanding release gates.

Deployed to and exercised on Stellar Testnet as of 24 October 2026 (see
[deployments/](deployments/)). No Mainnet deployment or external audit has
occurred.
