"""Adversarial coverage reports must fail without weakening production gates.

Small synthetic LLVM exports isolate report-policy failures; these are not
measurements of contract coverage. Actual LLVM reports remain a separate run.
"""
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SOURCES = sorted(path.resolve() for path in (ROOT / "contracts").glob("*/src/**/*.rs")
                 if path != ROOT / "contracts/common/src/test.rs")


def report():
    return {
        "type": "llvm.coverage.json.export", "version": "3.0.1",
        "data": [{"files": [
            {"filename": str(path), "summary": {"lines": {"count": 20, "covered": 20}}}
            for path in SOURCES
        ], "functions": [
            {"name": f"production_function_{index}", "filenames": [str(path)],
             "regions": [[1, 1, 1, 2, 1, 0, 0, 0]]}
            for index, path in enumerate(SOURCES)
        ]}],
    }


def source_record(value, crate):
    return next(file for file in value["data"][0]["files"]
                if file["filename"].endswith(f"/contracts/{crate}/src/lib.rs"))


def helper_regions(value, crate):
    path = ROOT / "contracts" / crate / "src/lib.rs"
    attributes = [line for line, text in enumerate(path.read_text().splitlines(), 1)
                  if text.strip() == "#[contracttype]"][:2]
    assert len(attributes) == 2
    functions = [{
        "name": f"stellar_xdr_ScVal_convert7TryFromR{index}",
        "filenames": [str(path)], "regions": [[line, 1, line, 2, 0, 0, 0, 0]],
    } for index, line in enumerate(attributes)]
    value["data"][0]["functions"].extend(functions)
    source_record(value, crate)["summary"]["lines"]["covered"] = 18
    return attributes


