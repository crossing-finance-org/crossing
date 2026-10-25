#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$repo_dir"
python3 scripts/wasm_artifacts.py build
export CROSSING_WASM_DIR
CROSSING_WASM_DIR="$(python3 scripts/wasm_artifacts.py directory)"
CROSSING_TEST_MODE= cargo test -p crossing-auction --test lifecycle --locked poc_demo_ -- --nocapture --test-threads=1
