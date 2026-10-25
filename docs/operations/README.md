# Build, release, and recovery operations

These runbooks describe the intended full-system operations. The repository
currently provides a local smart-contract PoC and CI configuration; it contains
no production addresses, deployment credentials, hosted keeper, or deployment
automation. Publishing repository changes does not deploy a contract.
Baseline: source `0.1.0`, [architecture v1.6](../../ARCHITECTURE.md).
The [documentation index](../README.md) tracks review dates and ownership.
SDK/dApp/keeper procedures describe planned services.

## Local build identity

```bash
./scripts/check.sh
./scripts/demo.sh
# With the pinned coverage tools installed:
./scripts/coverage.sh
python3 scripts/wasm_artifacts.py package
sha256sum Cargo.lock
```

The verified build used by these scripts honors Cargo's configured target
directory. `python3 scripts/wasm_artifacts.py directory` prints the WASM location;
`verify` checks its recorded source inputs and four artifact hashes. Packaging
writes the four WASMs, checksums, build manifest, lockfile, workspace manifest,
and toolchain file to `<Cargo target directory>/verified-contracts/`.
CI preserves this bundle as `tested-contracts-<commit>` for 14 days after checks
pass. The build manifest records dirty working trees explicitly; its hashes
identify local inputs without claiming those edits have been committed or merged.

Use Rust 1.94.1, the repository toolchain file, Soroban SDK 25.3.2, the locked
dependencies, and `wasm32v1-none`. Record source revision, build environment,
compiler/CLI versions, lockfile hash, profile, and WASM hashes together. A fresh
dependency/advisory review is separate from an offline build. Do not publish
generated snapshots, wallet material, or local environment files.

Coverage starts by removing earlier reports and uses `target/llvm-cov-target/`
for instrumentation. Keep `export.stderr`, `html.stderr`, raw JSON, and the
production summary together. Missing/empty/duplicate source evidence or LLVM
profile/source mismatches fail the run; do not substitute an earlier passing
summary. Reuse mode deletes both old raw profiles and merged data before running.

