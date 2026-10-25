# Crossing Protocol

**Single-price auctions for private-market RWAs on Stellar.**

Crossing Protocol is a set of Soroban smart contracts for selling and buying back
tokenized real-world assets. An issuer brings an auction to market, participants
submit funded bids, and the contracts bring those bids together at one clearing
price. The mechanism supports assets such as private credit, real estate
interests, and structured notes that may trade only occasionally.

Both sides commit assets before trading: the issuer escrows the inventory or
buyback budget, and each participant funds its bid when placing it. Completed
trades exchange those escrowed assets at the common clearing price. Auction
results and actual execution totals can then be published on-chain.

[Run the PoC](docs/POC_DEMO.md) · [Architecture](ARCHITECTURE.md) ·
[Contract API](docs/reference/CONTRACT_API.md) ·
[Testnet evidence](deployments/TESTNET_TRANSACTIONS.md) ·
[Documentation](docs/README.md)

## Why an auction for private-market assets?

An asset can have willing buyers and sellers without enough activity to sustain
continuous trading. A private-credit position, a property interest, or a
structured note may change hands around a financing event, a liquidity window,
or an issuer buyback.

Crossing collects that interest over a defined bidding window. Participants do
not need to arrive simultaneously or negotiate a separate execution price for
every trade. Their bids form one auction book, and eligible allocations share
one clearing price. This gives the issuer a structured way to distribute or
repurchase tokens and participants a common basis for execution.

The resulting observation also makes an otherwise infrequent trade visible to
other applications. It records what that auction cleared at, together with how
much actually traded.

## Sales and buybacks

In a **sale**, the issuer escrows a fixed quantity of RWA tokens. Buyers submit
quantities and maximum prices, backed by deposits in the settlement asset.
Higher-priced bids receive priority. If demand exceeds the available inventory,
the marginal price determines the clearing price and equal-price marginal bids
share the remaining quantity pro rata. An undersubscribed sale uses its reserve
price if the minimum-fill condition is met.

In a **buyback**, the issuer escrows a settlement-asset budget. Token holders
submit the quantities they want to sell and their minimum prices, depositing the
RWA tokens with their bids. Lower-priced asks receive priority. The contracts
choose allocations at a common price while keeping total payments within the
funded budget.

Prices are denominated in a Stellar-issued settlement asset, such as a
stablecoin. Each auction fixes its RWA token, settlement asset, price range,
minimum fill, and deadlines when it is created.

### One price, multiple participants

Suppose an issuer offers **10 RWA tokens**:

- Alice bids for 6 tokens at a maximum price of 9 settlement tokens each,
  depositing 54.
- Bob bids for 8 tokens at a maximum price of 7, depositing 56.

The auction clears at **7**. Alice receives 6 RWA tokens for 42, and Bob receives
4 for 28. Both pay the same unit price. Alice's unused deposit of 12 and Bob's
unused deposit of 28 become credits they can withdraw. The issuer receives
70 settlement tokens after the corresponding deliveries.

