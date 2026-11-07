# Repository, architecture, and security audit — 8 November 2026

Target: full-project architecture **v2.1**, the current `0.1.0` contract
interface, all five contract/shared source files, the complete published
documentation set, repository structure, and CI/release tooling. This is an
internal engineering review, not an independent external audit. It
supplements, and does not replace, the four prior review passes consolidated
in [FINDINGS_AND_FIXES.md](FINDINGS_AND_FIXES.md) and the documentation audit
in [ARCHITECTURE_REVIEW.md](ARCHITECTURE_REVIEW.md).

## Scope and method

Read every contract source file in full
(`contracts/{common,auction,auction-factory,price-registry,oracle-adapter}/src/lib.rs`,
3,231 lines) directly against `ARCHITECTURE.md`, `docs/reference/CONTRACT_API.md`,
and the four existing security review documents, rather than treating those
documents' prior claims as ground truth. Independently traced every stage
transition in the `Auction` contract's state machine (`fund`, `place_bid`,
`commit_bid`, `reveal_bid`, `advance`, `finalize`, `expire`, `claim_bid`,
`expire_bid`, `expire_range`, `withdraw`, `maintain`) against the mermaid
state diagram and prose in `ARCHITECTURE.md` §5. Reviewed the top-level
repository layout, `.github/workflows/ci.yml`, `.gitignore`, and the
documentation index for structural issues (duplication, misplacement,
inconsistent naming, undocumented scripts). Swept every published Markdown
file for references to AI tools, grant programs, or funding applications.

## Findings from this pass

| ID | Severity | Finding | Applied correction |
|---|---|---|---|
| R-01 | Low | `ARCHITECTURE.md` §5's mermaid state diagram omitted the `Funded --> Cancelled` transition entirely. A funded auction whose reveal/placement window ends with nobody ever calling `advance()` sits in `Funded` until `close_deadline`, at which point permissionless `expire()` cancels it with reason `CloseExpired` — exactly like the already-diagrammed `Closing`/`Sweep`/`Ready` cases, but the `Funded` source state had no edge to `Cancelled` at all. | Added `Funded --> Cancelled: deadline, never advanced`. |
| R-02 | Low | The same diagram's `Closing --> Cancelled` edge was labeled "minimum fails or deadline". Tracing the actual control flow shows `Reason::Minimum` is only ever produced inside `finalize()`, which requires `Stage::Ready` — it cannot occur from `Closing`. The real non-deadline cause of a `Closing`-stage cancellation is `Reason::NoDemand`, raised inside `advance()`'s closing walk when cumulative demand is zero at the end of the grid (or, in Buyback, when even the cheapest ask is unaffordable). | Relabeled the edge `Closing --> Cancelled: no demand or deadline`. |
| R-03 | Low | The diagram's `Ready --> Cancelled` edge was labeled only "close deadline", omitting that `finalize()` can cancel a `Ready` auction immediately — before the deadline — for either `Reason::GlobalAuth` (the escrow itself or, in Buyback, the recipient, loses required authorization) or `Reason::Minimum` (actual fill below the configured minimum). Both are real, already-implemented, already-tested transitions; only the diagram omitted them. | Relabeled the edge `Ready --> Cancelled: minimum fails, global auth failure, or deadline`. |
| R-04 | Informational | `SCF46_OPEN_TRACK_SUBMISSION.md` was a tracked, published grant-application draft (funding round, team/eligibility narrative, budget guidance, application-stage tracking). This directly contradicted `docs/README.md`'s own stated scope ("Funding proposals and Word documents are excluded from the published repository") and the project's stated goal of reading as an independent engineering project. The parallel `docs/archive/` material covering the same ground was already correctly `.gitignore`d and never published; only this one file had been committed. | Removed `SCF46_OPEN_TRACK_SUBMISSION.md` from the repository. Confirmed no other published file links to it. |
| R-05 | Informational | The README explained the implemented PoC well but gave a first-time reader no single place to see the complete intended system (contracts plus SDK, dApp, indexer, keepers, governance) or the current repository layout without first opening `ARCHITECTURE.md`. | Added a table of contents, a "the full project at a glance" status table distinguishing implemented from planned layers, and an explicit repository-structure tree to `README.md`. |
| R-06 | Informational | No single document collected every unresolved item toward the full project (as distinct from the already-closed findings in `FINDINGS_AND_FIXES.md`); open items were scattered across `DESIGN_AUDIT.md`'s gate matrix, `ARCHITECTURE.md` §15.1's target paths, and `SECURITY.md`'s hosting-controls table. | Added `docs/FUTURE_WORK.md`, organized by area with priority, status, dependencies, acceptance criteria, and a cross-reference index back to the architecture sections and release gates each item comes from. |

