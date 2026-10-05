#!/usr/bin/env bash
# Coordinator-only heavy queue entry point. Do not run during lane development.
set -uo pipefail
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/mnt/linux-extra/moss-cargo-targets/codex-saccade-w6}"
failed=0
gate() {
  local name=$1; shift
  if "$@"; then echo "GATE $name PASS"; else echo "GATE $name FAIL"; failed=1; fi
}
# Cargo options precede rustc/test arguments.
clippy_gate() { cargo_gate clippy -p saccade -p saccade-core --all-targets --features schema,embeddings,ocr,documents,credentials -- -D warnings; }
# Keep options before -- rather than appending -j to test-harness arguments.
cargo_gate() {
  local free_gb command=$1; shift
  free_gb=$(df -BG --output=avail /mnt/linux-extra | tail -1 | tr -dc '0-9')
  if [[ ! "$free_gb" =~ ^[0-9]+$ ]] || (( free_gb < 25 )); then echo 'disk admission: less than 25 GB free'; return 1; fi
  local toolchain=()
  if [[ -n "${SACCADE_W6_TOOLCHAIN:-}" ]]; then toolchain=("+$SACCADE_W6_TOOLCHAIN"); fi
  nice -n 19 cargo "${toolchain[@]}" "$command" -j 4 "$@"
}
gate fmt cargo fmt --all -- --check
gate check-minimal cargo_gate check -p saccade --no-default-features
msrv_gate() { SACCADE_W6_TOOLCHAIN=1.88 cargo_gate check -p saccade --no-default-features --features schema,embeddings,ocr,documents,credentials,mcp,parallel; }
gate msrv-wave6 msrv_gate
gate clippy clippy_gate
gate core-tests cargo_gate test -p saccade-core --features schema,embeddings,ocr,documents,credentials
gate cli-tests cargo_gate test -p saccade --features schema,embeddings,ocr,documents,credentials
gate registration-heavy cargo_gate test -p saccade-core --lib general::registration -- --ignored
gate embeddings-heavy cargo_gate test -p saccade-core --features embeddings --lib general::embedding:: -- --ignored
gate embedding-qualification cargo_gate test -p saccade-core --features embeddings --lib general::embedding_qualification -- --ignored
gate credentials-heavy cargo_gate test -p saccade-core --features credentials --lib general::credentials -- --ignored
gate forensic-history cargo_gate test -p saccade-core --lib general::forensics -- --ignored
gate export-script-help python3 scripts/export-wave6-embeddings.py --help
gate hashing-scale cargo_gate test -p saccade-core --lib general::hashing -- --ignored
gate wave6-cli-heavy cargo_gate test -p saccade --features embeddings,ocr,documents,credentials --test wave6_contract -- --ignored
gate docs python3 scripts/check-wave6-docs.py
exit "$failed"
