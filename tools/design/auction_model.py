"""Executable specification, NOT a Soroban contract or canonical-XDR implementation.

Uses Python arbitrary precision as the arithmetic oracle. Contract implementation
must add checked i128 storage, 256-bit intermediates, real SAC/auth/TTL/rollback
and XDR golden-vector tests. This model validates economics and liability flows.
"""
from __future__ import annotations

from collections import defaultdict
from dataclasses import dataclass
from typing import Optional

I128_MAX = (1 << 127) - 1
ASSETS = ("rwa", "quote")


def mul_div(a: int, b: int, denominator: int, *, ceiling: bool = False) -> int:
    if not (0 <= a <= I128_MAX and 0 <= b <= I128_MAX and denominator > 0):
        raise ValueError("invalid arithmetic operands")
    quotient, remainder = divmod(a * b, denominator)
    result = quotient + int(ceiling and remainder != 0)
    if result > I128_MAX:
        raise OverflowError("i128 result overflow")
    return result


@dataclass(frozen=True)
class Config:
    mode: str
    supply: int
    min_price: int
    max_price: int
    tick_size: int = 1
    scale: int = 10_000_000
    min_fill: int = 1
    min_bid: int = 1
    min_deposit: int = 1
    max_bids: int = 16
    per_address: int = 16
    sealed: bool = False
    nonreveal_penalty_bps: int = 0
    start: int = 10
    end: int = 20
    reveal_end: int = 20
    close_deadline: int = 40
    delivery_deadline: int = 60

    def __post_init__(self) -> None:
        if self.mode not in ("sale", "buyback"):
            raise ValueError("mode")
        if not 0 <= self.nonreveal_penalty_bps <= 5_000 or (not self.sealed and self.nonreveal_penalty_bps):
            raise ValueError("non-reveal penalty")
        values = (self.supply, self.min_price, self.max_price, self.tick_size,
                  self.scale, self.min_fill, self.min_bid, self.min_deposit)
        if not all(0 < value <= I128_MAX for value in values):
            raise ValueError("positive i128 values required")
        if self.max_price < self.min_price or (self.max_price-self.min_price) % self.tick_size:
            raise ValueError("grid")
        if self.tick_count > 4096 or not (1 <= self.per_address <= self.max_bids <= 16):
            raise ValueError("capacity")
        if not (0 < self.start < self.end <= self.reveal_end < self.close_deadline < self.delivery_deadline < 2**32):
            raise ValueError("schedule")
        if self.sealed != (self.reveal_end > self.end):
            raise ValueError("reveal schedule")
        if mul_div(self.min_bid, self.min_price, self.scale) < 1:
            raise ValueError("minimum bid has zero value")
        affordable = self.supply if self.mode == "sale" else mul_div(self.supply, self.scale, self.min_price)
        if self.min_fill > affordable:
            raise ValueError("unreachable minimum fill")

    @property
    def tick_count(self) -> int:
        return 1 + (self.max_price-self.min_price)//self.tick_size

    @property
    def deposit_asset(self) -> str:
        return "quote" if self.mode == "sale" else "rwa"

    @property
    def issuer_asset(self) -> str:
        return "rwa" if self.mode == "sale" else "quote"

    def cost(self, quantity: int, price: int) -> int:
        return mul_div(quantity, price, self.scale, ceiling=self.mode == "sale")

    def validate_bid(self, price: int, quantity: int) -> None:
        if not (self.min_price <= price <= self.max_price) or (price-self.min_price) % self.tick_size:
            raise ValueError("bid price")
        if not (self.min_bid <= quantity <= I128_MAX):
            raise ValueError("bid quantity")


@dataclass
class Bid:
    owner: str
    deposit: int
    price: Optional[int] = None
    quantity: Optional[int] = None
    excluded: bool = False
    resolved: bool = False
    outcome: Optional[str] = None
    reason: Optional[str] = None


