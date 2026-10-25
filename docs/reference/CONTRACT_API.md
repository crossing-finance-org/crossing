# Contract API — version 1

Current source version: `0.1.0`. All contracts are immutable. Constructor
parameters and the generated WASM specifications determine wire encodings.
Reviewed **24 October 2026** against [architecture v1.6](../../ARCHITECTURE.md).
Amounts/prices use `i128`, bid IDs and work limits `u32`, discovery indexes `u64`,
and hashes/nonces/salts `BytesN<32>`. Config is immutable after construction.

## AuctionFactory

| Method | Arguments | Return | Authorization |
|---|---|---|---|
| `__constructor` | `registry, registry_wasm_hash, auction_wasm_hash` | `()` | Atomic deployment; mutually verified registry bindings |
| `bindings` | none | `(registry, registry_hash, template_hash)` | Public |
| `predict` | `issuer, user_salt` | `Address` | Public; reserves nothing |
| `create` | `config:Config, user_salt` | `Address` | Current RWA SAC admin, exactly `config.issuer` |
| `get_auction` | `auction` | `Option<AuctionMeta>` | Public |
| `count` | none | `u64` | Public |
| `list` | `start:u64, limit:u32` | `Vec<Address>` | Public; limit 1–32 |
| `maintain` | `start:u64, count:u32` | `u32` extended | Permissionless; count 1–8 |

Deployment, metadata, and factory indexes commit atomically. Creation moves no
assets. Prepare both SAC authorizations for the predicted/deployed escrow before
funding it. The RWA SAC administrator is rechecked at funding. Both pair assets
must be genuine issued SACs; native XLM is excluded on both sides.

`create` authorizes the complete config and salt. The derived deployment salt is
SHA256 of canonical ScVal XDR for `(Symbol("CROSSING_AUC_V1"), network_id,
factory_address, issuer, user_salt)`. Prediction has no side effects. Creation
count is monotonic, not the number of funded/active auctions. Factory metadata
records `issuer,rwa,quote,mode,config_hash,template,ledger`; deployment and all
discovery writes roll back together on failure. Registry construction stores the
future factory pin; factory construction validates the complete mutual binding.
Constructors are reserved deployment hooks and cannot be called as public reset
functions after deployment.

## Auction

Constructor: `(config:Config, factory:Address, registry:Address,
auction_wasm_hash:BytesN<32>)`. The constructor validates config and executable
identity. Funding additionally verifies the factory registration and mutual
factory/registry bindings. An unregistered clone cannot fund through this release.

| Method | Arguments | Return | Authorization / phase |
|---|---|---|---|
| `fund` | none | `()` | Issuer; Created, before start; exact token-transfer sub-invocation |
| `place_bid` | `bidder, price, quantity` | bid ID | Bidder; Open, Funded, placement window; exact deposit transfer |
| `commit_bid` | `bidder, nonce, hash, max_locked` | bid ID | Bidder; Sealed, placement window; exact deposit transfer |
| `reveal_bid` | `id, price, quantity, salt` | `()` | Stored bid owner; Sealed reveal window |
| `advance` | `work_limit` | `Progress` | Permissionless after reveal, before closing deadline |
| `finalize` | none | `Finalization` | Permissionless; Ready, before closing deadline |
| `expire` | none | `Reason` | Permissionless; unfunded after start or unfinished after closing deadline |
| `claim_bid` | `id` | `Resolution` | Permissionless; Cleared/Cancelled, before delivery deadline |
| `expire_bid` | `id` | `Resolution` | Permissionless; Cleared/Cancelled, at/after delivery deadline |
| `expire_range` | `start_id, count` | `(next_id,newly_resolved)` | Permissionless; at/after delivery deadline; Cleared/Cancelled/Settled/Refunded; count 1–8 |
| `withdraw` | `owner, asset, amount` | transferred `i128` | Owner; positive amount within its same-asset credit |
| `maintain` | `Vec<MaintenanceKey>` | extended count | Permissionless; 1–8 typed keys |

`advance` rejects zero work and clamps each call to at most 32 ticks or 8 bid
checks according to stage. An exclusion returns successful progress and resets
the epoch; clients must inspect the result and reread state. `finalize` returns
`Cleared(Clearing)`, `Excluded(id,epoch)`, or `Cancelled(Reason)`.

Individual repeated claims/reveals/commits fail with replay errors. `expire_range`
is idempotent over already-resolved bids and returns an empty result when its
start is beyond the book. Failed transfers cause root invocation rollback.
`claim_bid` can credit a refund instead of delivering; inspect `Resolution`.
Crediting a refund is not an on-chain token payment.

