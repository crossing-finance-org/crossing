#!/usr/bin/env python3
"""Build, verify, and package the exact WASMs consumed by integration tests."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess


ROOT = Path(__file__).resolve().parents[1]
CONTRACTS = ("crossing_auction", "crossing_auction_factory",
             "crossing_price_registry", "crossing_oracle_adapter")
MANIFEST = "test-wasm-manifest.json"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_inputs():
    paths = [ROOT / name for name in
             ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml")]
    paths.extend(sorted((ROOT / "contracts").glob("*/Cargo.toml")))
    paths.extend(sorted(path for path in (ROOT / "contracts").glob("*/src/**/*.rs")
                        if path != ROOT / "contracts/common/src/test.rs"))
    return {str(path.relative_to(ROOT)): sha256(path) for path in paths}


def target_directory():
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
        cwd=ROOT, text=True, capture_output=True, check=True)
    return Path(json.loads(result.stdout)["target_directory"])


def wasm_directory():
    return target_directory() / "wasm32v1-none/release"


def build():
    directory = wasm_directory()
    manifest_path = directory / MANIFEST
    # A failed build must not leave a previous success marker usable by tests.
    manifest_path.unlink(missing_ok=True)
    inputs = source_inputs()
    subprocess.run(["cargo", "wasm"], cwd=ROOT, check=True)
    if inputs != source_inputs():
        raise ValueError("Contract build inputs changed during the WASM build; retry.")
    artifacts = {}
    for name in CONTRACTS:
        path = directory / f"{name}.wasm"
        if not path.read_bytes().startswith(b"\x00asm\x01\x00\x00\x00"):
            raise ValueError(f"Not a WASM module: {path}")
        artifacts[path.name] = {"sha256": sha256(path), "bytes": path.stat().st_size}
    revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT,
                              text=True, capture_output=True, check=True).stdout.strip()
    status = subprocess.run(["git", "status", "--porcelain"], cwd=ROOT,
                            text=True, capture_output=True, check=True).stdout
    value = dict(version=1, source_revision=revision, working_tree_dirty=bool(status),
                 rustc=subprocess.check_output(["rustc", "--version"], text=True).strip(),
                 cargo=subprocess.check_output(["cargo", "--version"], text=True).strip(),
                 target="wasm32v1-none", profile="release", command="cargo wasm",
                 source_inputs_sha256=inputs, artifacts=artifacts)
    manifest_path.write_text(json.dumps(value, indent=2) + "\n")
    print(f"Built and recorded four test WASMs: {directory}")


def verify(directory):
    value = json.loads((directory / MANIFEST).read_text())
    if value.get("version") != 1 or value.get("source_inputs_sha256") != source_inputs():
        raise ValueError("Stale WASM build inputs; run python3 scripts/wasm_artifacts.py build.")
    if set(value["artifacts"]) != {f"{name}.wasm" for name in CONTRACTS}:
        raise ValueError("Expected exactly four contract WASMs in the build manifest.")
    for name, record in value["artifacts"].items():
        path = directory / name
        if sha256(path) != record["sha256"] or path.stat().st_size != record["bytes"]:
            raise ValueError(f"WASM identity mismatch: {path}")
    return value


def package():
    directory = wasm_directory()
    value = verify(directory)
    destination = target_directory() / "verified-contracts"
    destination.mkdir(parents=True, exist_ok=True)
    # Copy only verified artifacts; never glob unrelated build outputs.
    for name in (*value["artifacts"], MANIFEST):
        shutil.copyfile(directory / name, destination / name)
    for name in ("Cargo.lock", "Cargo.toml", "rust-toolchain.toml"):
        shutil.copyfile(ROOT / name, destination / name)
    (destination / "SHA256SUMS").write_text("".join(
        f"{record['sha256']}  {name}\n" for name, record in value["artifacts"].items()))
    print(f"Packaged four verified test WASMs and build metadata: {destination}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "directory", "verify", "package"))
    args = parser.parse_args()
    if args.command == "build":
        build()
    elif args.command == "directory":
        print(wasm_directory())
    elif args.command == "package":
        package()
    else:
        verify(Path(os.environ["CROSSING_WASM_DIR"]) if "CROSSING_WASM_DIR" in os.environ
               else wasm_directory())
        print("All four WASM identities and current contract build inputs match.")


if __name__ == "__main__":
    main()
