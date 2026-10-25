"""Design regression checks. Run: python3 -m unittest discover -s tests/design -v"""
from __future__ import annotations

import copy
import itertools
import json
from pathlib import Path
import random
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools' / 'design'))
from auction_model import (ASSETS, I128_MAX, AuctionModel, Bid, Config, TickWalker,
                           clear, mul_div)


def config(mode='sale', supply=10, **kwargs):
    values = dict(min_price=1, max_price=10, scale=1)
    values.update(kwargs)
    return Config(mode, supply, **values)


def bids_for(c, entries):
    return [Bid(f'b{i}', c.cost(q, p) if c.mode == 'sale' else q, p, q)
            for i, (p, q) in enumerate(entries)]


def funded(c, entries=()):
    model = AuctionModel(c)
    model.fund()
    model.ledger = c.start
    for i, (p, q) in enumerate(entries):
        model.place(f'b{i}', p, q)
    return model


def closed(c, entries):
    model = funded(c, entries)
    model.ledger = c.reveal_end
    model.close()
    return model


def drain(model):
    for (owner, asset), amount in list(model.credits.items()):
        if amount:
            model.withdraw(owner, asset, amount, owner)
    model.assert_invariants()
    assert all(n == 0 for n in model.liability.values())
    assert all(n == 0 for n in model.balance.values())