@dataclass(frozen=True)
class Candidate:
    price: int
    target: int
    numerator: int
    denominator: int


@dataclass(frozen=True)
class Result:
    candidate: Optional[Candidate]
    allocations: tuple[int, ...]
    payments: tuple[int, ...]

    @property
    def filled(self) -> int:
        return sum(self.allocations)

    @property
    def paid(self) -> int:
        return sum(self.payments)


def active(bid: Bid) -> bool:
    return bid.quantity is not None and not bid.excluded


class TickWalker:
    """Resumable pure tick walk. The state can be serialized between every step."""
    def __init__(self, config: Config, bids: list[Bid]):
        if len(bids) > config.max_bids:
            raise ValueError("book capacity")
        self.config = config
        self.totals: dict[int, int] = defaultdict(int)
        for bid in bids:
            if active(bid):
                assert bid.price is not None and bid.quantity is not None
                config.validate_bid(bid.price, bid.quantity)
                self.totals[bid.price] += bid.quantity
        if sum(self.totals.values()) > I128_MAX:
            raise OverflowError("aggregate quantity")
        self.cursor = config.tick_count-1 if config.mode == "sale" else 0
        self.cumulative = 0
        self.previous: Optional[int] = None
        self.done = False
        self.candidate: Optional[Candidate] = None
        self.processed = 0

    def step(self, requested: int) -> None:
        if requested <= 0:
            raise ValueError("work limit")
        for _ in range(min(requested, 32)):
            if self.done:
                return
            c = self.config
            p = c.min_price + self.cursor*c.tick_size
            q = self.totals.get(p, 0)
            self.processed += 1
            if c.mode == "sale":
                if self.cumulative+q >= c.supply:
                    self.candidate = Candidate(p, c.supply, c.supply-self.cumulative, q)
                    self.done = True
                    return
                self.cumulative += q
                self.cursor -= 1
                if self.cursor < 0:
                    if self.cumulative:
                        self.candidate = Candidate(c.min_price, self.cumulative, 1, 1)
                    self.done = True
            else:
                if q:
                    capacity = mul_div(c.supply, c.scale, p)
                    if capacity <= self.cumulative:
                        if self.cumulative:
                            assert self.previous is not None
                            self.candidate = Candidate(self.previous, self.cumulative, 1, 1)
                        self.done = True
                        return
                    if self.cumulative+q >= capacity:
                        self.candidate = Candidate(p, capacity, capacity-self.cumulative, q)
                        self.done = True
                        return
                    self.cumulative += q
                    self.previous = p
                self.cursor += 1
                if self.cursor >= c.tick_count:
                    if self.cumulative:
                        assert self.previous is not None
                        self.candidate = Candidate(self.previous, self.cumulative, 1, 1)
                    self.done = True


def allocate(config: Config, bids: list[Bid], candidate: Optional[Candidate]) -> Result:
    quantities, payments = [], []
    for bid in bids:
        f = 0
        if candidate is not None and active(bid):
            assert bid.price is not None and bid.quantity is not None
            better = bid.price > candidate.price if config.mode == "sale" else bid.price < candidate.price
            if better:
                f = bid.quantity
            elif bid.price == candidate.price:
                f = mul_div(bid.quantity, candidate.numerator, candidate.denominator)
        payment = config.cost(f, candidate.price) if candidate else 0
        if not payment:
            f = 0
        quantities.append(f)
        payments.append(payment)
    if not sum(quantities) or sum(quantities) < config.min_fill:
        return Result(None, tuple(0 for _ in bids), tuple(0 for _ in bids))
    return Result(candidate, tuple(quantities), tuple(payments))


def find_candidate(config: Config, bids: list[Bid], chunk: int = 32) -> Optional[Candidate]:
    walker = TickWalker(config, bids)
    while not walker.done:
        walker.step(chunk)
    return walker.candidate


def clear(config: Config, bids: list[Bid], chunk: int = 32) -> Result:
    return allocate(config, bids, find_candidate(config, bids, chunk))


