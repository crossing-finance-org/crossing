# Crossing Protocol — Internal Cryptographic Review

Date: **22 October 2026**. Scope: the current `0.1.0` source, SDK `25.3.2`,
commitment/configuration encoding, deployment salt derivation, authorization,
replay prevention, and their regression evidence. This is an internal development
review, not an external audit, deployment approval, or production certification.

No exploitable cryptographic defect was confirmed in the reviewed source. The
review closed specific encoding and authorization assurance gaps by adding
independent fixed vectors and authenticated runtime regressions. Overall suite
results, resource measurements, and coverage belong to the
[testing record](../TESTING.md) and [contract review](CONTRACT_REVIEW.md); this
document does not substitute a separate total or coverage percentage.

The principal implementation references are the
[shared hashing helpers and wire types](../../contracts/common/src/lib.rs),
[auction commit/reveal and withdrawal paths](../../contracts/auction/src/lib.rs),
and [factory deployment derivation](../../contracts/auction-factory/src/lib.rs).
The configuration read refactor passes the same immutable configuration to
storage/TTL helpers; it does not change the commitment inputs or permit callers
to select replacement terms during reveal.

Configuration hashing is `SHA256(XDR(ScVal(Config)))`. The current structure has
22 named fields, including `nonreveal_penalty_bps`. The SDK serializes the record
as a map with sorted symbol keys. Enum values use their tagged ScVal encoding;
addresses include their account/contract discriminants. `i128` values use the
XDR signed high 64-bit part and unsigned low 64-bit part. JSON strings, display
addresses, decimal concatenation, and native machine integer layouts are not
the hash preimage.

The bid commitment is exactly:

```text
SHA256(XDR(ScVal::Vec([
  Symbol("CROSSING_BID_V1"),
  Bytes(network_id[32]), Address(auction), Address(owner), Bytes(nonce[32]),
  Bytes(config_hash[32]), I128(max_locked), I128(price), I128(quantity),
  Bytes(salt[32])
])))
```

The network identifier is the host's 32-byte network ID. The auction address is
the currently executing contract, not a caller-provided destination. Reveal
uses the stored owner, nonce, full deposit, and configuration hash. It checks
the current reveal window, unrevealed state, admissible price/quantity,
sufficient collateral, and the recorded owner's authorization before accepting
the opening. A copied commitment cannot change its owner, auction, terms, or
locked amount without changing the digest.

Typed, length-delimited XDR and the fixed tuple remove concatenation ambiguity.
Appending data to a SHA256 digest cannot yield a valid opening of this fixed
structure. These properties still depend on SHA256's cryptographic resistance;
the tests are conformance and attack regressions, not a proof of the primitive.

The factory separately derives its deployment salt from canonical ScVal XDR for
`(Symbol("CROSSING_AUC_V1"), network_id, factory, issuer, user_salt)`. The Soroban
deployer then determines the contract address. This namespaces issuer salts and
separates deployment inputs from bid commitments. The factory requires the
issuer to authorize the complete configuration and user salt.

The distinct nonce/salt mechanisms must not be confused:

| Value | Purpose and visibility |
|---|---|
| Bid `nonce: BytesN<32>` | Public, unique per owner within an auction; the persistent guard prevents another admission using that owner/nonce pair. It also binds the opening to its stored bid. |
| Soroban credential `nonce: i64` and expiration ledger | Host-enforced authorization replay protection. Both are included in the signed authorization payload. An expired credential is invalid, and changing expiration without re-signing invalidates the signature. |
| Bid `salt: BytesN<32>` | Secret entropy for hiding low-entropy prices and quantities until reveal. Clients must generate it with a cryptographically secure random source and preserve it securely. |
| Factory `user_salt: BytesN<32>` | Public deterministic deployment input; it is not a bid-opening secret. |

Commitments are hashes, not encryption. A predictable salt permits offline
guessing of plausible bid values. A public nonce does not repair a weak salt.
The contract cannot establish that a user chose a random secret. Bidder identity,
deposit size, timing, and count remain public, and deposits can constrain the
possible quantity. The fixed secrets in regression fixtures are test-only;
production secret generation, encrypted recovery, wallet UX, and secret logging
remain outside this contract implementation.

The [independent Node encoder](../../tools/design/check_commitment.mjs) implements
ScVal XDR directly, without importing a Stellar SDK or production hashing helper.
Its default command checks the existing
[single vector](../../tests/vectors/commitment-v1.json) and the new
[36-case matrix](../../tests/vectors/commitment-domains.json):

- The existing Sale/account-owner case, plus Buyback with a contract owner.
- Positive amounts crossing the 64-bit boundary and reaching `i128::MAX`.
- One mutation for each of the 22 configuration fields.
- One mutation for each of the 10 commitment components, including the domain
  symbol, account/contract owner tag, and bound configuration hash.

Each matrix case records its typed inputs, expected configuration digest, bound
configuration digest, commitment, and complete preimage XDR. Configuration
mutation cases test wire encoding; some deliberately violate admission rules.
The appended [Rust conformance test](../../contracts/common/src/test.rs) compares
the production helpers with fixed expected hashes obtained from that independent
encoder. Its altered-domain case separately verifies that the production helper
continues to use the fixed V1 domain. The original Rust vector test remains.
`--print` retains its original single-vector output; `--print-domains` explicitly
prints the matrix. Neither command writes or silently refreshes expected files.