class ClearingRegressions(unittest.TestCase):
    def test_sale_uniform_price(self):
        c = config(supply=10)
        r = clear(c, bids_for(c, [(9, 6), (7, 8), (2, 4)]))
        self.assertEqual(r.candidate.price, 7)
        self.assertEqual(r.allocations, (6, 4, 0))
        self.assertEqual(r.payments, (42, 28, 0))

    def test_partial_sale_uses_reserve_without_double_counting(self):
        c = config(supply=20, min_price=2)
        r = clear(c, bids_for(c, [(9, 3), (4, 2), (2, 4)]))
        self.assertEqual(r.candidate.price, 2)
        self.assertEqual(r.allocations, (3, 2, 4))
        self.assertEqual(r.paid, 18)

    def test_buyback_reprices_every_accepted_unit(self):
        c = config('buyback', 100, max_price=20)
        r = clear(c, bids_for(c, [(10, 4), (20, 4)]))
        self.assertEqual(r.candidate.price, 20)
        self.assertEqual(r.allocations, (4, 1))
        self.assertEqual(r.paid, 100)

    def test_buyback_sparse_price_gap(self):
        c = config('buyback', 100, max_price=20)
        r = clear(c, bids_for(c, [(10, 8), (20, 1)]))
        self.assertEqual(r.candidate.price, 10)
        self.assertEqual(r.allocations, (8, 0))
        self.assertEqual(r.paid, 80)

    def test_buyback_zero_nominal_marginal_does_not_set_price(self):
        c = config('buyback', 100, max_price=20)
        r = clear(c, bids_for(c, [(10, 5), (20, 1)]))
        self.assertEqual(r.candidate.price, 10)
        self.assertEqual(r.paid, 50)

    def test_undersubscribed_buyback_uses_last_ask_not_ceiling(self):
        c = config('buyback', 1000, max_price=20)
        r = clear(c, bids_for(c, [(3, 4), (7, 5)]))
        self.assertEqual(r.candidate.price, 7)
        self.assertEqual(r.allocations, (4, 5))

    def test_unaffordable_first_ask_cancels(self):
        c = config('buyback', 1, max_price=2)
        self.assertIsNone(clear(c, bids_for(c, [(2, 1)])).candidate)

    def test_actual_not_nominal_minimum(self):
        c = config(supply=5, min_fill=4)
        r = clear(c, bids_for(c, [(7, 2), (7, 2), (7, 2)]))
        self.assertIsNone(r.candidate)  # three floors of 2*5/6 = 1
        self.assertEqual(r.filled, 0)

    def test_zero_payment_allocations_are_refunded(self):
        c = config('buyback', 1, min_price=1, max_price=1, scale=10, min_bid=10)
        r = clear(c, bids_for(c, [(1, 10), (1, 10), (1, 10)]))
        self.assertIsNone(r.candidate)  # each nominal fill 3 has zero quote value

    def test_sale_ceiling_vs_buyback_floor(self):
        self.assertEqual(mul_div(5, 3, 10, ceiling=True), 2)
        self.assertEqual(mul_div(5, 3, 10), 1)
        self.assertEqual(mul_div(0, 3, 10, ceiling=True), 0)

    def test_256_bit_intermediate_and_i128_result_bound(self):
        self.assertEqual(mul_div(I128_MAX, I128_MAX, I128_MAX), I128_MAX)
        with self.assertRaises(OverflowError):
            mul_div(I128_MAX, 2, 1)
        for a, b, d in [(-1, 2, 3), (1, -2, 3), (1, 2, 0)]:
            with self.assertRaises(ValueError):
                mul_div(a, b, d)

    def test_empty_nonrevealed_and_excluded_never_set_price(self):
        c = config()
        book = [Bid('hidden', 999), Bid('revoked', 999, 10, 99, excluded=True)]
        self.assertIsNone(clear(c, book).candidate)

    def test_grid_and_schedule_validation(self):
        for kwargs in [dict(tick_size=2), dict(max_bids=17), dict(min_price=0),
                       dict(end=20, reveal_end=21), dict(close_deadline=60),
                       dict(min_bid=1, scale=10), dict(max_price=5000)]:
            with self.assertRaises(ValueError):
                config(**kwargs)
        c = config(min_price=2, max_price=10, tick_size=2)
        with self.assertRaises(ValueError):
            c.validate_bid(3, 5)

    def test_chunk_sizes_do_not_change_results(self):
        for mode in ('sale', 'buyback'):
            c = config(mode, 95, max_price=500)
            book = bids_for(c, [(3, 10), (21, 12), (375, 80), (500, 6)])
            expected = clear(c, book, 1)
            for chunk in (2, 7, 16, 32, 1000):
                self.assertEqual(clear(c, book, chunk), expected)
            walker = TickWalker(c, book)
            with self.assertRaises(ValueError):
                walker.step(0)

    def test_dust_cannot_overallocate(self):
        c = config(supply=10)
        r = clear(c, bids_for(c, [(5, 7), (5, 7), (5, 7)]))
        self.assertEqual(r.allocations, (3, 3, 3))
        self.assertEqual(r.filled, 9)

    def test_nominal_marginal_price_survives_individual_rounding(self):
        c = config(supply=31, min_price=5, max_price=9, scale=10, min_bid=10)
        r = clear(c, bids_for(c, [(9, 30), (5, 10), (5, 10)]))
        self.assertEqual(r.candidate.price, 5)
        self.assertEqual(r.allocations, (30, 0, 0))
        c = config('buyback', 11, min_price=2, max_price=3, scale=10, min_bid=10)
        r = clear(c, bids_for(c, [(2, 30), (3, 10), (3, 10), (3, 10)]))
        self.assertEqual(r.candidate.price, 3)
        self.assertEqual(r.allocations, (30, 0, 0, 0))

    def test_maximum_buyback_capacity_must_fit_storage_type(self):
        with self.assertRaises(OverflowError):
            config('buyback', I128_MAX, min_price=1, max_price=1, scale=10, min_bid=10)