No Critical, High, or Medium-severity finding was identified in this pass.
Independently re-tracing the clearing walk (`advance()`'s `Closing`/`Sweep`
branches), the escrow accounting deltas (`admit`, `resolve`, `cancel`,
`withdraw`), the commitment/reveal path, and the factory/registry provenance
checks against their documented invariants found no disagreement between the
implemented `0.1.0` source and the current architecture, contract API, or
auction-logic documentation beyond R-01–R-03 above.

## Verification report

What this pass independently confirmed by reading source directly (not by
re-citing an earlier report's conclusion):

- **Reentrancy guard.** Every mutating entry point in `Auction`, `AuctionFactory`,
  and `PriceRegistry` calls `enter()`/`leave()` around its body, backed by a
  single persisted `Lock` boolean per contract instance. A nested call into the
  same contract while the lock is held returns `Error::Reentry` rather than
  re-executing. `OracleAdapter` has no mutating entry point beyond `maintain()`,
  which touches no financial state.
- **Per-asset conservation.** `Accounting::balanced()` is checked on every write
  (`save_accounting`) and requires
  `pending + reserve + credit == liability` for each of the two tracked assets
  independently — RWA and quote are never summed together.
- **Clearing direction.** `advance()`'s `Closing` branch walks tick indices
  *descending* from `tick_count(c) - 1` in Sale mode and *ascending* from `0`
  in Buyback mode, matching §7.1/§7.2's descending-demand/ascending-supply
  description exactly, including the budget-capacity formula
  `floor(budget * SCALE / price)` recomputed at each occupied Buyback tick.
- **Non-reveal penalty precedence (the C-19 fix).** `resolve()` classifies
  `b.quantity.is_none()` as `Reason::NonReveal` strictly before checking the
  caller-supplied `expired` flag, and only after checking `Stage::Cancelled`
  first. This matches the documented precedence (cancellation, then
  non-reveal, then delivery-expiry) and was confirmed to apply identically
  whether resolution happens through `claim_bid` or `expire_bid`/`expire_range`.
- **Sealed Sale hidden-quantity reservation.** `admit()` calls
  `mul_div(env, a.admitted_deposit, SCALE, c.min_price, false)` and propagates
  its `Error::Arithmetic` before any transfer, exactly as §6.2 and finding
  C-14 describe; the check runs on the already-updated `admitted_deposit`
  total, so it is cumulative across commits, not per-bid.
- **Buyback recipient exclusion.** Both the constructor and `fund()`
  independently reject a `buyback_recipient` equal to the auction itself, the
  factory, or the registry, matching finding S-01's fix in two separate places
  (construction-time and funding-time), consistent with `fund()`'s own
  comment that an admin transfer between creation and funding must not bypass
  this check.
- **Repository structure.** The top-level directory contains only the
  expected governance/policy documents and standard tooling configuration; no
  duplicated, misplaced, or orphaned file was found. `scripts/check_docs.py`
  passes against the current tree (28 Markdown files, all local links and
  fenced code blocks valid).
- **No AI-tool or grant/funding reference** exists anywhere in the published
  Markdown, Rust, Python, TOML, or YAML sources after this pass (verified by
  a repository-wide case-insensitive sweep).

What remains unverified by this pass, and should not be inferred from it:

- No new compiled-WASM test, Rust unit test, or coverage run was executed as
  part of this pass beyond the existing Python `tests/design` (50 tests) and
  `tests/tooling` (15 tests) suites, which were re-run and passed unchanged.
  `cargo fmt`/`clippy`/the integration suite were not re-run because no Rust
  source changed.
- This pass did not re-verify the 106 Testnet transaction records, the
  deployed WASM hashes, or any on-chain state; it worked entirely from the
  local source tree.
- This pass is not an external audit and does not change the status of
  `FW-901` in `FUTURE_WORK.md` or gate `G-12` in `DESIGN_AUDIT.md`.

## Final change summary

Modified: `ARCHITECTURE.md` (state-diagram edge corrections), `README.md`
(table of contents, full-project status table, repository-structure tree),
`docs/README.md` (index entry for the new future-work register).
Added: `docs/FUTURE_WORK.md`, this report. Removed: `SCF46_OPEN_TRACK_SUBMISSION.md`.
No contract source, test, script, license, or deployment file changed; no
auction pricing, escrow, settlement, refund, or authorization logic was
touched. `python3 scripts/check_docs.py`,
`python3 -m unittest discover -s tests/design -q`, and
`python3 -m unittest discover -s tests/tooling -q` all pass against the
resulting tree.
