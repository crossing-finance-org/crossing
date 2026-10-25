#!/usr/bin/env python3
"""Gate production line coverage, retaining raw LLVM totals and exact exclusions.

SDK 25's testutils-only TryFrom<&Record> for ScVal wrappers can introduce zero
regions at #[contracttype] attributes. They do not exist in guest artifacts.
Only those exact generated, zero-counter attribute regions may be omitted.
Handwritten functions, error branches, and production converters stay counted.
"""
import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("report", type=Path)
parser.add_argument("--diagnostics", nargs="+", type=Path, default=[])
args = parser.parse_args()
report = json.loads(args.report.read_text())
if report.get("type") != "llvm.coverage.json.export" or not report.get("data"):
    raise SystemExit("Expected a nonempty LLVM coverage export.")
failed = []
for diagnostic in args.diagnostics:
    if re.search(r"mismatched data|hash mismatch|out[- ]of[- ]date|malformed.*profile",
                 diagnostic.read_text(), re.IGNORECASE):
        failed.append(f"Invalid LLVM profile/source diagnostics: {diagnostic}")
unit_files = {(ROOT / "contracts/common/src/test.rs").resolve()}
expected = {p.resolve() for p in (ROOT / "contracts").glob("*/src/**/*.rs")
            if p.resolve() not in unit_files}
sources = {p: p.read_text().splitlines() for p in expected}
observed = {}
native_helpers = {}
production_regions = set()
sdk_pin = 'soroban-sdk = "=25.3.2"'
if sdk_pin not in (ROOT / "Cargo.toml").read_text():
    raise SystemExit("Review the SDK test-helper coverage policy after changing the SDK pin.")
for item in report["data"]:
    for file in item["files"]:
        path = Path(file["filename"]).resolve()
        if path in expected:
            if path in observed:
                failed.append(f"Duplicate coverage record: {path.relative_to(ROOT)}")
                continue
            observed[path] = file["summary"]["lines"]
    for function in item["functions"]:
        # This is the SDK testutils ScVal bridge, not the guest IntoVal path.
        helper = bool(re.search(r"stellar_xdr.*ScVal.*convert\d+TryFromR", function["name"]))
        for region in function["regions"]:
            if region[7] != 0:  # LLVM code regions, not expansions/gaps.
                continue
            path = Path(function["filenames"][region[5]]).resolve()
            if path not in expected:
                continue
            if not 1 <= region[0] <= region[2] <= len(sources[path]):
                failed.append(f"Invalid source region: {path.relative_to(ROOT)}")
                continue
            for line in range(region[0], region[2] + 1):
                key = (path, line)
                if helper and region[4] == 0 and region[0] == region[2]:
                    native_helpers.setdefault(key, set()).add(function["name"])
                else:
                    production_regions.add(key)
summary = []
contract_totals = {}
for path in sorted(expected):
    if path not in observed:
        failed.append(f"Missing coverage: {path.relative_to(ROOT)}")
        continue
    if not any(file == path for file, _ in production_regions):
        failed.append(f"Missing production function mappings: {path.relative_to(ROOT)}")
    lines = observed[path]
    count, covered = lines["count"], lines["covered"]
    source = sources[path]
    if (type(count) is not int or type(covered) is not int
            or not 0 < count <= len(source) or not 0 <= covered <= count):
        failed.append(f"Invalid or empty coverage counters: {path.relative_to(ROOT)}")
        continue
    exclusions = [dict(line=line, symbols=sorted(symbols))
                  for (file, line), symbols in sorted(native_helpers.items())
                  if file == path and (file, line) not in production_regions
                  and source[line - 1].strip() == "#[contracttype]"]
    production_count = count - len(exclusions)
    if not 0 < production_count or not 0 <= covered <= production_count:
        failed.append(f"Inconsistent coverage counters: {path.relative_to(ROOT)}")
        continue
    raw_percent = covered / count * 100
    percent = covered / production_count * 100
    print(f"{path.relative_to(ROOT)}: {covered}/{production_count} production lines ({percent:.2f}%); raw LLVM {covered}/{count} ({raw_percent:.2f}%); {len(exclusions)} SDK test-only attribute regions")
    summary.append(dict(file=str(path.relative_to(ROOT)), raw_lines=count,
                        covered=covered, raw_percent=raw_percent,
                        production_lines=production_count, production_percent=percent,
                        sdk_test_only_exclusions=exclusions))
    crate = path.relative_to(ROOT / "contracts").parts[0]
    manifest = ROOT / "contracts" / crate / "Cargo.toml"
    if '"cdylib"' in manifest.read_text():
        totals = contract_totals.setdefault(crate, [0, 0])
        totals[0] += covered
        totals[1] += count
    if covered * 100 < production_count * 95:
        failed.append(f"Below 95%: {path.relative_to(ROOT)}")
for crate, (covered, count) in sorted(contract_totals.items()):
    if covered * 100 < count * 95:
        failed.append(f"Below 95% raw LLVM coverage in deployable contract: {crate}")
args.report.with_name("production-summary.json").write_text(
    json.dumps(dict(metric="LLVM executable source lines", minimum_percent=95,
                    raw_deployable_contract_totals=contract_totals,
                    files=summary, failures=failed), indent=2) + "\n")
if not expected or failed:
    raise SystemExit("\n".join(failed) or "No production sources found")
print("Every production contract source meets the 95% line-coverage threshold; raw totals are retained.")
