#!/usr/bin/env bash
# G28 acceptance, bounded resource envelope. No release/GPU/model/provider jobs.
set -u
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:?set the assigned lane target directory}"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
source scripts/gate-env.sh
status=0
gate() {
  local name="$1"; shift
  if "$@"; then echo "GATE $name PASS exit=0";
  else local rc=$?; echo "GATE $name FAIL exit=$rc"; status=1; fi
}
build() {
  saccade_headroom || return 75
  if [[ -d "$CARGO_TARGET_DIR" ]] && (( $(du -sb "$CARGO_TARGET_DIR" | cut -f1) >= 8000000000 )); then
    echo 'print target exceeds 8 GB'; return 75
  fi
  local subcommand="$1"; shift
  timeout 900 nice -n 19 cargo "$subcommand" -j 4 "$@"
}
gate fmt cargo fmt --all -- --check
gate clippy-default build clippy --all-targets -- -D warnings
gate clippy-print build clippy --all-targets --features print -- -D warnings
gate clippy-extension build clippy -p saccade-print --all-targets -- -D warnings
gate tests-extension build test -p saccade-print
gate tests-cli-default build test -p saccade
gate tests-cli-print build test -p saccade --features print
gate schema-discovery build test -p saccade --features print --test wave10_field schema_discovery_and_perf_validator_use_exact_shipped_documents -- --ignored
gate build-cli build build -p saccade --features print
fixtures=$(mktemp -d)
trap 'rm -rf "$fixtures"' EXIT
gate fixtures build run -p saccade-print --example fixtures -- "$fixtures"
gate cross-domain python3 scripts/test-print.py --saccade "$CARGO_TARGET_DIR/debug/saccade" --fixtures "$fixtures"
gate docs python3 scripts/gen-docs.py --check
gate hygiene bash scripts/check-public-hygiene.sh
gate genericity bash scripts/check-genericity.sh
gate package cargo package -p saccade-print --list --allow-dirty
exit "$status"