Sealed Sale admission reserves hidden-quantity capacity:
`floor(total_admitted_deposits * SCALE / min_price)` must fit i128. Overflow
returns `Arithmetic` before transfer, nonce consumption, or index updates; retry
with a smaller supported deposit is possible. Excess collateral counts against
this conservative limit. See architecture §6.2 for the reveal-order proof.

The optional non-reveal penalty applies to a `NonReveal` resolution on a cleared
auction, both before and after the delivery deadline. An unrevealed owner's
later SAC revocation does not change that reason. A cancelled auction and a
revealed bid's `Revoked` or `DeliveryExpired` resolution receive full refunds.
The 22-field Config changes canonical
hashes and is incompatible with earlier 21-field pre-release client payloads.

| Getter | Return / behavior |
|---|---|
| `config()` | `Config` |
| `provenance()` | `Provenance {factory,registry,template,config_hash}` |
| `state()` | `State {stage,epoch,bid_count,tick_cursor,sweep_cursor,cumulative,previous_price,candidate}` |
| `accounting()` | Pending escrow, reversible reserves, credits, liabilities, admitted totals, executed/refunded/withdrawn totals, and resolved count |
| `clearing()` | `Option<Clearing>` |
| `settlement()` | `Option<Settlement>` after every bid resolves |
| `get_bid(id)` | One `Bid`; nonexistent ID fails |
| `credit(owner,asset)` | Same-owner same-asset credit; unsupported asset fails |
| `tick_total(index)` | Revealed/open aggregate at an in-range tick, or zero |
| `cancel_reason()` | `ReasonValue` |
| `solvency()` | `(rwa_balance,rwa_liability,quote_balance,quote_liability)` |
| `timing()` | `(close_deadline_passed,delivery_deadline_passed)`; raw clock flags, not stage-dependent eligibility or a mutation |
| `bid_ids(owner,start,limit)` | At most 32 owner-local bid IDs; retrieve each record separately |

`MaintenanceKey` variants are `Bid(id)`, `Tick(index)`, `Count(owner)`,
`OwnerBid(owner,index)`, `Nonce(owner,nonce)`, and `Credit(owner,asset)`.
Known nonexistent keys are skipped; invalid bounded IDs/assets fail. Instance and
code TTL are extended by mutation. Construction forces both the fresh instance
and reused shared WASM code through the accepted delivery/recovery horizon;
later mutations use rolling extensions. TTL excludes the current ledger, so an
exact `storage.max_ttl()` horizon is supported. Archival restoration is separate
from TTL extension; restoration cannot change ownership or remove replay guards.

Clearing compliance covers every revealed, nonexcluded bid at or better than the
candidate price, including marginal bids whose individual quantity/payment
rounds to zero. Definite revocation removes its full quantity and restarts the
price computation. Query failures abort without permanently excluding the bid.

## PriceRegistry

Constructor: `(factory, factory_wasm_hash, auction_wasm_hash)`. The factory can be
precomputed and deployed after the registry. Publication verifies its executable
and the actual auction's registration, code, terms, and immutable outcomes.

| Method | Arguments | Return |
|---|---|---|
| `bindings` | none | `(factory,factory_hash,template_hash)` |
| `record_clearing` | `auction` | Existing/new auction record ID (`Address`) |
| `record_settlement` | `auction` | Existing/new record ID; source must be Settled |
| `get_clearing` | `auction` | `Option<Observation>` |
| `get_settlement` | `auction` | `Option<Execution>` |
| `latest` | `rwa,quote` | Latest clearing observation for this ordered pair |
| `latest_settled` | `rwa,quote` | Latest positive-execution record, ordered by clearing source |
| `count` | `rwa,quote` | Pair history length |
| `history` | `rwa,quote,start:u64,limit:u32` | At most 32 auction addresses in publication order |
| `maintain` | `rwa,quote,start:u64,count:u32` | History records extended; count 1–8 |

All methods are public/permissionless. Registry publication is separate from
auction finalization and claims. History records use publication order; `latest`
uses `(clearing ledger, raw auction contract ID)` order. Publication time cannot
refresh source time. Zero-execution settlements remain queryable, but do not
replace `latest_settled`'s positive execution record.

New clearing publication requires a registered source in `Cleared`/`Settled`
with positive allocated RWA and quote. Settlement publication requires `Settled`;
it may record zero actual execution. `Cancelled`/`Refunded` auctions cannot publish
registry records. Repeated publication of an existing immutable record is
idempotent. Registry maintenance renews only the selected history range's records
plus pair pointers; maintain/restore the pointed records as dependencies too.

