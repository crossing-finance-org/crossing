#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$repo_dir"
cargo fmt --all -- --check
python3 scripts/wasm_artifacts.py build
export CROSSING_WASM_DIR
CROSSING_WASM_DIR="$(python3 scripts/wasm_artifacts.py directory)"
CROSSING_TEST_MODE= cargo test --workspace --locked -- --test-threads=1
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -m unittest discover -s tests/design -v
python3 -m unittest discover -s tests/tooling -v
node tools/design/check_commitment.mjs
python3 tools/design/generate_contract_vectors.py --check
python3 scripts/check_docs.py
