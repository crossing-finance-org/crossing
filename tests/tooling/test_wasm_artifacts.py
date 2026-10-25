"""Reject stale build inputs and altered artifacts before executing contract tests."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("wasm_artifacts", ROOT / "scripts/wasm_artifacts.py")
ARTIFACTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ARTIFACTS)


class WasmArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="crossing-artifacts-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.manifest = dict(version=1, source_inputs_sha256=ARTIFACTS.source_inputs(), artifacts={})
        for name in ARTIFACTS.CONTRACTS:
            path = self.directory / f"{name}.wasm"
            path.write_bytes(b"\0asm\x01\0\0\0")
            self.manifest["artifacts"][path.name] = dict(sha256=ARTIFACTS.sha256(path), bytes=8)
        self.write_manifest()

    def write_manifest(self):
        (self.directory / ARTIFACTS.MANIFEST).write_text(json.dumps(self.manifest))

    def test_custom_target_directory_with_spaces_uses_cargo_configuration(self):
        target = self.directory / "custom target"
        with patch.dict(os.environ, CARGO_TARGET_DIR=str(target)):
            self.assertEqual(ARTIFACTS.wasm_directory(), target / "wasm32v1-none/release")

    def test_changed_source_or_lockfile_rejects_previous_artifacts(self):
        for source in ("contracts/auction/src/lib.rs", "Cargo.lock"):
            with self.subTest(source=source):
                stale = dict(self.manifest["source_inputs_sha256"])
                stale[source] = "0" * 64
                with patch.object(ARTIFACTS, "source_inputs", return_value=stale):
                    with self.assertRaisesRegex(ValueError, "Stale WASM"):
                        ARTIFACTS.verify(self.directory)

    def test_replaced_or_truncated_wasm_cannot_use_a_previous_manifest(self):
        path = self.directory / "crossing_auction.wasm"
        for content in (b"\0asm\x02\0\0\0", b"\0asm"):
            with self.subTest(content=content):
                path.write_bytes(content)
                with self.assertRaisesRegex(ValueError, "identity mismatch"):
                    ARTIFACTS.verify(self.directory)

    def test_missing_artifact_or_manifest_and_incomplete_sets_fail(self):
        path = self.directory / "crossing_auction.wasm"
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            ARTIFACTS.verify(self.directory)
        del self.manifest["artifacts"][path.name]
        self.write_manifest()
        with self.assertRaisesRegex(ValueError, "exactly four"):
            ARTIFACTS.verify(self.directory)
        (self.directory / ARTIFACTS.MANIFEST).unlink()
        with self.assertRaises(FileNotFoundError):
            ARTIFACTS.verify(self.directory)

    def test_failed_build_invalidates_previous_success_manifest(self):
        with patch.object(ARTIFACTS, "wasm_directory", return_value=self.directory), \
                patch.object(ARTIFACTS.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "cargo")):
            with self.assertRaises(subprocess.CalledProcessError):
                ARTIFACTS.build()
        self.assertFalse((self.directory / ARTIFACTS.MANIFEST).exists())

    def test_package_preserves_verified_bytes_checksums_and_build_metadata(self):
        with patch.object(ARTIFACTS, "wasm_directory", return_value=self.directory), \
                patch.object(ARTIFACTS, "target_directory", return_value=self.directory):
            ARTIFACTS.package()
        package = self.directory / "verified-contracts"
        for name in self.manifest["artifacts"]:
            self.assertEqual((package / name).read_bytes(), (self.directory / name).read_bytes())
        self.assertEqual(json.loads((package / ARTIFACTS.MANIFEST).read_text()), self.manifest)
        self.assertEqual((package / "Cargo.lock").read_bytes(), (ROOT / "Cargo.lock").read_bytes())
        self.assertEqual(len((package / "SHA256SUMS").read_text().splitlines()), 4)


if __name__ == "__main__":
    unittest.main()