## OracleAdapter

Constructor: `(registry, registry_wasm_hash, base)`. Pins the registry's WASM
hash once at construction (immutable afterward); fixes `base` as this
instance's quote asset for its entire lifetime. One instance answers queries
for one base asset; deploy another instance for a different base.
The base must be a genuine seven-decimal issued SAC; native XLM is unsupported.

| Method | Arguments | Return |
|---|---|---|
| `base` | none | `OracleAsset::Stellar(base)` |
| `decimals` | none | `7`, matching `PRICE_SCALE` |
| `resolution` | none | `0` — event-driven publication, not periodic |
| `lastprice` | `asset:OracleAsset` | `Option<OraclePriceData>`; `Stellar(rwa)` maps to `registry.latest(rwa, base)`, `Other(_)` is always `None` |
| `maintain` | none | `()`; permissionless renewal of adapter instance/code TTL |

Price reads hold no funds and change no financial state. Construction fixes
bindings and forces retention for instance and reused code; `maintain` renews
only those TTLs, separately from registry maintenance. A returned price may
precede delivery or have zero actual execution, and retains its source timestamp.
This is a **partial** SEP-40 surface: `assets`, `price`, and `prices` are
not implemented, since the registry is indexed for append-only history and
`latest`, not timestamp lookup or pair enumeration. A consumer needing only the
current clearing price is fully served by `lastprice`; one needing historical
point-in-time queries should read `PriceRegistry.history`/`get_clearing`
directly. See [architecture §11.3](../../ARCHITECTURE.md#113-oracleadapter-a-partial-raw-sep-40-read-surface).

## Shared records

The exact fields below are defined in
[common/src/lib.rs](../../contracts/common/src/lib.rs). Config's complete 22-field
schema is in [architecture §3.2](../../ARCHITECTURE.md#32-schema). Structs use
Soroban contract-value encoding; do not infer tuple order from this table.

| Type | Fields |
|---|---|
| `Provenance` | `factory:Address, registry:Address, template:BytesN<32>, config_hash:BytesN<32>` |
| `AuctionMeta` | `issuer:Address, rwa:Address, quote:Address, mode:Mode, config_hash:BytesN<32>, template:BytesN<32>, ledger:u32` |
| `Candidate` | `price:i128, target:i128, numerator:i128, denominator:i128` |
| `State` | `stage:Stage, epoch:u32, bid_count:u32, tick_cursor:u32, sweep_cursor:u32, cumulative:i128, previous_price:i128, candidate:CandidateValue` |
| `Bid` | `owner:Address, deposit:i128, nonce:Option<BytesN<32>>, commitment:Option<BytesN<32>>, price:Option<i128>, quantity:Option<i128>, excluded:bool, resolution:ResolutionValue` |
| `Resolution` | `delivered:bool, rwa:i128, quote:i128, refund:i128, reason:ReasonValue, ledger:u32, timestamp:u64` |
| `Clearing` | `candidate:Candidate, filled:i128, quote:i128, winners:u32, epoch:u32, ledger:u32, timestamp:u64` |
| `Settlement` | `executed_rwa:i128, executed_quote:i128, refunded_rwa:i128, refunded_quote:i128, resolved:u32, ledger:u32, timestamp:u64` |
| `Progress` | `stage:Stage, epoch:u32, cursor:u32, processed:u32` |
| `Observation` | `auction:Address, meta:AuctionMeta, clearing:Clearing, delivery_deadline:u32, published_at:u64` |
| `Execution` | `auction:Address, clearing_ledger:u32, settlement:Settlement, published_at:u64` |
| `OracleAsset` | `Stellar(Address)` or `Other(Symbol)` — SEP-40's asset discriminant; this registry only ever resolves `Stellar` |
| `OraclePriceData` | `price:i128, timestamp:u64` — SEP-40's `PriceData` shape |

All `Accounting` fields are i128 except `resolved:u32`: `pending_rwa,pending_quote`,
`reserve_rwa,reserve_quote`, `credit_rwa,credit_quote`,
`liability_rwa,liability_quote`, `admitted_quantity,admitted_deposit`,
`executed_rwa,executed_quote`, `refunded_rwa,refunded_quote`,
`withdrawn_rwa,withdrawn_quote`. Refund amounts use the original deposit asset
(Sale quote; Buyback RWA), derived from config rather than a separate refund-asset
field. Config identity is stored in provenance/metadata, not `Clearing`.

`Mode` variants are `Sale,Buyback`; `BidMode` variants are `Open,Sealed`.
`Stage` variants are `Created,Funded,Closing,Sweep,Ready,Cleared,Cancelled,Settled,Refunded`.
`Reason` variants are `NoDemand,Minimum,Revoked,GlobalAuth,CloseExpired,Unfunded,DeliveryExpired,NonReveal,NotAllocated`.
`Finalization` variants are `Cleared(Clearing)`, `Excluded(u32,u32)`, and `Cancelled(Reason)`.

## Numeric errors

| Code | Variant | Meaning |
|---:|---|---|
| 1 | Phase | Persisted stage or auction mode disallows the operation |
| 2 | Window | Ledger outside the required operation window |
| 3 | Config | Invalid bounds, schedule, or schema version |
| 4 | Asset | Unsupported asset/interface/precision |
| 5 | Unauthorized | Current SAC authority or authorization check failed |
| 6 | Arithmetic | Checked sum/product/division/conversion/invariant failure |
| 7 | Capacity | Global or per-address bid limit reached |
| 8 | Price | Price/tick outside the configured grid |
| 9 | Amount | Quantity/deposit/amount below required bounds |
| 10 | Missing | Required record not present |
| 11 | Replay | Duplicate nonce, reveal, resolution, or registration |
| 12 | Commitment | Canonical reveal does not match the original hash |
| 13 | Collateral | Revealed obligation exceeds the locked deposit |
| 14 | Query | Authorization query failed; no exclusion is persisted |
| 15 | Credit | Withdrawal exceeds the owner's credit |
| 16 | Insolvent | Either asset balance is below its outstanding liability |
| 17 | Provenance | Deployment, registration, or executable binding mismatch |
| 18 | Work | Invalid zero/batch/page work bound |
| 19 | Ttl | Deadline/recovery horizon cannot fit storage limits |
| 20 | Reentry | Mutating entry point called while a mutation is active |
| 21 | Recipient | Forbidden bidder or delivery recipient |
| 22 | Transfer | Token transfer failed; full invocation rolls back |

Missing transaction authorization and some host/storage/resource failures are
host errors, not protocol errors. Do not interpret a transport failure as a
definite SAC revocation or automatically retry a transaction with a new nonce.

## Encoding and events

`Config` field names and variants are defined in
[common/src/lib.rs](../../contracts/common/src/lib.rs). The architecture's schema
describes this version-1 `Config`. Persisted optional custom records
use tagged enums: `CandidateValue`, `ReasonValue`, `ResolutionValue`, each with
`None` / `Some(value)` variants. Scalar/hash option fields use ordinary `Option`.

Commitments use canonical ScVal XDR, not JSON, concatenated decimal strings, or
packed integers. See [architecture §6](../../ARCHITECTURE.md#6-open-and-sealed-bids)
and [the independent vector](../../tests/vectors/commitment-v1.json),
[36-case matrix](../../tests/vectors/commitment-domains.json), and
[cryptographic review](../security/CRYPTOGRAPHIC_REVIEW.md).

Events have two topics, `(Symbol("crossing_v1"), Symbol(operation))`:

| Emitter / operation | Payload |
|---|---|
| Factory `created` | `(auction:Address, meta:AuctionMeta)` |
| Auction `funded` | `supply_or_budget:i128` |
| Auction `bid` | `(id:u32, price:i128, quantity:i128, deposit:i128)` |
| Auction `commit` | `(id:u32, hash:BytesN<32>, max_locked:i128)` |
| Auction `reveal` | `(id:u32, price:i128, quantity:i128)` |
| Auction `excluded` | `(id:u32, epoch:u32)` |
| Auction `progress` | `Progress` |
| Auction `cleared` | `Clearing` |
| Auction `cancelled` | `Reason` |
| Auction `completed` | `Settlement` |
| Auction `delivered` / `refund_credited` | `(id:u32, outcome:Resolution)` |
| Auction `nonreveal_penalty` | `(id:u32, penalty_amount:i128)`; emitted only when `nonreveal_penalty_bps > 0` and the computed penalty is positive |
| Auction `withdrawn` | `(owner:Address, asset:Address, amount:i128)` |
| Registry `clearing_recorded` / `settlement_recorded` | `auction:Address` |

Successful creation and bid admission reveal no sealed preimage. Owner/nonce/config
must be read from state where absent from events. Indexers reconcile events with
public state and authenticate the emitting artifact; an event is not a substitute
for balance checks. Multiple events from one invocation can reflect intermediate
steps (for example `completed` plus the final `delivered`); reconstruct the final
state after the invocation succeeds.
