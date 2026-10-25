#!/usr/bin/env python3
"""Deterministic contract inputs with expected values from the BigInt reference.

Run with --check in verification; omit it to regenerate the reviewed fixture.
Does not import the Python auction model or derive expected clearing results.
"""
import argparse
import json
import random
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / "tests/vectors/clearing-v1.json"
SCALE = 10_000_000
SEED = 0xA0D17


def build():
    rng = random.Random(SEED)
    cases = []
    for index in range(48):
        mode = "sale" if index % 2 == 0 else "buyback"
        low = rng.choice((SCALE // 2, SCALE, 2 * SCALE))
        tick = rng.choice((SCALE // 2, SCALE))
        # Multi-call walks, one-price books, and a full 16-bid book.
        count = (1, 20, 65)[index % 3]
        minimum_bid = max(1, (SCALE + low - 1) // low)
        supply = rng.randint(1, 180)
        if mode == "buyback":
            supply = max(supply, (low + SCALE - 1) // SCALE)
        capacity = supply if mode == "sale" else supply * SCALE // low
        minimum_fill = rng.randint(1, max(1, min(capacity, 30)))
        bids = [dict(price=str(low + tick * rng.randrange(count)),
                     quantity=str(rng.randint(minimum_bid, 35)), excluded=False)
                for _ in range(16 if index == 0 else rng.randrange(9))]
        config = dict(mode=mode, supply=str(supply), scale=str(SCALE),
                      min_price=str(low), min_fill=str(minimum_fill))
        cases.append(dict(name=f"seed-{SEED:x}-case-{index}", config=config,
                          max_price=str(low + tick * (count - 1)),
                          tick_size=str(tick), min_bid=str(minimum_bid), bids=bids))
    payload = "".join(json.dumps(case) + "\n" for case in cases)
    completed = subprocess.run(["node", str(ROOT / "tools/design/reference.mjs")],
                               input=payload, text=True, capture_output=True,
                               check=True, timeout=30)
    expected = [json.loads(line) for line in completed.stdout.splitlines()]
    if len(expected) != len(cases):
        raise ValueError("reference result count does not match inputs")
    for case, result in zip(cases, expected):
        case["expected"] = result
    return json.dumps(dict(version=1, seed=SEED, cases=cases), indent=2) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    content = build()
    if args.check:
        if TARGET.read_text() != content:
            raise SystemExit("Contract reference vectors differ; review before regenerating.")
        print("48 contract clearing vectors match the independent BigInt reference.")
    else:
        TARGET.write_text(content)
