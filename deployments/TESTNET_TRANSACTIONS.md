# Testnet transaction log

Network: Stellar Testnet. Companion to [testnet.json](testnet.json) (current
contract addresses/hashes) and the
[consolidated findings report](../docs/security/FINDINGS_AND_FIXES.md) (the
C-19 fix that made this redeployment necessary). Every entry below is a real
transaction submitted to and accepted by the public Stellar Testnet — not a
local test or simulation. A transaction that only reached simulation and
failed before submission has no hash and is not counted here; those are
called out inline where they affected the sequence of events.

This log starts from the 24 October 2026 redeployment (post [C-19
fix](../docs/security/FINDINGS_AND_FIXES.md#phase-3--contract-implementation-review)).
It is updated as the demonstration continues. A 25 October 2026 read-only audit
decoded all 106 transactions from public Testnet RPC, corrected whole-token
amounts below using seven decimals (`10,000,000` raw units per token), and
recovered entry 100's originally missing hash from ledger 5368459. These are
evidence corrections; no historical transaction or deployed contract changed.

## 1. Redeployment (auction WASM changed by the C-19 fix)

| # | Action | Tx hash |
|---|---|---|
| 1 | Upload fixed `crossing_auction.wasm` (hash `e0fe1c94…2763d`) | [b33f38c9](https://stellar.expert/explorer/testnet/tx/b33f38c98b181da1e69a0d47d1ed45287c4ebf40fb6b2c43c2bc3d627237fb70) |
| 2 | Deploy new PriceRegistry, pinned to the predicted factory address and the fixed auction hash | [e06cbd88](https://stellar.expert/explorer/testnet/tx/e06cbd88207e5ce202b96863e603d54c65df52f203dad66a3edb5e92b5b399e1) |
| 3 | Deploy new AuctionFactory at the predicted address, mutually verified against the new registry | [278613b3](https://stellar.expert/explorer/testnet/tx/278613b366343198f795358f5fe85ba3bf2603816393c9cccad579a987edddc4) |
| 4 | Deploy OracleAdapter — first attempt, `base` mistakenly set to the RWA asset (CRWA) instead of the quote asset; left unused, see #28 | [fd5f8f0a](https://stellar.expert/explorer/testnet/tx/fd5f8f0a6d49f4d7ae6fb0a1b2ebc309339cd412ad6286ca51709439d4e217e1) |

Mutual bindings were read back and confirmed consistent (read-only calls,
not transactions) before any auction was created against the new factory.

## 2. Auction A / A2 — Open Sale (demonstrates the standard lifecycle)

Auction A missed its own funding window (the `start_ledger` passed between
`create` and `fund` while earlier steps were still being prepared); it was
formally cancelled with reason `Unfunded` rather than left in limbo — a real
exercise of that recovery path, not a bug. Auction A2 is the auction that
actually ran.

| # | Action | Tx hash |
|---|---|---|
| 5 | `create` Auction A (Open Sale) | [6feebc6c](https://stellar.expert/explorer/testnet/tx/6feebc6c35b619c497632b3cc92154288b410d67753d8e685f8a84ceb574aba1) |
| 6 | `set_authorized` escrow A for CRWA | [f498c506](https://stellar.expert/explorer/testnet/tx/f498c5062558cc96fc5ad87542aadaf2f008d7be01ce28f10fd8cd47b97e10ba) |
| — | `fund` A — rejected in simulation (`Error::Window`, start_ledger already passed); never submitted, no hash | — |
| 7 | `expire` A → cancelled, reason `Unfunded` | [394421a5](https://stellar.expert/explorer/testnet/tx/394421a514efe4276bbd608e0e3078641caa84da8906f2eb9d31b926649b704c) |
| 8 | `create` Auction A2 (Open Sale) | [446d3b9a](https://stellar.expert/explorer/testnet/tx/446d3b9a28a4f834b5551aaf0c152b4f2215c82ae9bd8e26c94038de5101ae2d) |
| 9 | `set_authorized` escrow A2 for CRWA | [1d2e6c60](https://stellar.expert/explorer/testnet/tx/1d2e6c601dbdb922e580a5740df57e1b997c25f023cb6870f64a5b2c258f84a8) |
| 10 | `fund` A2 (60 CRWA minted/transferred to escrow; 600,000,000 raw units) | [8fcfebdb](https://stellar.expert/explorer/testnet/tx/8fcfebdb26aa99a8259c027b3780c847f5b85105b9c02eca8dbc21de33315eab) |
| 11 | `place_bid` user-2, price 3.0, qty 15 | [cc4806f8](https://stellar.expert/explorer/testnet/tx/cc4806f8225e14d16f4d9f83cd39ef3e62e9b9ded9ecf5b5a3919a3902b71e2e) |
| 12 | `place_bid` user-3, price 2.0, qty 15 | [f65d5d8a](https://stellar.expert/explorer/testnet/tx/f65d5d8ac41b032250fef0afa4d7cf8a3bf43ec0cb6ca0d2966c07e35af25578) |
| 13 | `place_bid` user-4, price 1.0, qty 20 | [67496e03](https://stellar.expert/explorer/testnet/tx/67496e03a61036f2588e4e06e8718d3bcc919d448d2043e8a52ab3ccb663a5fb) |
| — | `place_bid` user-1 at price 4.0 / qty 15 — rejected in simulation (deposit would exceed user-1's CUSD balance); never submitted, no hash | — |
| 14 | `place_bid` user-1 (retried), price 4.0, qty 10 | [37af8d98](https://stellar.expert/explorer/testnet/tx/37af8d98b6ae690363413b0068e711e31268e0083a1f94f6c88065e52fe7ea19) |
| 15 | `advance` (tick walk → candidate found) | [929f6119](https://stellar.expert/explorer/testnet/tx/929f6119e16868030d6f18e70782454a3dbe4f24173f5568115eda363c723c30) |
| 16 | `advance` (sweep → Ready) | [4b011c9c](https://stellar.expert/explorer/testnet/tx/4b011c9c5791908005374ffd1f0d70284e7243dde1b16bc966fbd25668d6ac92) |
| 17 | `finalize` → Cleared at price 1.0, 4 winners, 60 CRWA filled for 60 CUSD | [c63c3311](https://stellar.expert/explorer/testnet/tx/c63c3311e2394e27e0fa1088ba5d5c44ab1e56704f9dcac76eb3f9c4335adb2b) |
| 18 | `claim_bid` 0 (user-2) — delivered, refund credited | [ea09d110](https://stellar.expert/explorer/testnet/tx/ea09d11051aba53fc893d3e502655979a3b3b296a048613c1115c99f4476c172) |
| 19 | `claim_bid` 1 (user-3) — delivered, refund credited | [b8270f0e](https://stellar.expert/explorer/testnet/tx/b8270f0ea5e9b81e8d45f93f07873889c9e8a7821b4d73cced04a434d9344501) |
| 20 | `claim_bid` 2 (user-4) — delivered, exact payment, no refund | [a757ae83](https://stellar.expert/explorer/testnet/tx/a757ae83a4875b5a425f01bf273cd363654c88a39b5a7ba6b64a1ecfa0caba09) |
| 21 | `claim_bid` 3 (user-1) — delivered, refund credited; auction fully resolved | [671e5fee](https://stellar.expert/explorer/testnet/tx/671e5fee57f0d6bc7cfa9e66a1bb4472752e5aa25477ff51ace3fe2255af2438) |
| 22 | `withdraw` issuer, 60 CUSD proceeds | [84f7b311](https://stellar.expert/explorer/testnet/tx/84f7b311fd6e9562c7adcd18b6c344fc799a66da5beb9e55145b7ef7f965a631) |
| 23 | `withdraw` user-1 refund, 30 CUSD | [6981913c](https://stellar.expert/explorer/testnet/tx/6981913c9460a380a7eb758282d0d3980a310c0485b77efe19d91dc9e5f8ab1c) |
| 24 | `withdraw` user-2 refund, 30 CUSD | [73a4b73c](https://stellar.expert/explorer/testnet/tx/73a4b73c8125503d4e75297604ff97fbc5c90c887bde8ae4096caa684699a848) |
| 25 | `withdraw` user-3 refund, 15 CUSD | [061ec286](https://stellar.expert/explorer/testnet/tx/061ec286f9f5559c2102bbc5b4d3ba3358d08e485ce10be888a4cf17cfba633e) |
| 26 | Registry `record_clearing` Auction A2 | [2abbf0da](https://stellar.expert/explorer/testnet/tx/2abbf0da31682b0a7c9d400a6d9a8c2f42761428592f94f446f9d01dbf68a53d) |
| 27 | Registry `record_settlement` Auction A2 | [a2b39562](https://stellar.expert/explorer/testnet/tx/a2b39562d1bbf131a31217306d265d1f271e30426b7acd187d3aad9ae2e08e5e) |

user-4's bid needed no withdrawal: its deposit exactly matched its payment
(price 1.0 × qty 20 = 20, clearing price 1.0 × fill 20 = 20), so its refund
credit was zero. The registry's `latest()` and the oracle adapter's
`lastprice()` were both read back afterward (read-only calls, not
transactions) and matched the recorded clearing price of 1.0 CUSD/CRWA.

## 3. OracleAdapter correction

| # | Action | Tx hash |
|---|---|---|
| 28 | Deploy OracleAdapter — corrected, `base` = CUSD (the quote/settlement asset, matching SEP-40 semantics where `lastprice(OracleAsset::Stellar(rwa))` reports that rwa's price in the fixed base) | [14456f2a](https://stellar.expert/explorer/testnet/tx/14456f2ac5b4b625f4cad53edae0f98fb928f00e9d13d9bdbce3f3483a981a5d) |

## 4. Auction B — Sealed Sale, abandoned (commit window missed)

Created and funded, but the commit-tool needed a real fix before it could
produce a valid commitment hash (see §5), and that detour ran past this
auction's short commit window. Left to resolve on its own later via
`advance`/`expire` once its own deadlines pass — not a protocol defect, the
same kind of recoverable miss as Auction A above, just this time caught
before any bidder funds were locked into it.

| # | Action | Tx hash |
|---|---|---|
| 29 | `create` Auction B (Sealed Sale, `nonreveal_penalty_bps` 1000) | [8d65a1d8](https://stellar.expert/explorer/testnet/tx/8d65a1d872a195371619c248cc400d2a897a11aee8421675c8b9c1c8250cdaaf) |
| 30 | `set_authorized` escrow B for CRWA | [294fc5cf](https://stellar.expert/explorer/testnet/tx/294fc5cf56ff8c9b61bc26328d070779eb83c8ce613ebbcc02b225c61deced65) |
| 31 | `fund` B | [c744ae74](https://stellar.expert/explorer/testnet/tx/c744ae740a6f2c5cdf3b35c8bed10d4852b0275e9bdd6839009ee793021ce5b9) |
| — | `commit_bid` × 3 — all rejected in simulation (`Error::Window`, `end_ledger` already passed); never submitted, no hashes | — |

## 5. Commit-tool fix

The standalone Rust tool used to compute real sealed-bid commitment hashes
for testnet (it calls the audited `crossing_common::commitment()` helper
directly, rather than re-implementing XDR encoding by hand) originally
wrapped the call in `env.as_contract(&auction_address, …)` so that
`env.current_contract_address()` inside `commitment()` would resolve to the
real auction address. That panics in a bare `Env::default()`: there is no
actually-registered contract instance behind an arbitrary testnet address,
and `as_contract`'s bookkeeping needs one. The fix builds the exact same
typed tuple `commitment()` builds, substituting the auction's address
literally in place of `current_contract_address()` — identical during real
execution, since that call always equals the auction's own address — and
hashes it directly with no `as_contract` wrapper. The corrected tool's
output was checked against the existing
[independent commitment vector](../tests/vectors/commitment-v1.json) (whose
expected hash was itself cross-checked against a from-scratch JavaScript XDR
encoder) before it was trusted with any real bid: it reproduced
`eb9d5a5209daf34a55afd313cfbb7a8186ce3233ed645b8b4a57d4fb292ca6b6` exactly.

## 6. Auction C — Sealed Sale (the commit-tool in real use)

Same parameters as the abandoned Auction B, with a much larger commit/reveal
buffer. `nonreveal_penalty_bps` 1000 (10%). Three sealed bids: user-5 and
user-6 commit and reveal normally; user-7 commits and deliberately never
reveals, specifically to exercise the
[C-19 fix](../docs/security/FINDINGS_AND_FIXES.md#phase-3--contract-implementation-review)
live — resolving user-7's bid after the delivery deadline, via `expire_bid`
rather than `claim_bid`, to confirm the penalty still applies exactly as it
would have before the deadline (the bypass C-19 fixed would have made this
one path produce a full, unpenalized refund instead).

| # | Action | Tx hash |
|---|---|---|
| 32 | `create` Auction C (Sealed Sale) | [6ecc02da](https://stellar.expert/explorer/testnet/tx/6ecc02da0d80c8ebe0fd8a1463b851992f334c6d0b5917e6b877094ad4336554) |
| 33 | `set_authorized` escrow C for CRWA | [594f88e8](https://stellar.expert/explorer/testnet/tx/594f88e8398b0c5898663fd20aa08272e0b1910379a79cdd727ec7ff46252b86) |
| 34 | `fund` C | [88714124](https://stellar.expert/explorer/testnet/tx/88714124c5eabbe18e19647131d33c4f1318cd555e66ecd9dd8026ac79dce0bf) |
| 35 | `commit_bid` user-5 (price 2.0, qty 10, deposit 25 CUSD) | [14545c9e](https://stellar.expert/explorer/testnet/tx/14545c9efcbf45f9d7218eb7cfd4e7d4dfe2c1a81b55d5201dc207663763cd5c) |
| 36 | `commit_bid` user-6 (price 1.0, qty 10, deposit 15 CUSD) | [8eb3f39c](https://stellar.expert/explorer/testnet/tx/8eb3f39cff264da1d5061c0c44a750733d9925d0ed560cf8f722344c4ba4d841) |
| 37 | `commit_bid` user-7 (deposit 35 CUSD; will never reveal) | [d131280d](https://stellar.expert/explorer/testnet/tx/d131280dcf997657fd0f5546230107432cc315e69b8d6c1a725c9a1c6dcb72b1) |
| 38 | Cleanup: `expire` abandoned Auction B → cancelled, reason `CloseExpired` | [f513c665](https://stellar.expert/explorer/testnet/tx/f513c665d5abc68ed7c16b0f47c53471f5a3273769870b2dfe3d98e727bc7043) |
| 39 | Cleanup: issuer `withdraw` 30 CRWA back from cancelled Auction B (burned, since CRWA's issuer is the recipient) | [75728e31](https://stellar.expert/explorer/testnet/tx/75728e317ee6139e09fa6e39ef11970d07f91a6746c85a910f81eeb9233c4f43) |
| 40 | `reveal_bid` user-5, bid 0 → price 2.0, qty 10 (matches commitment) | [90b39943](https://stellar.expert/explorer/testnet/tx/90b39943ce3cc77db0905845c133cf9f33cc83215feeae65ddaf7a6f04ec9050) |
| 41 | `reveal_bid` user-6, bid 1 → price 1.0, qty 10 (matches commitment) | [5a4229d3](https://stellar.expert/explorer/testnet/tx/5a4229d39f855180a45f690c9f59eb8d857a906b72964b69cb5666a231cbd340) |
| 42 | `advance` (tick walk → candidate found) | [797a61e5](https://stellar.expert/explorer/testnet/tx/797a61e5a90a7fea3cb28d6fc3547f205f736ec4a59b6dd2abe42d7f6020043c) |
| 43 | `advance` (sweep → Ready) | [9e72ce77](https://stellar.expert/explorer/testnet/tx/9e72ce77be0e110dab8840865f73f315cdc2a714f9437248e4eb234889405def) |
| 44 | `finalize` → Cleared at price 1.0 (reserve floor, undersubscribed: 20 revealed units against 30 supply), 2 winners | [d160e746](https://stellar.expert/explorer/testnet/tx/d160e7469706bd04421371ef00fa98d37d38ed209735838ff0f5f55e0a7c33fe) |
| 45 | `claim_bid` 0 (user-5) — delivered before the deadline, refund credited | [f464defa](https://stellar.expert/explorer/testnet/tx/f464defa71e9dc55d0df33ad24149d5b176f10f13ab50a76aebb73010f82df46) |
| 46 | `claim_bid` 1 (user-6) — delivered before the deadline, refund credited | [43aae2b9](https://stellar.expert/explorer/testnet/tx/43aae2b909f28ba1e58f18b48175ac97b5d5ded34577786799d6e904bcb017e0) |
| 47 | **`expire_bid` 2 (user-7, never revealed), called *after* `delivery_deadline`** → resolved with `reason: NonReveal` and the 10% penalty (3.5 CUSD of the 35 CUSD deposit) credited to the issuer; refund 31.5 CUSD. This is the exact path the pre-fix contract mishandled: before the C-19 fix, resolving a non-revealed bid this way (after the deadline, via the permissionless expiry path instead of `claim_bid`) reported `DeliveryExpired` and skipped the penalty entirely. Confirmed live here that the fixed contract applies the penalty identically regardless of which path or timing resolves the bid. | [b553de2a](https://stellar.expert/explorer/testnet/tx/b553de2a009e33699f3a4cff6c0ac3b887a6cfad9d56f7762b7098142d0468cc) |
| 48 | `withdraw` issuer, 10 CRWA unused supply (burned back) | [791cf8b5](https://stellar.expert/explorer/testnet/tx/791cf8b5314dc9a675e9e0163fc7a15aac110f5fc6e99fa56a97f9b472de2ad3) |
| 49 | `withdraw` issuer, 23.5 CUSD (20 proceeds + 3.5 penalty) | [afd4f258](https://stellar.expert/explorer/testnet/tx/afd4f2589a008700ecb76d8df831603e021fdc084d4dbb6a55110e85c6ff067f) |
| 50 | `withdraw` user-7, 31.5 CUSD penalized refund | [f1034326](https://stellar.expert/explorer/testnet/tx/f10343268ebf24cc28db2bc29b51fc72a511929d1d5d1d1ec9b0a2c5acb86c62) |
| 51 | Registry `record_clearing` Auction C | [027b8b3c](https://stellar.expert/explorer/testnet/tx/027b8b3cb3ba8c09eb4be41d30fb7ddb446ae16c445f7d1940272152613e07f2) |
| 52 | Registry `record_settlement` Auction C | [d5163324](https://stellar.expert/explorer/testnet/tx/d516332448ca5b3b2cc969e8205b8ab51b8c94259fd332b1455290d1f6cfa434) |

The registry's `latest()` and the oracle adapter's `lastprice()` were read
back afterward (read-only, not transactions) and both correctly reflect
Auction C's later clearing (ledger 5367832 supersedes Auction A2's 5367447
for the same asset pair), price 1.0 CUSD/CRWA.

Credit bookkeeping checked before withdrawal: issuer's quote credit was
exactly 23.5 CUSD = 20 CUSD proceeds (2 × 10 units × 1.0) + 3.5 CUSD penalty
(10% of user-7's 35 CUSD deposit) — confirming the penalty is credited
through the same mechanism as ordinary proceeds, not a separate side
channel, and that `refund + issuer_credit == original_deposit` held for
user-7 (31.5 + 3.5 = 35.0).

## 7. Auction D — Buyback, Open mode (the other auction mode and direction)

Everything above used Sale mode (issuer sells RWA for quote). Auction D
demonstrates Buyback (issuer spends a quote budget to repurchase RWA from
bidders), and also exercises the uniform-price mechanism's other direction:
the clearing price is the *highest accepted ask*, benefiting cheaper sellers,
and a seller asking too high (user-10) is unallocated rather than overpaid.

| # | Action | Tx hash |
|---|---|---|
| 53 | `create` Auction D (Buyback, Open, budget 20 CUSD) | [67711eed](https://stellar.expert/explorer/testnet/tx/67711eed7e4872a496a48b12222c3348b6abe6e445a49d14f1d32b4ea04dfa7a) |
| 54 | `set_authorized` escrow D for CRWA | [640245e3](https://stellar.expert/explorer/testnet/tx/640245e323538ff1f32177182271b4b54b90291846e6b01e43b59d380a7aea2d) |
| 55 | `fund` D (20 CUSD budget transferred to escrow) | [ba3cb63d](https://stellar.expert/explorer/testnet/tx/ba3cb63d5e41eb03fb2aa8f99ee2f22f504b2b95783b5ac51ab5cc97020bf95c) |
| 56 | `place_bid` user-8, asking price 1.0, qty 5 (deposits 5 CRWA) | [11a3a43c](https://stellar.expert/explorer/testnet/tx/11a3a43c36f137290ef13289c270f00e7b33d0750c17639f498c8af3558f7330) |
| 57 | `place_bid` user-9, asking price 2.0, qty 5 (deposits 5 CRWA) | [a170e692](https://stellar.expert/explorer/testnet/tx/a170e692cce78d8ce19eb2435916d3bc9df455b74353e6693f2222bda62a29b9) |
| 58 | `place_bid` user-10, asking price 3.0, qty 5 (deposits 5 CRWA) | [92e28ff2](https://stellar.expert/explorer/testnet/tx/92e28ff2494529032dbeb4d3e3ea77742ff2877f2464a3375b91d807bbfd4389) |
| 59 | `advance` (tick walk) | [385538a0](https://stellar.expert/explorer/testnet/tx/385538a02ec819ddbc8e2e8ca6d1155dd54258f243bdd8388793ce3f6dbe0cd1) |
| 60 | `advance` (sweep → Ready) | [8eb5902d](https://stellar.expert/explorer/testnet/tx/8eb5902dc550a885763bc632fbbcee72429274cdde3bcd0da4517a3a80e3e36f) |
| 61 | `finalize` → Cleared at price 2.0 (the full budget exactly covers user-8 + user-9's combined 10 units), 2 winners | [c39dffa8](https://stellar.expert/explorer/testnet/tx/c39dffa8b233a23967f066612be401c18dad005fd281291f2ecc767259cafc89) |
| 62 | `claim_bid` 0 (user-8) — RWA delivered to its classic asset issuer (auto-burned), 10 CUSD paid | [0f76e432](https://stellar.expert/explorer/testnet/tx/0f76e432f0d9c1858e5f6f68dfb4408fb8d919a5529246f3de71ebfe127cf004) |
| 63 | `claim_bid` 1 (user-9) — RWA delivered, 10 CUSD paid | [2e790ba4](https://stellar.expert/explorer/testnet/tx/2e790ba4056695a6c76ef7b559cfd05cbcbc3bdd8dff99f3e2f88131a822b109) |
| 64 | `claim_bid` 2 (user-10) — `NotAllocated` (asked above the clearing price): full 5 CRWA refund credited, no payment | [1498516c](https://stellar.expert/explorer/testnet/tx/1498516c143ed3fe720d35097d2072497f099bd3111717e171235af3d70a5d3f) |
| 65 | `withdraw` user-10, 5 CRWA refund | [f900cb66](https://stellar.expert/explorer/testnet/tx/f900cb66ce7a74aaa3a644655e8ea2c592c9b1ef37e97c257e673dcbe2d73ac2) |
| 66 | Registry `record_clearing` Auction D | [ee37c4bc](https://stellar.expert/explorer/testnet/tx/ee37c4bc67a3cef8069055d09bfc916b66e76a59cfc902ad2c297c535363a162) |
| 67 | Registry `record_settlement` Auction D | [a518bb82](https://stellar.expert/explorer/testnet/tx/a518bb8201a80eacaafde153fb9e922fd167378f2ff36aab26cc37243f99d503) |

No issuer withdrawal was needed for RWA or quote on this auction: the
acquired RWA was delivered straight to the issuer (and auto-burned, since
the buyback recipient here is CRWA's classic asset issuer) in each `claim_bid`
call, and the full 20 CUSD budget was spent paying user-8 and user-9 — there
was no leftover quote credit to withdraw.

## 8. Auction E — Open Sale with a live authorization revocation mid-auction

Demonstrates the exclusion-and-restart path directly, as distinct from
Auction D's price-based `NotAllocated` outcome: a bidder's SAC
authorization is revoked by the issuer *after* bidding closes but *before*
clearing finishes, forcing the finalizer to detect it, exclude that bid,
bump the clearing epoch, and restart the tick walk without it — the same
mechanism that protects a cleared price from being distorted by a bidder
who becomes unauthorized between bidding and settlement (C-10 in the
[findings report](../docs/security/FINDINGS_AND_FIXES.md#phase-3--contract-implementation-review)).

| # | Action | Tx hash |
|---|---|---|
| 68 | `create` Auction E (Open Sale, 60 units) | [3d3dc738](https://stellar.expert/explorer/testnet/tx/3d3dc7381c50a7d8180ff90c5e10de323c167c03f45f8ce3c15f99504f9c14ed) |
| 69 | `set_authorized` escrow E for CRWA | [6100bbd4](https://stellar.expert/explorer/testnet/tx/6100bbd4030e75b714d088efce90676ed45c332a0a58dfa04e56ec8b63b97ce4) |
| 70 | `fund` E | [f3e43aa2](https://stellar.expert/explorer/testnet/tx/f3e43aa20af72e55809f1cf057e8b9bf95a81d10f0cd4c46ced34a7bcc7f4fb8) |
| 71 | `place_bid` user-1, price 3.0, qty 10 | [502f2fac](https://stellar.expert/explorer/testnet/tx/502f2fac8fa2d06250d61a09c4f6c5a02b9bf07fc6e9e1983e36f3805fc18457) |
| 72 | `place_bid` user-2, price 2.0, qty 15 | [42a97771](https://stellar.expert/explorer/testnet/tx/42a977714ce91502e04420d15ada4b4f6bc3150e0c1edde1269db753d816641c) |
| 73 | `place_bid` user-3, price 1.0, qty 15 (will be revoked) | [fdf0888e](https://stellar.expert/explorer/testnet/tx/fdf0888e56e2eefd491191a84d99cced3e4a736cccc3fc264fcf2a8e827e604a) |
| 74 | `place_bid` user-4, price 1.0, qty 20 | [8113cae1](https://stellar.expert/explorer/testnet/tx/8113cae1ac6c8298183e4ac511d90ec54c41121ee3de9854e6acc103079060ff) |
| 75 | Issuer revokes user-3's CRWA authorization (`set_authorized` false), after bidding closes | [0c2dbee5](https://stellar.expert/explorer/testnet/tx/0c2dbee59802e173f6212d70c21434ccc319d39872f2f783352e41e62a7f4f47) |
| 76 | `advance` (tick walk, epoch 1 — finds a candidate using all four bids' demand) | [d740640f](https://stellar.expert/explorer/testnet/tx/d740640ffe867296aff804c1594c8dfe543b689c316b0f9404d085400335da6b) |
| 77 | `advance` (sweep detects user-3 unauthorized → **excluded**, epoch bumps to 2, restarts) | [01fa46d5](https://stellar.expert/explorer/testnet/tx/01fa46d58db6a1d963f82041d42f6a1b2da1761848b7cf4dce5ec29323d8bb55) |
| 78 | `advance` (tick walk, epoch 2 — new candidate from the remaining three bids) | [70d5fe4d](https://stellar.expert/explorer/testnet/tx/70d5fe4dcb44d63e7471b195009064a47a92b120e609f1332b25b0b00d3363b2) |
| 79 | `advance` (sweep → Ready, all three remaining bids pass) | [70ba08b2](https://stellar.expert/explorer/testnet/tx/70ba08b2f164666624c5794cd6712dc2e7fceb4ff3f5f68ed915843e71a0d094) |
| 80 | `finalize` → Cleared at the reserve price 1.0 (undersubscribed once user-3's demand was removed), 3 winners | [69588a6d](https://stellar.expert/explorer/testnet/tx/69588a6dab03f23b6324e226c45f094c4a2ae2ccdc9d1bb6033b7ce81ba6fb49) |
| 81 | `claim_bid` 0 (user-1) — delivered, refund credited | [001d434e](https://stellar.expert/explorer/testnet/tx/001d434e21465e06be8db186b855b7718ab1daabbb643a7c4e4b71394d32d20a) |
| 82 | `claim_bid` 1 (user-2) — delivered, refund credited | [b271b54e](https://stellar.expert/explorer/testnet/tx/b271b54e473ff8416fe57f9fd693c7991103ef34f83c09a03393e8465d7b9eea) |
| 83 | `claim_bid` 2 (user-3) — **`reason: Revoked`**, full deposit refunded, no delivery (its own authorization revocation, not a timing penalty) | [5028dad3](https://stellar.expert/explorer/testnet/tx/5028dad3961201b59572d095224ffe1de9bb73c49a7264a4b37b9eda65d9af26) |
| 84 | `claim_bid` 3 (user-4) — delivered, exact payment, no refund; auction fully resolved | [f6a000b7](https://stellar.expert/explorer/testnet/tx/f6a000b7e513711540dcbc4d10b3a3325e97eccbcb46ddd09ac1585003baf6fb) |
| 85 | Issuer re-authorizes user-3's CRWA access (cleanup, for future use) | [6c2bd216](https://stellar.expert/explorer/testnet/tx/6c2bd216d847737181a09f8e9a2faec709b6faab5fc5be223822aea14c5878ed) |
| 86 | `withdraw` issuer, 15 CRWA unused supply (burned back) | [ae1c6742](https://stellar.expert/explorer/testnet/tx/ae1c674268cec9ffc8e9c77abf334a02e1063c4d749a65cf0e1a40a2a73cec7d) |
| 87 | `withdraw` issuer, 45 CUSD proceeds | [8899f417](https://stellar.expert/explorer/testnet/tx/8899f41727eb6e9b92c52dde7ee6e621d5db25c66e2b248a1506bc3d5e06baf4) |
| 88 | `withdraw` user-1 refund, 20 CUSD | [b748e4cc](https://stellar.expert/explorer/testnet/tx/b748e4cc1a033f21363605cda8359785c8a0144acf57b1a3b147e93aee567562) |
| 89 | `withdraw` user-2 refund, 15 CUSD | [e7527d04](https://stellar.expert/explorer/testnet/tx/e7527d04924d162a1949b4f8864bd7f6492bc6d0e42160b9e19aa36c0f96fdd8) |
| 90 | `withdraw` user-3, full 15 CUSD refund (revoked, never delivered) | [d348044f](https://stellar.expert/explorer/testnet/tx/d348044fdc3b31a673d9fa5f9b5645de2e830391cf70658e6e1a6735110375bc) |
| 91 | Registry `record_clearing` Auction E | [dea91691](https://stellar.expert/explorer/testnet/tx/dea91691cb0a665773d87984ea765b553d91a75ee238dd61045af4444fae936e) |
| 92 | Registry `record_settlement` Auction E | [d7de2cfb](https://stellar.expert/explorer/testnet/tx/d7de2cfbb8bac9cadf70a842cef22ff1508299db1c52a91ca4e7af0ad7d9f11e) |

## 9. Maintenance entry points (TTL renewal)

A quick demonstration of the permissionless `maintain()` entry point each
contract exposes, renewing storage/code TTL without changing any
accounting or auction state.

| # | Action | Tx hash |
|---|---|---|
| 93 | AuctionFactory `maintain` (renews up to 8 tracked auction entries' TTL) | [5206a0df](https://stellar.expert/explorer/testnet/tx/5206a0df69fb73447a8fe0ca215f7da12f5cffe9cf6343a553acba9eadf1a9a4) |
| 94 | OracleAdapter `maintain` (renews instance/code TTL) | [fb24b720](https://stellar.expert/explorer/testnet/tx/fb24b7206516c660912dbe99cc5b8410e9b9b21e1867c2a6ace3fdf64cfe4ec4) |
| 95 | PriceRegistry `maintain` for the CRWA/CUSD pair (renews its history entries' TTL) | [148b19b9](https://stellar.expert/explorer/testnet/tx/148b19b97c5afddf8930e6f8d59604d3bfa740bd3abdebb2dda09dba4d98a080) |
| 96 | Auction E `maintain` on bids 0 and 1 (renews their persistent-entry TTL) | [fb45618a](https://stellar.expert/explorer/testnet/tx/fb45618ad279950c839b3b0d642411c1bd318ffa062a0804e5b78a6412e05955) |

## 10. Auction F — a second missed-deadline recovery (Open Sale)

A background interruption during bid placement meant only one of two
planned bids (user-8's) actually landed before the bidding window closed,
and by the time that was discovered, `close_deadline` had passed too — a
real instance of the same kind of session-timing gap that affected Auction A
earlier, now against the live, already-redeployed contracts. Handled the
same way: `expire` → `CloseExpired`, resolve the one placed bid, withdraw
everything back. This is the recovery path working correctly under
uncontrolled real-world interruption, not a scripted scenario.

| # | Action | Tx hash |
|---|---|---|
| 97 | `create` Auction F (Open Sale, 20 units) | [4211cf2a](https://stellar.expert/explorer/testnet/tx/4211cf2a3321b52d9737f198b852c53c21c005e973ff674ba23c1e1f9cb938fe) |
| 98 | `set_authorized` escrow F for CRWA | [0fab5217](https://stellar.expert/explorer/testnet/tx/0fab5217c6960791308e3a9c4e7f4daa4a060c2633e0512adcca9f7807a93bac) |
| 99 | `fund` F | [a3fb8598](https://stellar.expert/explorer/testnet/tx/a3fb8598e6ebe477d2c1af097cc5a62e3a060f013cf2ead133702e7777e8bec3) |
| 100 | `place_bid` user-8, price 2.0, qty 10; hash recovered on 25 October from ledger 5368459 after the original CLI output was lost | [fce4e366](https://stellar.expert/explorer/testnet/tx/fce4e36633eadf197cf050c32baa5db34af1b4876cb468ccc160379e5dcf31e4) |
| — | `place_bid` user-9 — never reached the network: by the time the interruption was discovered and checked, `end_ledger` had already passed, so this bid was never attempted on-chain | — |
| 101 | `expire` F → cancelled, reason `CloseExpired` | [ae7fca8e](https://stellar.expert/explorer/testnet/tx/ae7fca8e9cedae71131fc42f67a40c4c6210d8a04b5d3f10bc80681ff0468c0e) |
| 102 | `claim_bid` 0 (user-8) → `CloseExpired`, full refund | [48d00510](https://stellar.expert/explorer/testnet/tx/48d00510d0516a5f037f6a3305fe9eea7c8a1b85189a106dc1f2a25287851649) |
| 103 | `withdraw` user-8, full 20 CUSD refund | [3adda22a](https://stellar.expert/explorer/testnet/tx/3adda22a1c1ec0575c4d4ba0abae5e029b24a61118487d8e5b435cfd05d1097f) |
| 104 | `withdraw` issuer, 20 CRWA unused supply (burned back) | [9afb1aa1](https://stellar.expert/explorer/testnet/tx/9afb1aa1bed84ab4ce09bf5f7bebc293fbd733b3fc1195e1ae00475f6a2407a0) |

## 11. Correction — two missed withdrawals on Auction C

A post-hoc balance check (querying each resolved auction escrow's token
balances directly, expecting zero) found Auction C still holding 20 CUSD:
user-5's and user-6's refund credits from §6 had been correctly resolved by
`claim_bid` but never actually withdrawn — only user-7's penalized refund
and the issuer's credits were withdrawn at the time. The credits were never
at risk (an unwithdrawn credit just sits as a permanent, owner-only claim —
this is exactly the D-03/D-15 guarantee from the
[findings report](../docs/security/FINDINGS_AND_FIXES.md#phase-1--original-design-review-first-pass)
working as intended), but leaving them unclaimed would have been a
misleading way to close out the demonstration.

| # | Action | Tx hash |
|---|---|---|
| 105 | `withdraw` user-5, 15 CUSD refund (Auction C, missed earlier) | [9ebd43ab](https://stellar.expert/explorer/testnet/tx/9ebd43abadb83682f3051db9dd1bcbfeb63baac4152586fa47f7359096c09964) |
| 106 | `withdraw` user-6, 5 CUSD refund (Auction C, missed earlier) | [d23a44da](https://stellar.expert/explorer/testnet/tx/d23a44daa531d9663ba541ae86a2b276b54e417f3007a879b5e3d3ee730b80ef) |

Confirmed afterward: every one of the five resolved auction escrows
(A2, C, D, E, F) now holds a zero balance in both CRWA and CUSD.

## Running total

**106 real Testnet transactions submitted and accepted on-chain**, all with
recorded hashes after entry 100 was independently recovered and verified on
25 October. Together these cover: a from-scratch provenance-pinned
redeployment after a real audit fix; an Open-mode Sale auction taken through
its full lifecycle, including one missed funding window recovered via
`expire`; a Sealed-mode Sale auction using independently-computed real
commitment hashes, with a non-reveal penalty resolved *after* its delivery
deadline to directly exercise the exact bug the C-19 fix closed; a Buyback
auction showing the mechanism's other direction, including a seller
correctly excluded for asking above the clearing price; an Open-mode Sale
auction where a bidder's SAC authorization is revoked mid-auction, forcing a
live exclusion-and-restart of the clearing epoch; permissionless `maintain()`
TTL renewal on all four contract types; and a second missed-deadline
recovery caused by a real, uncontrolled session interruption rather than a
scripted one. A handful of other attempts (one `fund`, two `place_bid`
calls, three `commit_bid` calls on the abandoned Auction B, one `place_bid`
on Auction F) were rejected during local simulation or never attempted
before their window closed, and so never reached the network or produced a
hash; they are listed inline above for an honest record of what was
attempted, not counted toward this total.