class SettlementRegressions(unittest.TestCase):
    def test_issuer_first_withdrawal_cannot_take_sale_refund(self):
        model = closed(config(supply=10), [(9, 6), (7, 8)])
        self.assertEqual(model.credits['issuer', 'quote'], 0)
        with self.assertRaises(ValueError):
            model.withdraw('issuer', 'quote', 1, 'issuer')
        model.resolve(0)
        model.withdraw('issuer', 'quote', model.credits['issuer', 'quote'], 'issuer')
        model.denied.add(('b1', 'rwa'))
        model.resolve(1)
        self.assertEqual(model.credits['b1', 'quote'], 56)
        self.assertEqual(model.credits['issuer', 'rwa'], 4)
        model.denied.clear()
        drain(model)

    def test_unused_buyback_withdrawal_preserves_all_payment_reserves(self):
        model = closed(config('buyback', 100, max_price=20), [(10, 8), (20, 1)])
        model.withdraw('issuer', 'quote', 20, 'issuer')
        self.assertEqual(model.reserve['quote'], 80)
        model.denied.add(('b0', 'rwa'))
        model.resolve(0)
        self.assertEqual(model.credits['b0', 'rwa'], 8)
        self.assertEqual(model.credits['issuer', 'quote'], 80)
        model.resolve(1)
        model.denied.clear()
        drain(model)

    def test_revocation_after_preliminary_sweep_cannot_pass_finalize(self):
        model = funded(config(supply=10), [(9, 6), (7, 8)])
        model.ledger = model.config.reveal_end
        model.prepare(1)
        model.denied.add(('b0', 'rwa'))
        self.assertFalse(model.finalize())
        self.assertTrue(model.bids[0].excluded)
        self.assertIsNone(model.ready)
        model.denied.clear()  # exclusion is still permanent
        model.close(1)
        self.assertEqual(model.result.allocations, (0, 8))

    def test_each_restart_excludes_a_new_bid(self):
        model = funded(config(supply=1), [(10, 1), (9, 1), (8, 1)])
        model.ledger = model.config.reveal_end
        model.denied.update((f'b{i}', 'rwa') for i in range(3))
        model.close(1)
        self.assertEqual(model.exclusion_count, 3)
        self.assertEqual(model.state, 'cancelled')
        for i in range(3): model.resolve(i)
        model.denied.clear()
        drain(model)

    def test_revoked_zero_fill_marginal_demand_cannot_set_price(self):
        for mode in ('sale', 'buyback'):
            model = funded(config(mode, 3 if mode == 'sale' else 6),
                           [(3 if mode == 'sale' else 1, 2), (2, 1), (2, 1)])
            model.ledger = model.config.reveal_end
            model.prepare()
            self.assertEqual(model.ready.allocations, (2, 0, 0))
            model.denied.update({('b1', 'rwa'), ('b2', 'rwa')})
            self.assertFalse(model.finalize())
            model.close()
            self.assertEqual(model.exclusion_count, 2)
            self.assertEqual(model.result.candidate.price, 1)
            self.assertEqual(model.result.allocations, (2, 0, 0))
            for i in range(3):
                model.resolve(i)
            model.denied.clear()
            drain(model)

    def test_zero_payment_revocation_is_checked_before_minimum_cancellation(self):
        model = funded(config('buyback', 1, scale=10, min_price=1, max_price=1, min_bid=10),
                       [(1, 10), (1, 10)])
        model.ledger = model.config.reveal_end
        model.prepare()
        self.assertIsNone(model.ready.candidate)
        model.denied.add(('b1', 'rwa'))
        self.assertFalse(model.finalize())
        model.close()
        self.assertEqual(model.result.allocations, (10, 0))
        self.assertEqual(model.result.payments, (1, 0))
        model.resolve(0)
        model.resolve(1)
        model.denied.clear()
        drain(model)

    def test_frozen_refund_cannot_block_another_bid(self):
        model = closed(config(supply=2), [(10, 2), (2, 2)])
        model.denied.add(('b1', 'quote'))
        model.resolve(1)
        with self.assertRaises(ValueError):
            model.withdraw('b1', 'quote', 4, 'b1')
        self.assertEqual(model.credits['b1', 'quote'], 4)
        model.resolve(0)
        self.assertEqual(model.bids[0].outcome, 'delivered')
        model.denied.clear()
        drain(model)

    def test_failed_delivery_preserves_bid_and_both_buyback_legs(self):
        model = closed(config('buyback', 100, max_price=20), [(10, 4), (20, 4)])
        model.blocked.add(('b0', 'quote'))
        before = copy.deepcopy((model.balance, model.liability, model.credits, model.delivered))
        with self.assertRaises(ValueError): model.resolve(0)
        self.assertEqual((model.balance, model.liability, model.credits, model.delivered), before)
        self.assertFalse(model.bids[0].resolved)
        model.blocked.clear()
        model.resolve(0)
        model.resolve(1)
        drain(model)

    def test_withdrawal_requires_owner_and_cannot_redirect(self):
        model = closed(config(supply=3), [(5, 1)])
        with self.assertRaises(ValueError):
            model.withdraw('issuer', 'rwa', 1, 'attacker')
        model.withdraw('issuer', 'rwa', 1, 'issuer')
        self.assertEqual(model.delivered['issuer', 'rwa'], 1)
        with self.assertRaises(ValueError):
            model.withdraw('issuer', 'rwa', 2, 'issuer')

    def test_claim_order_equivalence_with_revoked_and_zero_fills(self):
        for mode, supply in [('sale', 10), ('buyback', 60)]:
            initial = closed(config(mode, supply), [(9, 6), (7, 8), (2, 5)])
            initial.denied.add(('b1', 'rwa'))
            outcomes = []
            for order in itertools.permutations(range(3)):
                m = copy.deepcopy(initial)
                for i in order: m.resolve(i)
                outcomes.append((dict(m.credits), dict(m.delivered), m.balance, m.liability))
            self.assertTrue(all(item == outcomes[0] for item in outcomes))

    def test_delivery_deadline_only_allows_refunds(self):
        model = closed(config(), [(5, 10)])
        model.ledger = model.config.delivery_deadline
        with self.assertRaises(ValueError): model.resolve(0)
        model.resolve(0, expire=True)
        self.assertEqual(model.bids[0].outcome, 'refund')
        self.assertEqual(model.executed_quantity, 0)
        drain(model)

    def test_expiry_survives_global_freeze_and_does_not_forfeit(self):
        model = closed(config(), [(5, 10)])
        model.ledger = model.config.delivery_deadline
        model.denied.update(('auction', a) for a in ASSETS)
        model.resolve(0, expire=True)
        self.assertEqual(model.credits['b0', 'quote'], 50)
        with self.assertRaises(ValueError): model.withdraw('b0', 'quote', 50, 'b0')
        model.ledger += 10000
        model.denied.clear()
        drain(model)

    def test_closing_deadline_cannot_be_bypassed_by_finalizer(self):
        model = funded(config(), [(5, 10)])
        model.ledger = model.config.reveal_end
        model.prepare()
        model.ledger = model.config.close_deadline
        with self.assertRaises(ValueError): model.finalize()
        model.expire_close()
        model.resolve(0)
        drain(model)

    def test_unfunded_instance_expires_without_entitlements(self):
        model = AuctionModel(config())
        with self.assertRaises(ValueError): model.place('b', 1, 1)
        model.ledger = model.config.start
        with self.assertRaises(ValueError): model.fund()
        model.expire_close()
        self.assertEqual(model.state, 'refunded')
        self.assertEqual(sum(model.liability.values()), 0)

    def test_nonreveal_and_excess_sealed_deposit_full_ownership(self):
        c = config(supply=3, sealed=True, reveal_end=25)
        model = funded(c)
        first = model.commit('b0', 100)
        second = model.commit('b1', 200)
        model.ledger = c.end
        model.reveal(first, 5, 3, 'b0')
        with self.assertRaises(ValueError): model.reveal(first, 5, 3, 'b0')
        self.assertEqual(model.pending['quote'], 300)  # no reveal refund transfer
        model.ledger = c.reveal_end
        model.close()
        model.resolve(first)
        model.resolve(second)
        self.assertEqual(model.credits['b0', 'quote'], 85)
        self.assertEqual(model.credits['b1', 'quote'], 200)
        drain(model)

    def test_buyback_sealed_excess_token_refund(self):
        c = config('buyback', 10, sealed=True, reveal_end=25)
        model = funded(c)
        i = model.commit('b0', 10)
        model.ledger = c.end
        model.reveal(i, 5, 3, 'b0')
        model.ledger = c.reveal_end
        model.close()
        model.resolve(i)
        self.assertEqual(model.credits['b0', 'rwa'], 8)
        self.assertEqual(model.delivered['treasury', 'rwa'], 2)
        drain(model)

    def test_reveal_undercollateralized_or_wrong_owner_preserves_deposit(self):
        for mode in ('sale', 'buyback'):
            c = config(mode, 10, sealed=True, reveal_end=25)
            model = funded(c)
            i = model.commit('b0', 2)
            model.ledger = c.end
            with self.assertRaises(ValueError): model.reveal(i, 5, 3, 'b0')
            with self.assertRaises(ValueError): model.reveal(i, 1, 1, 'attacker')
            self.assertIsNone(model.bids[i].quantity)
            self.assertEqual(model.pending[c.deposit_asset], 2)

    def test_every_bidder_requires_rwa_authorization_even_sale_investor(self):
        model = funded(config())
        model.denied.add(('b0', 'rwa'))
        with self.assertRaises(ValueError): model.place('b0', 5, 2)
        self.assertFalse(model.bids)

    def test_bid_caps_include_nonreveals(self):
        c = config(sealed=True, reveal_end=25, max_bids=2, per_address=1)
        model = funded(c)
        model.commit('b0', 5)
        with self.assertRaises(ValueError): model.commit('b0', 5)
        model.commit('b1', 5)
        with self.assertRaises(ValueError): model.commit('b2', 5)
        self.assertEqual(model.pending['quote'], 10)

    def test_replay_cannot_duplicate_refund_or_issuer_credit(self):
        model = closed(config(supply=10), [(5, 5)])
        model.resolve(0)
        before = copy.deepcopy((model.credits, model.liability, model.balance))
        with self.assertRaises(ValueError): model.resolve(0)
        self.assertEqual((model.credits, model.liability, model.balance), before)
        with self.assertRaises(ValueError): model.finalize()
        drain(model)

    def test_unsolicited_surplus_is_not_issuer_credit(self):
        model = closed(config(), [(5, 10)])
        model.balance['quote'] += 7
        model.resolve(0)
        for (owner, asset), amount in list(model.credits.items()):
            if amount: model.withdraw(owner, asset, amount, owner)
        self.assertEqual(model.balance['quote'], 7)
        self.assertEqual(model.liability['quote'], 0)
        model.assert_invariants()

    def test_clawback_deficit_stops_outgoing_but_not_deadline_refund(self):
        model = closed(config(supply=10), [(5, 5)])
        model.balance['rwa'] -= 1
        with self.assertRaisesRegex(ValueError, 'insolvent'):
            model.withdraw('issuer', 'rwa', 1, 'issuer')
        with self.assertRaisesRegex(ValueError, 'insolvent'): model.resolve(0)
        self.assertFalse(model.bids[0].resolved)
        model.ledger = model.config.delivery_deadline
        model.resolve(0, expire=True)
        model.balance['rwa'] += 1  # external replenishment
        drain(model)

    def test_boundary_placement_and_reveal(self):
        c = config(sealed=True, reveal_end=25)
        model = AuctionModel(c)
        model.fund()
        model.ledger = c.start-1
        with self.assertRaises(ValueError): model.commit('b0', 10)
        model.ledger = c.start
        i = model.commit('b0', 10)
        model.ledger = c.end
        with self.assertRaises(ValueError): model.commit('b1', 10)
        model.reveal(i, 1, 1, 'b0')
        j = AuctionModel(c)
        j.fund(); j.ledger = c.start
        k = j.commit('b1', 10); j.ledger = c.reveal_end
        with self.assertRaises(ValueError): j.reveal(k, 1, 1, 'b1')

    def test_protocol_addresses_cannot_bid_or_receive_buyback(self):
        model = funded(config())
        for owner in ('issuer', 'treasury', 'auction', 'factory', 'registry'):
            with self.assertRaises(ValueError): model.place(owner, 5, 1)
        for recipient in ('auction', 'factory', 'registry'):
            with self.assertRaises(ValueError):
                AuctionModel(config('buyback'), buyback_recipient=recipient)

    def test_aggregate_overflow_preserves_preexisting_deposits(self):
        c = config(supply=1, min_price=1, max_price=1)
        model = funded(c, [(1, I128_MAX)])
        before = copy.deepcopy((model.balance, model.liability, model.pending))
        with self.assertRaises(OverflowError): model.place('b1', 1, 1)
        self.assertEqual((model.balance, model.liability, model.pending), before)
        self.assertEqual(len(model.bids), 1)

    def test_funding_requires_issuer_and_is_exactly_once(self):
        model = AuctionModel(config())
        with self.assertRaises(ValueError): model.fund('attacker')
        model.fund()
        with self.assertRaises(ValueError): model.fund()
        self.assertEqual(model.balance['rwa'], 10)
        self.assertEqual(model.liability['rwa'], 10)