class AuctionModel:
    """Accounting/state reference with abstract authorizations, not cryptography.

    Transfer failures are preflighted to model transaction atomicity. Genuine
    Soroban integration tests must establish actual trap/rollback behavior.
    """
    def __init__(self, config: Config, buyback_recipient: str = "treasury"):
        if config.mode == "buyback" and buyback_recipient in ("auction", "factory", "registry"):
            raise ValueError("unsafe recipient")
        self.config = config
        self.recipient = buyback_recipient
        self.ledger = 1
        self.state = "created"
        self.bids: list[Bid] = []
        self.balance = {a: 0 for a in ASSETS}
        self.liability = {a: 0 for a in ASSETS}
        self.pending = {a: 0 for a in ASSETS}
        self.reserve = {a: 0 for a in ASSETS}
        self.credits: dict[tuple[str, str], int] = defaultdict(int)
        self.delivered: dict[tuple[str, str], int] = defaultdict(int)
        self.denied: set[tuple[str, str]] = set()
        self.blocked: set[tuple[str, str]] = set()
        self.result: Optional[Result] = None
        self.ready: Optional[Result] = None
        self.prepared_candidate: Optional[Candidate] = None
        self.executed_quantity = 0
        self.executed_quote = 0
        self.exclusion_count = 0

    def authorized(self, owner: str, asset: str) -> bool:
        return (owner, asset) not in self.denied

    def _check_authorized(self, owner: str) -> None:
        if not all(self.authorized(owner, a) for a in ASSETS):
            raise ValueError("unauthorized")

    def _add_credit(self, owner: str, asset: str, amount: int) -> None:
        assert amount >= 0
        self.credits[owner, asset] += amount

    def fund(self, caller: str = "issuer") -> None:
        if caller != "issuer" or self.state != "created" or self.ledger >= self.config.start:
            raise ValueError("fund")
        self._check_authorized("issuer")
        self._check_authorized("auction")
        if self.config.mode == "buyback" and not self.authorized(self.recipient, "rwa"):
            raise ValueError("recipient")
        a = self.config.issuer_asset
        self.balance[a] = self.liability[a] = self.reserve[a] = self.config.supply
        self.state = "funded"
        self.assert_invariants()

    def _admit(self, owner: str, deposit: int, price: Optional[int], quantity: Optional[int]) -> int:
        c = self.config
        if self.state != "funded" or not c.start <= self.ledger < c.end:
            raise ValueError("placement window")
        if owner in ("issuer", self.recipient, "auction", "factory", "registry"):
            raise ValueError("self bid")
        self._check_authorized(owner)
        self._check_authorized("auction")
        if len(self.bids) >= c.max_bids or sum(b.owner == owner for b in self.bids) >= c.per_address:
            raise ValueError("capacity")
        if not c.min_deposit <= deposit <= I128_MAX:
            raise ValueError("deposit")
        a = c.deposit_asset
        if self.pending[a]+deposit > I128_MAX:
            raise OverflowError("deposit total")
        # Reserve enough integer quantity capacity for every accepted hidden
        # order, using only its public collateral and the lowest allowed price.
        if c.sealed and c.mode == "sale" and (self.pending[a]+deposit)*c.scale//c.min_price > I128_MAX:
            raise OverflowError("sealed reveal capacity")
        if quantity is not None and sum(b.quantity or 0 for b in self.bids)+quantity > I128_MAX:
            raise OverflowError("quantity total")
        self.bids.append(Bid(owner, deposit, price, quantity))
        self.balance[a] += deposit
        self.liability[a] += deposit
        self.pending[a] += deposit
        self.assert_invariants()
        return len(self.bids)-1

    def place(self, owner: str, price: int, quantity: int) -> int:
        if self.config.sealed:
            raise ValueError("sealed auction")
        self.config.validate_bid(price, quantity)
        deposit = self.config.cost(quantity, price) if self.config.mode == "sale" else quantity
        return self._admit(owner, deposit, price, quantity)

    def commit(self, owner: str, deposit: int) -> int:
        """Only collateral/non-reveal semantics; does NOT model commitment XDR."""
        if not self.config.sealed:
            raise ValueError("open auction")
        return self._admit(owner, deposit, None, None)

    def reveal(self, bid_id: int, price: int, quantity: int, caller: str) -> None:
        c, bid = self.config, self.bids[bid_id]
        if self.state != "funded" or not c.sealed or not c.end <= self.ledger < c.reveal_end:
            raise ValueError("reveal window")
        if caller != bid.owner or bid.quantity is not None:
            raise ValueError("reveal authority/replay")
        c.validate_bid(price, quantity)
        needed = c.cost(quantity, price) if c.mode == "sale" else quantity
        if needed > bid.deposit:
            raise ValueError("undercollateralized reveal")
        if sum(b.quantity or 0 for b in self.bids)+quantity > I128_MAX:
            raise OverflowError("quantity total")
        bid.price, bid.quantity = price, quantity

    def prepare(self, chunk: int = 32) -> None:
        c = self.config
        if self.state != "funded" or not c.reveal_end <= self.ledger < c.close_deadline:
            raise ValueError("closing window")
        # Retain nominal price even when integer allocations fail the minimum:
        # revoked zero-fill contributors must be removed before cancellation.
        self.prepared_candidate = find_candidate(c, self.bids, chunk)
        self.ready = allocate(c, self.bids, self.prepared_candidate)

    def _cancel(self) -> None:
        for a in ASSETS:
            self._add_credit("issuer", a, self.reserve[a])
            self.reserve[a] = 0
        self.state = "cancelled" if self.bids else "refunded"
        self.result = None
        self.ready = None
        self.prepared_candidate = None
        self.assert_invariants()

    def finalize(self) -> bool:
        c = self.config
        if self.state != "funded" or self.ready is None or self.ledger >= c.close_deadline:
            raise ValueError("finalization")
        r = self.ready
        if not all(self.authorized("auction", a) for a in ASSETS) or (
            c.mode == "buyback" and not self.authorized(self.recipient, "rwa")
        ):
            self._cancel()
            return True
        candidate = self.prepared_candidate
        for bid in self.bids:
            contributes = candidate is not None and active(bid) and (
                bid.price >= candidate.price if c.mode == 'sale' else bid.price <= candidate.price
            )
            if contributes and not all(self.authorized(bid.owner, a) for a in ASSETS):
                bid.excluded = True
                self.exclusion_count += 1
                self.ready = None
                self.prepared_candidate = None
                return False
        if r.candidate is None:
            self._cancel()
            return True
        self.result, self.ready = r, None
        self.prepared_candidate = None
        a = c.issuer_asset
        required = r.filled if c.mode == "sale" else r.paid
        self._add_credit("issuer", a, self.reserve[a]-required)
        self.reserve[a] = required
        self.state = "cleared"
        self.assert_invariants()
        return True

    def close(self, chunk: int = 32) -> None:
        for _ in range(len(self.bids)+1):
            self.prepare(chunk)
            if self.finalize():
                return
        raise AssertionError("exclusion termination bound")

    def expire_close(self) -> None:
        if self.state == "created" and self.ledger >= self.config.start:
            self._cancel()
        elif self.state == "funded" and self.ledger >= self.config.close_deadline:
            self._cancel()
        else:
            raise ValueError("expiry")

    def _transfer_preflight(self, transfers: list[tuple[str, str, int]]) -> None:
        if any(self.balance[a] < self.liability[a] for a in ASSETS):
            raise ValueError("insolvent")
        for owner, asset, amount in transfers:
            if amount and (not self.authorized("auction", asset) or
                           not self.authorized(owner, asset) or (owner, asset) in self.blocked):
                raise ValueError("blocked transfer")

    def _send(self, owner: str, asset: str, amount: int) -> None:
        self.balance[asset] -= amount
        self.liability[asset] -= amount
        self.delivered[owner, asset] += amount

    def resolve(self, bid_id: int, *, expire: bool = False) -> None:
        c, bid = self.config, self.bids[bid_id]
        if self.state not in ("cleared", "cancelled") or bid.resolved:
            raise ValueError("resolution/replay")
        if expire != (self.ledger >= c.delivery_deadline):
            raise ValueError("delivery boundary")
        f = self.result.allocations[bid_id] if self.result else 0
        payment = self.result.payments[bid_id] if self.result else 0
        allowed = all(self.authorized(bid.owner, a) and self.authorized("auction", a) for a in ASSETS)
        if c.mode == "buyback":
            allowed = allowed and self.authorized(self.recipient, "rwa")
        # Cancellation refunds fully. On a cleared auction, a non-reveal keeps
        # its penalty before and after delivery expiry, matching the contract.
        reason = ("cancelled" if self.result is None else
                  "nonreveal" if bid.quantity is None else
                  "delivery_expired" if expire else
                  "revoked" if bid.excluded else
                  "not_allocated" if not f else
                  "revoked" if not allowed else None)
        refund = reason is not None
        transfers: list[tuple[str, str, int]] = []
        if not refund:
            if c.mode == "sale":
                transfers = [(bid.owner, "rwa", f)]
            else:
                transfers = [(self.recipient, "rwa", f), (bid.owner, "quote", payment)]
            self._transfer_preflight(transfers)
        # Everything below is deterministic model bookkeeping after preflight.
        # Actual contracts must prove all-effects rollback against real SACs.
        self.pending[c.deposit_asset] -= bid.deposit
        self.reserve[c.issuer_asset] -= f if c.mode == "sale" else payment
        if refund:
            penalty = bid.deposit*c.nonreveal_penalty_bps//10_000 if reason == "nonreveal" else 0
            self._add_credit(bid.owner, c.deposit_asset, bid.deposit-penalty)
            self._add_credit("issuer", c.deposit_asset, penalty)
            self._add_credit("issuer", c.issuer_asset, f if c.mode == "sale" else payment)
        elif c.mode == "sale":
            self._add_credit(bid.owner, "quote", bid.deposit-payment)
            self._add_credit("issuer", "quote", payment)
        else:
            self._add_credit(bid.owner, "rwa", bid.deposit-f)
        for owner, asset, amount in transfers:
            self._send(owner, asset, amount)
        bid.resolved, bid.outcome = True, "refund" if refund else "delivered"
        bid.reason = reason
        if not refund:
            self.executed_quantity += f
            self.executed_quote += payment
        if all(b.resolved for b in self.bids):
            self.state = "settled" if self.result else "refunded"
        self.assert_invariants(allow_deficit=True)

    def withdraw(self, owner: str, asset: str, amount: int, caller: str) -> None:
        if caller != owner or asset not in ASSETS or amount <= 0 or self.credits[owner, asset] < amount:
            raise ValueError("withdrawal")
        self._transfer_preflight([(owner, asset, amount)])
        self.credits[owner, asset] -= amount
        self._send(owner, asset, amount)
        self.assert_invariants()

    def assert_invariants(self, *, allow_deficit: bool = False) -> None:
        for a in ASSETS:
            credits = sum(n for (_, asset), n in self.credits.items() if asset == a)
            assert self.pending[a] >= 0 and self.reserve[a] >= 0
            assert all(n >= 0 for n in self.credits.values())
            assert self.liability[a] == self.pending[a]+self.reserve[a]+credits
            if not allow_deficit:
                assert self.balance[a] >= self.liability[a]
        if self.result:
            if self.config.mode == "sale":
                assert self.result.filled <= self.config.supply
            else:
                assert self.result.paid <= self.config.supply