Four tests in the
[cryptographic runtime module](../../contracts/auction/tests/scenarios/cryptographic_tests.rs)
passed in the coordinated compiled-WASM auction run against genuine issued SACs:

| Regression | Evidence established |
|---|---|
| `real_ed25519_credentials_bind_calls_network_expiration_and_replay_nonce` | Real Ed25519 signature verification through a test custom account; rejection of forgery, changed root arguments, an incorrect signed SAC transfer child, changed nonce/expiration, expired credentials, and another network. Failed calls preserve funds and allow the authentic credential to retry. Successful deposits, nonce replay rejection, a fresh authorization, delivery, and signed credit withdrawal complete. |
| `signed_authentication_callback_cannot_reenter_and_healthy_retry_preserves_ownership` | An authenticated account callback attempts reentry. The test distinguishes an auction `Reentry` error from a host invocation rejection, verifies rollback of bids/indexes/ticks/accounting/balances, then disarms the callback and successfully retries the same signed credential. A native observation records the attempted rejection outside transaction storage; no financial state depends on it. |
| `classic_account_master_key_signatures_drive_real_trustline_debits_and_reject_replay` | Real classic account and trustline ledger entries, the classic signature-vector format, a master-key signature, forgery/replay rejection, actual trustline debits and credits, delivery, and signed refund withdrawal. No custom-account callback substitutes for classic account authentication. |
| `runtime_commitments_bind_network_nonce_locked_and_all_config_terms` | Wrong-network, nonce, locked amount, configuration, and penalty openings fail without changing aggregates or ownership. Correct controls reveal and settle; restoration of the original network permits a valid opening. Final withdrawals drain both liabilities. |

These tests submit signed **Soroban contract authorization entries**, whose
payload is SHA256 of `HashIdPreimage::SorobanAuthorization` XDR. That payload is
different from the bid's ScVal XDR and includes network ID, credential nonce,
signature expiration, and the authorized invocation tree. `require_auth()` binds
the current function's complete arguments; every required nested transfer must
also match its authorized invocation.

They do **not** submit signed network transaction envelopes. They do not prove
fee-payer signing, account sequence-number handling, transaction submission,
network acceptance, wallet integration, or RPC behavior. Account/trustline setup
is seeded through the test host, not through network account/trust operations.
The custom account fixtures are native test contracts; their callbacks use the
host's real Ed25519 verification, but the tests do not measure a deployed account
WASM's resource fit. Classic coverage includes a master-key account and the
2-of-2 account described below; it does not cover every signer, sponsorship,
threshold, or liability arrangement.

The assurance findings and their disposition are:

| Finding | Disposition |
|---|---|
| A single fixed vector did not establish every configuration/domain field or boundary encoding. | Add the 36 independent cases and fixed-hash Rust conformance test; preserve the existing vector. |
| Matching mock invocation trees did not establish actual Ed25519 verification, credential expiration/replay, classic account behavior, or authentication-callback reentry. | Add the four signed/runtime regressions above. The three signature-path regressions use `set_auths` with real bidder signatures. Administrative setup still uses explicit mocks; the domain-mutation regression uses exact invocation mocks to isolate the commitment verifier. |
| The new penalty field changes the configuration ABI while the current configuration version remains `1`. | Treat the current artifact/schema as explicit release identity. SDK decoding requires the exact field set, so an older payload missing the field fails closed. The penalty is included in config hashing and exact creation authorization. This is a compatibility requirement, not a demonstrated replay or unauthorized-penalty exploit. |

Immutable deployments are not migrated by changing source or replacing a vector.
Clients and release manifests must identify the network, addresses, exact WASM
hashes, and schema, rather than relying only on the numeric configuration version.
This review does not certify asset-administrator cooperation, oracle economic
suitability, external services, or private-key custody. The related financial and
protocol review remains in the [contract review](CONTRACT_REVIEW.md) and
[auction logic review](AUCTION_LOGIC_REVIEW.md).

Two further regressions in
[classic authority tests](../../contracts/auction/tests/scenarios/classic_authority_tests.rs)
exercise genuine classic signatures beyond the simple bidder master key:

- A classic current SAC administrator signs exact factory Config/salt, authorizes
  the issued SAC escrow, and funds both Sale and Buyback through the signed SAC
  transfer child. Mutated terms/salt and child amounts fail. Losing admin control
  after creation blocks funding; restoring it permits retry with the same
  unconsumed credential nonce. A classic admin is deliberately distinct from the
  immutable underlying asset issuer.
- A 2-of-2 classic account rejects empty/insufficient signatures, a disabled
  master key, duplicate or unsorted public keys, outsiders, expiry, and replay.
  Failed transfer-child authorization rolls back the nonce and escrow; valid
  signatures then fund a bid, receive actual delivery, and withdraw the owned
  refund. Both liabilities drain completely.

These use real signature verification and account/trustline/signer ledger
entries seeded through the local host. They do not submit network setup or
transaction envelopes, cover every signer arrangement, or measure maximum-book
resources with classic trustlines. The earlier four regressions remain in place.

Reproduce the focused checks with:

```bash
node tools/design/check_commitment.mjs
cargo test -p crossing-common --locked independent_commitment_matrix
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked cryptographic_tests:: -- --test-threads=1
```

Build the current WASM artifacts before the runtime command; the complete workflow
and evidence limitations are documented in [testing](../TESTING.md).
