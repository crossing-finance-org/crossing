# Auction logic review

Reviewed **22 October 2026** against [architecture v1.6](../../ARCHITECTURE.md),
the auction/shared source, and the local Soroban test suite. This is an internal
implementation review. External audit, actual network restoration, and target
network resource qualification remain release requirements.

## Corrections from this review

| Finding | Correction |
|---|---|
| C-10 — Medium: revoked marginal demand can still affect price when its individual allocation rounds to zero | Both preliminary sweep and atomic finalization now check every revealed, nonexcluded bid at or better than the candidate price. Full quantity is removed on definite revocation, and clearing restarts. |
| C-07 — Medium: reused auction WASM can expire before a new auction's accepted financial horizon | Construction forces both instance and shared-code retention through the full horizon before using rolling extension thresholds. |
| C-08 — Low: a supported exact-maximum TTL schedule is rejected | Relative TTL excludes the current ledger; remove the extra ledger and preserve maximum-plus-one rejection. |
| C-14 — Medium: hidden Sale quantities can overflow after commitments have been funded | Admission reserves `floor(total_deposits*SCALE/min_price)` within i128, before transfer or nonce/slot writes; valid near-limit reveals succeed in either order. |

Each issue has a runtime regression that failed against the previous
implementation. See the [contract review](CONTRACT_REVIEW.md) for the full finding
record, factory review, measured resources, and test evidence.
These corrections are implemented and verified in the current 60-test
compiled-WASM suite. The test/coverage totals and exact reproduction commands
are maintained in the [testing guide](../TESTING.md).

The C-10 counterexample uses sale inventory 3 and bids `2@3`, `1@2`, `1@2`.
The nominal marginal remainder is 1, split over two bids; both individual fills
floor to zero. The earlier implementation checked neither marginal owner's
authorization even though their demand selected price 2. The equivalent buyback
with budget 6 and asks `2@1`, `1@2`, `1@2` repriced the valid seller to 2 using
revoked demand. With the fix, both revoked quantities are excluded and the valid
book is recomputed. Prices here are whole quote units; amounts are base units.

## Clearing invariants

**Sale priority and funding.** Descending price traversal fully allocates better
prices and applies one common ratio at the marginal price. If inventory is not
fully demanded, all bids fill at the issuer's reserve. For every bid,
`0 <= fill <= quantity` and `clearing_price <= bid_price`. Monotonic ceiling
cost therefore gives `payment <= locked_deposit`. Sum of fills cannot exceed
funded inventory.

**Buyback priority and budget.** Ascending occupied asks use
`capacity(p) = floor(budget * SCALE / p)`, repricing the whole accepted prefix.
If a new ask makes that prefix unaffordable, the last affordable occupied price
is retained and the entire better prefix fills. This preserves priority and may
leave unused budget. Since total filled quantity is at most capacity and each
payment floors independently, `sum(payments) <= budget`.

**One price, exact integer payments.** All accepted allocations use the immutable
candidate price. Sale payments round up; buyback payments round down. U256
intermediates avoid multiplication overflow; results and aggregate additions
must fit checked nonnegative i128 values. Contract tests include large fractional
price/pro-rata books with products beyond i128 and complete liability withdrawal.

**Rounding and minimum.** Individual marginal quantities floor. Unallocated dust
is returned to the issuer through credits; there is no allocation by caller,
claim order, or transaction ordering. Zero-payment allocations are omitted.
Minimum fill is checked against actual positive allocations at finalization,
not the nominal candidate target. An authorized marginal level may still set
the nominal price even when all its individual allocations floor to zero. This
is the disclosed rounding policy; authorization now covers that level as well.
The result is not a guarantee of maximum integer volume or budget utilization.

## Compliance and bounded progress

Placement requires both SAC authorizations and the exact bidder's signature.
Revealed price contributors are checked in the preliminary sweep and again in
one final transaction. The hard 16-bid cap includes nonreveals, so this check
cannot grow without bound. Definite revocation removes a bid permanently,
subtracts its full revealed quantity once, increments the epoch, and resets
every clearing accumulator. Reauthorization cannot restore an excluded bid.
There can be at most one permanent exclusion per admitted bid.

Query failures abort atomically and permit retry; they are not interpreted as
revocation. Closing deadlines provide cancellation/refund progress even when
clearing cannot finish. Claims recheck delivery authorization independently, so
later revocation can lower actual execution without changing the published
clearing. Minimum fill constrains allocation at clearing, not subsequent delivery.