The GitHub workflow runs the same checks and demonstration with read-only
repository permissions. External actions are pinned to commit hashes, including
[checkout](https://github.com/actions/checkout/commit/11bd71901bbe5b1630ceea73d27597364c9af683)
and [Node setup](https://github.com/actions/setup-node/commit/49933ea5288caeca8642d1e84afbd3f7d6820020).
Dependency-update pull requests require review and complete checks. Workflow
execution on GitHub has not been verified by the local test run.

## Deployment preparation and order

Before any deployment, complete the applicable
[release gates](../security/DESIGN_AUDIT.md#5-open-implementation-and-deployment-gates).
Select the independent review process, network, asset controls, fee/rent
policy, and tested transaction limits (the software license is already
settled: [Apache-2.0](../../LICENSE)). Do not treat the local resource
fixture as a target-network simulation. Run a testnet pilot first.

1. Build and verify all four WASM hashes against the intended source and lockfile.
2. Choose the factory deployer and salt; compute its future address using the
   network and deployment preimage. Deployment keys are operational keys, not
   escrow upgrade/custody keys.
3. Deploy the registry atomically with `(predicted_factory, factory_wasm_hash,
   auction_wasm_hash)` as constructor arguments.
4. Deploy the factory at the predicted address with `(registry, registry_wasm_hash,
   auction_wasm_hash)`. Its constructor verifies the mutual bindings and code.
5. Read both `bindings()` results and actual executables from chain. Record
   transaction hashes, source ledgers, network passphrase/ID, addresses, hashes,
   constructor inputs, and tested limits in a versioned release manifest.
6. Have the client SDK/dApp authenticate this manifest before enabling signing.
   Do not point users to an alternate factory merely because it has the same ABI.
7. Deploy one `OracleAdapter` per quote asset consumers need SEP-40-shaped reads
   for, with `(registry, registry_wasm_hash, base)` as constructor arguments.
   Require a genuine seven-decimal issued-SAC base; include the adapter's
   permissionless `maintain()` in TTL operations separately from registry ranges.
   This is optional, independent of factory/registry deployment, and can be
   deployed, redeployed under a new address, or skipped entirely without
   affecting any auction: it only ever reads already-published registry state.

The registry cannot verify an undeployed future factory at its own constructor;
the factory completes the mutual check on deployment. An incomplete deployment
has no authorized escrow inventory and must not be advertised as an active release.
Constructor entry points cannot be invoked later to repair pins or reset storage.
The [factory runtime tests](../../contracts/auction/tests/lifecycle.rs) cover
four mismatched bootstrap pins, same-salt retry, constructor reinvocation, exact
create authorization, and post-deployment registration rollback.

## Opening an auction

1. Verify both assets are genuine issued SACs with seven decimals; native XLM and
   arbitrary tokens are unsupported. Confirm current RWA SAC administration and
   any distinction from the classic issuing account.
2. Review immutable pair, mode, recipient, price grid, supply/budget, actual minimum,
   bid/deposit minimums, 16-slot cap, and all ledger deadlines. For a buyback,
   explicitly identify whether delivery to the classic issuer will burn tokens.
3. Predict/create the dormant auction with issuer authorization and verify its
   factory registration, template/config hash, and registry binding. The issuer
   authorizes the exact config and salt; prediction reserves nothing. Salt
   derivation binds the network, factory, issuer, and user salt.
4. Obtain asset-admin authorization for the escrow on both SACs. Verify issuer
   balances/permissions and buyback recipient authorization. No automatic
   authorization bypass is available to the factory.
5. Before the start ledger, fund exactly once from the issuer. Confirm Funded
   state and the appropriate per-asset reserve/liability. Failed funding is not
   an open auction. If it remains unfunded at start, anyone can expire it.

## Closing, delivery, and publication

- After placement/reveal, call bounded `advance`, observing persisted stage,
  cursor, and epoch. When Ready, call `finalize`; an exclusion is successful
  progress requiring another clearing pass, not a terminal transaction failure.
  Both closing checks include marginal price contributors rounded to zero.
  Definite revocation removes the full revealed quantity; query errors abort and
  must be retried/restored, without treating the bidder as permanently revoked.
  If closing is unfinished at its deadline, call `expire` to enable refunds.
- Before delivery expiry, resolve individual bids. Reconcile returned resolution,
  actual transfers, credits, reserves, and liabilities. Earned issuer proceeds are
  withdrawable credits; undelivered backing must stay reserved.
- At the delivery deadline, resolve remaining bids using `expire_bid` or
  `expire_range` batches of at most 8. Overlapping batches skip resolved bids.
  A deadline alone does not execute refund accounting or pay token transfers.
- Publish `record_clearing` independently for a successful positive clearing
  (`Cleared` or `Settled`). Once that auction reaches `Settled`, publish
  `record_settlement`, including zero actual execution. Cancelled/refunded auctions
  have no successful clearing and are not registry sources. Publication retries
  are idempotent and cannot block claims. Compare allocated versus actual volume;
  preserve source ledger/time separately from publication time.
- Withdraw a credit only with its owner's authorization and original asset. A
  failed/frozen transfer leaves the credit intact. Neither keepers nor issuers
  can change the recipient to make an otherwise forbidden withdrawal succeed.

## TTL and restoration

Monitor instance, code, bid/tick indexes, nonces, credits, factory provenance, and
registry records. Mutation extends relevant keys; maintenance accepts bounded
recognized keys/ranges. Keep release manifests and indexed source references so
operators can identify records that need restoration.

The initial auction financial target is `delivery_deadline + 120960`, checked
against overflow and the current maximum TTL. TTL excludes the current ledger:
`live_until = current + extend_to`. An exactly supported maximum is accepted.
Construction forces instance and shared auction WASM independently to the full
financial horizon; later rolling extensions use a half-horizon threshold and
a 518,400-ledger retention target capped by current `max_ttl()`. The fixed recovery
margin is not reduced to fit; an unsupported financial schedule is rejected.
An older reused code entry cannot be assumed to inherit the new instance's TTL
automatically.

| Contract | Maintenance request | Retained dependencies |
|---|---|---|
| Auction | `maintain(Vec<MaintenanceKey>)`, 1–8 typed keys | Requested known records plus instance/code |
| Factory | `maintain(start,count)`, count 1–8 | Selected `At(id)` and `Auction(address)` plus instance/code |
| Registry | `maintain(rwa,quote,start,count)`, count 1–8 | Selected history and existing clearing/execution records, pair count/latest pointers, instance/code |
| OracleAdapter | `maintain()` | Adapter instance/code only; fixed registry/base bindings |

Factory metadata and registry records use their own retention horizon; an auction's
financial horizon does not automatically retain those dependencies. Maintain all
needed ranges and records referenced by latest pointers. Renewing a pointer alone
does not renew an observation outside the selected range. Funding/publication can
require restored provenance; claims on an already funded auction do not call
factory/registry. Credits have no forfeiture deadline after recovery retention ends.

Before accessing an archived dependency, simulate restoration using the target
network's supported transaction flow. Restore the original ledger entries and
their values. Never initialize a missing archived bid/credit/nonce as a new record,
recreate ownership, clear a replay guard, or interpret missing RPC history as an
empty auction. Rerun the intended operation after restoration and verify the
result. Actual archive/restore acceptance tests are still an open release gate.
An expired local snapshot regression now preserves credits, bid nonces, factory
metadata, and registry duplicate guards beyond the delivery/recovery horizon.
It uses the pinned test host's recording-mode automatic restoration; target
network restoration submission, fees, footprints, and dependency discovery
still need acceptance testing.

## Freeze, deficit, or infrastructure incident

| Observation | Response |
|---|---|
| Bidder authorization revoked | Exclude price contributors before clearing, including rounded-zero bids; revealed revoked bids refund fully. On a cleared auction, unrevealed bids retain the configured NonReveal penalty before and after delivery expiry. Query failure is not revocation. |
| Refund withdrawal frozen/full trustline | Preserve credit, explain the asset restriction, and retry after the asset/account condition changes |
| Escrow balance below liability | Stop outflows through the solvency guard; preserve bookkeeping/expiry and seek external replenishment; no first-claimant payout |
| Escrow/recipient authorization revoked | Permit deterministic cancellation/refund bookkeeping; await authorized transfers for withdrawals |
| Closing cannot finish by deadline | Use permissionless expiry; never extend immutable deadlines off-chain |
| RPC/indexer unavailable | Show unknown/stale state; switch only to a verified network provider and reconcile transaction hashes before retry |
| Registry publication unavailable | Continue escrow settlement and retry publication later without changing source time |
| Defect in an immutable release | Identify affected hashes/addresses, disable new hosted starts where possible, publish accurate information, and preserve existing ownership paths |

Hosted applications/keepers may stop initiating new operations during an incident.
They cannot pause immutable contracts or compel asset administrators to restore
permissions/backing. No promised recovery transaction can override contract rules.
Keep secrets out of diagnostics; log operation, network, contract, ledger,
transaction hash, and classified error. Preserve evidence for independent review.