class CoverageGateTests(unittest.TestCase):
    def run_gate(self, value, diagnostics=None):
        with tempfile.TemporaryDirectory(prefix="crossing-coverage-policy-") as directory:
            path = Path(directory) / "coverage.json"
            path.write_text(json.dumps(value))
            command = ["python3", str(ROOT / "scripts/check_coverage.py"), str(path)]
            if diagnostics is not None:
                diagnostic = Path(directory) / "llvm.stderr"
                diagnostic.write_text(diagnostics)
                command.extend(["--diagnostics", str(diagnostic)])
            result = subprocess.run(command, text=True, capture_output=True, cwd=ROOT)
            summary_path = path.with_name("production-summary.json")
            summary = json.loads(summary_path.read_text()) if summary_path.exists() else None
            return result, summary

    def test_complete_valid_report_retains_all_four_raw_contract_gates(self):
        result, summary = self.run_gate(report(), "Finished report saved to coverage.json\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(summary["minimum_percent"], 95)
        self.assertEqual(len(summary["raw_deployable_contract_totals"]), 4)
        self.assertEqual(summary["failures"], [])

    def test_missing_or_empty_contract_cannot_be_reported_as_fully_covered(self):
        for crate in ("auction", "auction-factory", "price-registry", "oracle-adapter", "common"):
            for missing in (False, True):
                with self.subTest(crate=crate, missing=missing):
                    value = report()
                    record = source_record(value, crate)
                    if missing:
                        value["data"][0]["files"].remove(record)
                    else:
                        record["summary"]["lines"] = {"count": 0, "covered": 0}
                    result, summary = self.run_gate(value)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertTrue(summary["failures"])

    def test_duplicate_last_record_cannot_hide_undercovered_source(self):
        for reverse in (False, True):
            value = report()
            original = source_record(value, "auction")
            duplicate = copy.deepcopy(original)
            original["summary"]["lines"]["covered"] = 1
            if reverse:
                value["data"][0]["files"].insert(0, duplicate)
            else:
                value["data"][0]["files"].append(duplicate)
            result, summary = self.run_gate(value)
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue(any("Duplicate coverage" in error for error in summary["failures"]))

    def test_positive_totals_cannot_replace_missing_production_function_maps(self):
        value = report()
        value["data"][0]["functions"] = []
        result, summary = self.run_gate(value)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(any("Missing production function" in error for error in summary["failures"]))

    def test_invalid_counter_types_ranges_and_source_denominators_fail(self):
        for counters in ({"count": True, "covered": True}, {"count": 20, "covered": -1},
                         {"count": 20, "covered": 21}, {"count": -1, "covered": -1},
                         {"count": 20.0, "covered": 20}, {"count": 100_000, "covered": 100_000}):
            with self.subTest(counters=counters):
                value = report()
                source_record(value, "auction")["summary"]["lines"] = counters
                result, _ = self.run_gate(value)
                self.assertNotEqual(result.returncode, 0)

    def test_exact_shared_sdk_bridges_are_disclosed_but_raw_contract_gate_remains(self):
        value = report()
        helper_regions(value, "common")
        result, summary = self.run_gate(value)
        self.assertEqual(result.returncode, 0, result.stderr)
        shared = next(file for file in summary["files"] if "/common/" in file["file"])
        self.assertEqual(shared["raw_percent"], 90)
        self.assertEqual(shared["production_percent"], 100)
        self.assertEqual(len(shared["sdk_test_only_exclusions"]), 2)
        value = report()
        helper_regions(value, "auction")
        result, summary = self.run_gate(value)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(any("raw LLVM" in error for error in summary["failures"]))

    def test_handwritten_or_overlapping_production_regions_cannot_be_excluded(self):
        for overlap in (False, True):
            value = report()
            attributes = helper_regions(value, "common")
            functions = value["data"][0]["functions"]
            if overlap:
                functions.append({"name": "handwritten_production_function",
                                  "filenames": functions[-2]["filenames"],
                                  "regions": [[attributes[0], 1, attributes[0], 2, 0, 0, 0, 0]]})
            else:
                # A helper-looking symbol on an ordinary source line is not enough.
                functions[-2]["regions"] = [[1, 1, 1, 2, 0, 0, 0, 0]]
            result, _ = self.run_gate(value)
            self.assertNotEqual(result.returncode, 0)

    def test_outdated_profile_diagnostics_and_invalid_regions_fail(self):
        for diagnostic in ("warning: 19 functions have mismatched data\n",
                           "warning: function hash mismatch\n", "profile is out of date\n"):
            result, summary = self.run_gate(report(), diagnostic)
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue(summary["failures"])
        value = report()
        helper_regions(value, "common")
        value["data"][0]["functions"][-2]["regions"][0][0] = 0
        result, _ = self.run_gate(value)
        self.assertNotEqual(result.returncode, 0)

    def test_failed_reuse_run_cannot_leave_prior_merged_data_or_passing_reports(self):
        # Isolated fake Cargo only exercises failure cleanup, never contract
        # correctness or the actual coverage measurement.
        with tempfile.TemporaryDirectory(prefix="crossing-coverage-runner-") as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            script = root / "scripts/coverage.sh"
            script.write_text((ROOT / "scripts/coverage.sh").read_text())
            # This test isolates LLVM cleanup; artifact verification has its own
            # adversarial tests and must not invoke a real build in this fixture.
            (root / "scripts/wasm_artifacts.py").write_text(
                "import sys\nfrom pathlib import Path\n"
                "if sys.argv[1] == 'directory':\n"
                "    print(Path(__file__).resolve().parents[1] / 'target/wasm32v1-none/release')\n")
            profiles = root / "target/llvm-cov-target"
            profiles.mkdir(parents=True)
            merged = profiles / "Crossing.profdata"
            merged.write_text("old merged profile")
            reports = root / "target/coverage"
            (reports / "html").mkdir(parents=True)
            old_reports = [reports / "coverage.json", reports / "production-summary.json",
                           reports / "html/index.html"]
            for old in old_reports:
                old.write_text("old passing report")
            binaries = root / "bin"
            binaries.mkdir()
            cargo = binaries / "cargo"
            cargo.write_text("#!/usr/bin/env python3\nimport sys\n"
                             "if sys.argv[1:3] == ['llvm-cov', 'report']:\n"
                             "    sys.stderr.write('no fresh raw profiles\\n')\n"
                             "    sys.exit(1)\n")
            cargo.chmod(0o755)
            environment = dict(os.environ, PATH=str(binaries) + os.pathsep + os.environ["PATH"])
            result = subprocess.run(["bash", str(script), "--reuse-build"], cwd=root,
                                    env=environment, text=True, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("no fresh raw profiles", result.stderr)
            self.assertFalse(merged.exists())
            for old in old_reports:
                self.assertFalse(old.exists(), str(old))


if __name__ == "__main__":
    unittest.main()