## Escrow and settlement conservation

For each asset, the contract maintains
`pending + reversible_reserves + credits == liability`. Token balances must
cover both asset liabilities before every outward transfer.
The runtime record is `Accounting`; its exact fields and the `Clearing`,
`Settlement`, and `Resolution` encodings are listed in the
[contract API](../reference/CONTRACT_API.md#shared-records).

| Path | Ownership/accounting transition |
|---|---|
| Sale delivery | Quote deposit becomes bidder surplus plus issuer proceeds. Delivered RWA reduces reserve and liability equally. |
| Sale refund | Full quote deposit becomes bidder credit; the undelivered allocated RWA becomes issuer credit. |
| Buyback delivery | RWA excess becomes bidder credit. Delivered RWA and paid quote reduce their respective liabilities. |
| Buyback refund | Full RWA deposit becomes bidder credit; the undisbursed allocated quote becomes issuer credit. |
| Cancellation | Issuer reserves become issuer credits; pending bidder deposits retain their original ownership. |
| NonReveal on cleared auction, before or after delivery expiry | Original-asset deposit becomes bidder credit minus the configured floored penalty, plus issuer credit for that penalty; no trade reserve or liability changes. Auction cancellation refunds fully. |
| Withdrawal | Only the credit owner authorizes payment, in the original asset to the same address. Credit and liability decrease only on successful transfer. |

Bid resolution is single use. Buyback's two transfer legs and all associated
bookkeeping roll back together on failure. Permissionless claims cannot choose
recipients. Expiry creates credits without asset calls, allowing ownership
bookkeeping to complete even during a freeze or balance deficit.

## Sealed bids and remaining limits

Commitments bind protocol domain, network, auction, owner, nonce, full config
hash, locked amount, price, quantity, and salt through canonical ScVal XDR.
Commit consumes an owner-scoped nonce and a bid slot. Reveal requires owner
authorization, exact commitment, valid grid/quantity, the reveal window, and
sufficient original collateral. It updates price aggregates once and keeps
excess collateral escrowed until resolution. Nonreveals never enter clearing;
their original deposit is resolved into credits under the configured policy below.

Sealed Sale admission conservatively reserves total hidden-quantity capacity
from all deposits at the minimum permitted price. Since every valid reveal has
`q <= floor(deposit*SCALE/price)`, summing and using `price >= min_price` proves
all accepted valid reveals fit the checked aggregate in any order. The bound
includes excess collateral and may reject a highly overcollateralized book;
failed admission leaves balances, nonce, count, and indexes unchanged.

Refundable nonreveals permit slot occupation and a reveal option. By default the
refund is exact; an issuer may instead configure a bounded penalty (at most 50%,
forced to zero in Open mode) applied only to the `NonReveal` reason, credited to
the issuer in the deposit's own asset through the same credit path every other
resolution uses — see [architecture §6.3](../../ARCHITECTURE.md#63-optional-non-reveal-penalty)
and [contract review addendum C-12](CONTRACT_REVIEW.md#addendum--21-october-2026-oracleadapter-and-the-optional-non-reveal-penalty).
Auction cancellation takes precedence and refunds fully. On an auction that
cleared, a never-revealed bid remains `NonReveal` despite later owner revocation
or delivery expiry; the configured penalty applies through both `claim_bid` and
`expire_bid`/`expire_range`. A revealed `Revoked` or `DeliveryExpired` bid refunds
fully. Both-mode precedence and conservation regressions cover this policy.
High-entropy salts are essential; deposit metadata is public. Related bidders
can manipulate auction prices, and SAC administrators retain
freeze/clawback/issuance powers. Raw registry observations are not a collateral
oracle policy, and the partial `OracleAdapter` read surface over them (contract
review addendum C-11) adds no manipulation or freshness policy of its own. Real
network archival restoration, multisig/wallet and broader account adversaries,
and complete network resource envelopes remain qualification work. Local tests
now cover classic master-key/custom Ed25519 credentials, auth callback re-entry
rollback, expired snapshot recovery, and 4096 ticks with all 16 exclusions in
2,200 calls, followed by complete refund withdrawal.

Reproduce the checked contract behavior with `./scripts/check.sh`; coverage and
test-mode boundaries are described in [testing](../TESTING.md).
