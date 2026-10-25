#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$repo_dir"
export CARGO_LLVM_COV_TARGET_DIR="$repo_dir/target/llvm-cov-target"
case "${1:-}" in
  ""|--reuse-build) ;;
  *) printf 'Usage: %s [--reuse-build]\n' "$0" >&2; exit 2 ;;
esac
mkdir -p target/coverage
# A failed run must not leave an older passing summary or HTML behind.
rm -f target/coverage/coverage.json target/coverage/production-summary.json \
  target/coverage/export.stderr target/coverage/html.stderr
rm -rf target/coverage/html
python3 scripts/wasm_artifacts.py build
export CROSSING_WASM_DIR
CROSSING_WASM_DIR="$(python3 scripts/wasm_artifacts.py directory)"
case "${1:-}" in
  "") cargo llvm-cov clean --workspace ;;
  --reuse-build)
    cargo llvm-cov clean --profraw-only
    # The pinned reporter can reuse merged data when no fresh raw profiles exist.
    rm -f "$CARGO_LLVM_COV_TARGET_DIR/"*.profdata
    ;;
esac
# Native service contracts deploy actual WASM auctions. Native auctions use the
# documented test-host deployment adapter. The ordinary check script separately
# runs the entire suite with all four actual WASM contracts and real SACs.
for contract_test_mode in services auction; do
  CROSSING_TEST_MODE="$contract_test_mode" cargo llvm-cov --workspace --locked --no-report -- --test-threads=1
done
report() {
  local diagnostics="$1"
  shift
  if cargo llvm-cov report "$@" 2>"$diagnostics"; then
    cat "$diagnostics" >&2
  else
    cat "$diagnostics" >&2
    return 1
  fi
}
report target/coverage/export.stderr --json --ignore-filename-regex '/tests/|/test\.rs$' --output-path target/coverage/coverage.json
report target/coverage/html.stderr --html --ignore-filename-regex '/tests/|/test\.rs$' --output-dir target/coverage
python3 scripts/check_coverage.py target/coverage/coverage.json \
  --diagnostics target/coverage/export.stderr target/coverage/html.stderr