class GeneratedVerification(unittest.TestCase):
    def test_extreme_i128_books_against_bigint_reference(self):
        m, s = I128_MAX, 10_000_000
        cases = [
            (config(supply=m, min_price=s, max_price=s, scale=s), [(s, m//2), (s, m-m//2)]),
            (config(supply=m-1, min_price=s, max_price=s, scale=s), [(s, m//2), (s, m-m//2)]),
            (config(supply=1, min_price=m, max_price=m, scale=s), [(m, 1)]),
            (config('buyback', m, min_price=m, max_price=m, scale=s), [(m, 2*s)]),
            (config('buyback', m, min_price=s, max_price=2*s, tick_size=s, scale=s), [(s, m//4), (2*s, m//2)]),
            (config('buyback', m, min_price=m, max_price=m, scale=1), [(m, 1)]),
        ]
        payloads, expected = [], []
        for c, entries in cases:
            book = bids_for(c, entries)
            r = clear(c, book)
            payloads.append(json.dumps(dict(config=dict(mode=c.mode, supply=str(c.supply),
                scale=str(c.scale), min_price=str(c.min_price), min_fill=str(c.min_fill)),
                bids=[dict(price=str(b.price), quantity=str(b.quantity), excluded=False) for b in book])))
            expected.append(dict(price=None if r.candidate is None else str(r.candidate.price),
                allocations=list(map(str, r.allocations)), payments=list(map(str, r.payments))))
        completed = subprocess.run(['node', str(ROOT/'tools/design/reference.mjs')],
            input='\n'.join(payloads)+'\n', text=True, capture_output=True, check=True, timeout=10)
        self.assertEqual([json.loads(line) for line in completed.stdout.splitlines()], expected)

    def test_10000_books_against_independent_bigint_reference(self):
        rng = random.Random(0xC20551)
        inputs, expected = [], []
        for case in range(10_000):
            scale = rng.choice((1, 10, 10_000_000))
            low, tick = rng.randint(1, 30), rng.randint(1, 5)
            high = low+tick*rng.randint(0, 12)
            min_bid = max(1, (scale+low-1)//low)
            mode = 'sale' if case % 2 == 0 else 'buyback'
            supply = rng.randint(1, 200)*min_bid if mode == 'sale' else max(rng.randint(1, 1000), (low+scale-1)//scale)
            c = config(mode, supply, min_price=low, max_price=high,
                       tick_size=tick, scale=scale, min_bid=min_bid)
            book = []
            for i in range(rng.randint(0, 16)):
                q = rng.randint(1, 30)*min_bid
                p = low+tick*rng.randrange(c.tick_count)
                b = bids_for(c, [(p, q)])[0]
                b.owner = f'b{i}'
                if rng.random() < .08: b.quantity, b.price = None, None
                b.excluded = rng.random() < .12
                book.append(b)
            r = clear(c, book, rng.randint(1, 32))
            self.assertLessEqual(r.filled, supply if mode == 'sale' else I128_MAX)
            if mode == 'buyback': self.assertLessEqual(r.paid, supply)
            for b, f, payment in zip(book, r.allocations, r.payments):
                if f:
                    self.assertLessEqual(f, b.quantity)
                    self.assertTrue(payment > 0)
                    if mode == 'sale':
                        self.assertLessEqual(payment, b.deposit)
                        self.assertLessEqual(r.candidate.price, b.price)
                    else: self.assertGreaterEqual(r.candidate.price, b.price)
            inputs.append(json.dumps(dict(config=dict(mode=mode, supply=str(supply),
                scale=str(scale), min_price=str(low), min_fill=str(c.min_fill)),
                bids=[dict(price=None if b.price is None else str(b.price),
                           quantity=None if b.quantity is None else str(b.quantity),
                           excluded=b.excluded) for b in book])))
            expected.append(dict(price=None if r.candidate is None else str(r.candidate.price),
                                 allocations=list(map(str, r.allocations)), payments=list(map(str, r.payments))))
        completed = subprocess.run(['node', str(ROOT/'tools/design/reference.mjs')],
                                   input='\n'.join(inputs)+'\n', text=True,
                                   capture_output=True, check=True, timeout=90)
        actual = [json.loads(line) for line in completed.stdout.splitlines()]
        self.assertEqual(len(actual), 10_000)
        for index, (got, want) in enumerate(zip(actual, expected)):
            self.assertEqual(got, want, f'seed 0xC20551 case {index}: {inputs[index]}')

    def test_1000_generated_lifecycles_conserve_both_assets(self):
        rng = random.Random(0x51E771E)
        for case in range(1000):
            mode = 'sale' if case % 2 == 0 else 'buyback'
            c = config(mode, rng.randint(1, 500))
            entries = [(rng.randint(1, 10), rng.randint(1, 30)) for _ in range(rng.randint(1, 12))]
            m = funded(c, entries)
            m.ledger = c.reveal_end
            for i in range(len(m.bids)):
                if rng.random() < .15: m.denied.add((f'b{i}', 'rwa'))
            m.close(rng.randint(1, 32))
            for i in range(len(m.bids)):
                if rng.random() < .2: m.denied.add((f'b{i}', rng.choice(ASSETS)))
            order = list(range(len(m.bids))); rng.shuffle(order)
            # Issuer can take only presently credited amounts before bidders.
            for a in ASSETS:
                n = m.credits['issuer', a]
                if n: m.withdraw('issuer', a, n, 'issuer')
            for i in order:
                if rng.random() < .1: m.ledger = c.delivery_deadline
                m.resolve(i, expire=m.ledger >= c.delivery_deadline)
                m.assert_invariants()
            m.denied.clear()
            drain(m)
            self.assertLessEqual(m.exclusion_count, len(m.bids))


class PenaltyRegressions(unittest.TestCase):
    def test_penalty_configuration_is_bounded_and_sealed_only(self):
        for rate in [-1, 5_001]:
            with self.assertRaises(ValueError):
                config(sealed=True, reveal_end=25, nonreveal_penalty_bps=rate)
        with self.assertRaises(ValueError):
            config(nonreveal_penalty_bps=1)

    def test_refund_reason_precedence_and_conservation_in_both_modes(self):
        for mode, terminal, revoked, reverse in itertools.product(
                ['sale', 'buyback'], ['cleared', 'cancelled', 'expired', 'cancelled_expired'], [False, True], [False, True]):
            with self.subTest(mode=mode, terminal=terminal, revoked=revoked, reverse=reverse):
                c = config(mode, 10, max_price=1, sealed=True, reveal_end=25,
                           min_fill=6 if terminal.startswith('cancelled') else 1,
                           nonreveal_penalty_bps=2_500)
                m = funded(c)
                good = m.commit('good', 5)
                absent = m.commit('absent', 100)
                m.ledger = c.end
                m.reveal(good, 1, 5, 'good')
                m.ledger = c.reveal_end
                m.close()
                if revoked:
                    m.denied.add(('absent', c.deposit_asset))
                expired = terminal.endswith('expired')
                if expired:
                    m.ledger = c.delivery_deadline
                for bid in ([absent, good] if reverse else [good, absent]):
                    m.resolve(bid, expire=expired)
                # Cleared non-reveals retain the penalty after delivery expiry;
                # genuine auction cancellation takes precedence at either time.
                penalty = 0 if terminal.startswith('cancelled') else 25
                self.assertEqual(m.credits['absent', c.deposit_asset], 100-penalty)
                self.assertEqual(m.credits['issuer', c.deposit_asset], penalty + (5 if mode == 'sale' and terminal == 'cleared' else 0))
                expected_reason = 'cancelled' if terminal.startswith('cancelled') else 'nonreveal'
                self.assertEqual(m.bids[absent].reason, expected_reason)
                m.denied.clear()
                drain(m)

    def test_penalties_floor_at_zero_maximum_and_wide_values_and_survive_failed_withdrawal(self):
        for mode, deposit, rate in itertools.product(
                ['sale', 'buyback'], [1, 999, 1_000, I128_MAX-1], [0, 2_500, 5_000]):
            with self.subTest(mode=mode, deposit=deposit, rate=rate):
                c = config(mode, 1, min_price=10_000_000, max_price=10_000_000,
                           scale=10_000_000, sealed=True, reveal_end=25,
                           nonreveal_penalty_bps=rate)
                m = funded(c)
                good = m.commit('good', 1)
                absent = m.commit('absent', deposit)
                m.ledger = c.end
                m.reveal(good, c.min_price, 1, 'good')
                m.ledger = c.reveal_end
                m.close()
                m.resolve(absent)
                fee = deposit*rate//10_000
                self.assertEqual(m.credits['absent', c.deposit_asset], deposit-fee)
                self.assertEqual(m.credits['issuer', c.deposit_asset], fee)
                self.assertLessEqual(fee, deposit//2)
                for owner in ['absent', 'issuer']:
                    amount = m.credits[owner, c.deposit_asset]
                    if not amount:
                        continue
                    m.blocked.add((owner, c.deposit_asset))
                    before = copy.deepcopy((m.credits, m.balance, m.liability))
                    with self.assertRaises(ValueError):
                        m.withdraw(owner, c.deposit_asset, amount, owner)
                    self.assertEqual((m.credits, m.balance, m.liability), before)
                    m.blocked.clear()
                    m.withdraw(owner, c.deposit_asset, amount, owner)
                m.resolve(good)
                drain(m)

    def test_sealed_sale_admission_guarantees_reveal_capacity_in_either_order(self):
        for reverse in [False, True]:
            c = config(supply=1, min_price=1, max_price=1, scale=10_000_000,
                       min_bid=10_000_000, sealed=True, reveal_end=25,
                       nonreveal_penalty_bps=2_500)
            m = funded(c)
            first = m.commit('first', 100)
            before = copy.deepcopy((m.bids, m.balance, m.liability, m.pending))
            with self.assertRaises(OverflowError):
                m.commit('second', I128_MAX//c.scale)
            self.assertEqual((m.bids, m.balance, m.liability, m.pending), before)
            accepted = I128_MAX//c.scale-100
            second = m.commit('second', accepted)
            m.ledger = c.end
            for bid_id in ([second, first] if reverse else [first, second]):
                m.reveal(bid_id, 1, m.bids[bid_id].deposit*c.scale, m.bids[bid_id].owner)
            self.assertEqual(sum(b.quantity for b in m.bids), (I128_MAX//c.scale)*c.scale)


if __name__ == '__main__':
    unittest.main()