This example is an executable scenario in the [local PoC](docs/POC_DEMO.md).
The architecture specifies the exact [clearing and rounding rules](ARCHITECTURE.md#7-clearing-mathematics),
including integer pro-rata allocation and buyback budget limits.

## Open or sealed bidding

An issuer can choose **open bids**, whose prices and quantities are visible as
they are placed, or **sealed bids** using a commitment-and-reveal process.

With sealed bidding, participants first deposit collateral and submit a hash
binding their bid details to the auction. After bidding ends, they reveal the
price, quantity, and secret needed to verify that commitment. Valid reveals enter
the clearing calculation. Revealing requires a transaction from the bidder;
there is no automatic decryption or reveal service.

Sealed bidding hides the committed price and quantity before reveal. Deposits,
participant addresses, and transaction timing remain public. Unrevealed bids do
not contribute to the clearing price. A cleared auction may apply a previously
configured non-reveal penalty of up to 50%; the default is zero. Cancellation
refunds bids in full.

## From funding to settlement

1. **Create the auction.** The RWA token's current Stellar Asset Contract (SAC)
   administrator authorizes an immutable configuration through the factory.
2. **Fund the escrow.** After the escrow is authorized on both assets, the issuer
   deposits the sale inventory or buyback budget before bidding starts.
3. **Place funded bids.** Buyers deposit settlement tokens for a sale; sellers
   deposit RWA tokens for a buyback. Sealed participants reveal in the later
   reveal window.
4. **Determine one price.** Permissionless, bounded transactions rank the book,
   check eligible price contributors, and finalize the clearing result.
5. **Claim and withdraw.** Participants resolve their allocations independently.
   Successful claims deliver assets; refunds and proceeds become credits owned by
   the original account and withdrawable in the corresponding asset.
6. **Publish the outcome.** Anyone can submit the transactions that publish
   verified clearing and execution observations to the registry.

Funding removes the need to collect a winning bidder's payment after the auction.
Delivery still depends on the asset's authorization rules and successful claim
transactions. For a sale, issuer proceeds are credited only after the
corresponding RWA delivery. For a buyback, both transfer legs succeed together
or the invocation rolls back.

## Compliance through the asset itself

Crossing uses the authorization rules of the underlying SACs. Participants must
be authorized on both assets when bidding. Before fixing the price, finalization
checks every price-contributing bid atomically, including marginal bids whose
individual allocation rounds to zero. Revoked contributors are excluded and
clearing restarts without their demand or supply. Delivery checks authorization
again at each claim.

These checks keep auction participation and delivery aligned with token-level
permissions. The asset administrator retains its existing freeze and clawback
powers. A freeze can delay a withdrawal, and a clawback can leave escrow short
of assets; funded bids do not remove those external asset risks. The contracts
track liabilities and block outflows while a deficit exists.

## Public prices with execution context

The **PriceRegistry** records two separate observations: the auction's clearing
result and its actual execution summary. This distinction matters because an
allocation can remain undelivered, for example when authorization changes before
a claim.

The **OracleAdapter** exposes the registry's latest clearing observation for a
fixed settlement asset through a read-only, partial SEP-40 interface. Applications
can use the published records as inputs to their own pricing policies. They must
consider execution volume, freshness, market participation, and manipulation
risk when deciding how much weight to give an observation.

## The contracts

- **[Auction](contracts/auction/src/lib.rs)** holds escrow, accepts bids, computes
  clearing, and handles delivery, credits, refunds, and expiry.
- **[AuctionFactory](contracts/auction-factory/src/lib.rs)** creates
  issuer-authorized auction instances and records their provenance.
- **[PriceRegistry](contracts/price-registry/src/lib.rs)** verifies auction
  provenance and publishes clearing and execution observations.
- **[OracleAdapter](contracts/oracle-adapter/src/lib.rs)** provides read access to
  registry prices in a fixed base asset.

The [shared library](contracts/common/src/lib.rs) defines wire types, checked
arithmetic, commitment encoding, and common validation. The
[architecture](ARCHITECTURE.md) explains how these contracts fit together and
specifies the wider application and integration design.

## Quick start

Install Rust through Rustup, Python 3.10+, and Node.js 18+; CI uses Node 22.
The repository selects Rust **1.94.1** and pins Soroban SDK **25.3.2**.

```bash
git clone https://github.com/crossing-finance-org/crossing.git
cd crossing
rustup show
rustup component add rustfmt clippy
rustup target add wasm32v1-none
./scripts/demo.sh
```

The demonstration builds the contracts and executes three asserted scenarios
against genuine SACs in Soroban's local test host:

- An open sale with two winners paying one price, followed by asset delivery,
  refund withdrawals, and publication of the execution result.
- A sealed buyback that rejects an altered reveal, executes valid asks at one
  price, and returns a non-revealer's deposit under the zero-penalty policy.
- A compliance change that excludes a revoked winner and recomputes clearing.

It runs locally without a wallet, RPC endpoint, or Testnet funds. The first build
needs access to the locked dependencies. Cached environments can use
`CARGO_NET_OFFLINE=true ./scripts/demo.sh`. See the
[PoC guide](docs/POC_DEMO.md) for the inputs, assertions, and expected results.

### Testnet evidence

The contracts have also been deployed to and exercised on Stellar Testnet. The
[deployment manifest](deployments/testnet.json) records contract addresses, WASM
hashes, source provenance, and build inputs. The
[transaction log](deployments/TESTNET_TRANSACTIONS.md) contains **106 transaction
references**, covering deployment, funded auctions, reveals, compliance changes,
delivery, deadline recovery, withdrawals, and registry publication.

These public-network records complement the reproducible local demonstration.
The repository contains the contract code, test fixtures, demo scenarios, and
technical review records needed to inspect the PoC.

## Verify the implementation

```bash
./scripts/check.sh
```

This runs formatting, locked WASM builds, Rust tests, Clippy, economic-model
checks, independent commitment and clearing vectors, tooling regressions, and
documentation checks. The suites contain **60 integration tests**, **9 shared
Rust unit tests**, **50 design tests**, and **15 tooling tests**. The integration
suite includes 48 independently generated clearing books. Larger generated-book
and accounting-lifecycle comparisons run against the independent design models.

For source coverage:

```bash
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
./scripts/coverage.sh
```

Coverage gates assess each production source separately at 95%, and additionally
require at least 95% unadjusted raw line coverage in each deployable contract.
[Testing and coverage](docs/TESTING.md) explains native instrumentation, the
compiled-WASM evidence, precise SDK exclusions, and measured results. GitHub CI
runs the checks and retains coverage reports and the tested WASMs with their
checksums and build metadata. Download these artifacts from the
[Contracts workflow](https://github.com/crossing-finance-org/crossing/actions/workflows/ci.yml).

## Scope and further review

This repository is a working smart-contract PoC exercised locally and on Testnet.
The current release supports **16 total bids per auction**, including unrevealed
commitments, and genuine issued SAC assets with seven decimals. Closing, claims,
publication, and maintenance require transactions from someone willing to submit
them.

The [contract review](docs/security/CONTRACT_REVIEW.md),
[auction logic review](docs/security/AUCTION_LOGIC_REVIEW.md), and
[findings and fixes](docs/security/FINDINGS_AND_FIXES.md) describe the reviewed
behavior and evidence. Mainnet qualification and an independent external audit
remain separate release requirements. Report vulnerabilities through
[SECURITY.md](SECURITY.md).

For implementation details and contribution guidance, start with the
[documentation index](docs/README.md), [contract API](docs/reference/CONTRACT_API.md),
and [CONTRIBUTING.md](CONTRIBUTING.md).

Licensed under [Apache-2.0](LICENSE).
