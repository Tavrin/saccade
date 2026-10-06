#!/usr/bin/env bash
# Focused CPU lane receipts; no providers or broad GPU/workspace gates.
set -uo pipefail
export CARGO_TARGET_DIR=/mnt/linux-extra/moss-cargo-targets/codex-saccade-ocr
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
scratch=${SACCADE_OCR_SCRATCH:-/mnt/linux-extra/moss-scratch/saccade-ocr}
cache=${SACCADE_OCR_CACHE:-/mnt/linux-extra/saccade-models}
mkdir -p "$scratch"
fail=0
gate() { local name=$1; shift; if "$@"; then echo "GATE $name PASS"; else echo "GATE $name FAIL"; fail=1; fi; }
cargo_gate() {
  python3 -c 'import shutil; assert shutil.disk_usage("/mnt/linux-extra").free >= 25_000_000_000, "less than 25 GB free"' || return 1
  nice -n 19 cargo "$1" -j 4 "${@:2}"
}
gate fmt cargo fmt --all -- --check
gate minimal cargo_gate check -p saccade --no-default-features
gate clippy cargo_gate clippy -p saccade -p saccade-core --no-default-features --features ocr,ocr-provider,mcp --lib --bin saccade -- -D warnings
gate core cargo_gate test -p saccade-core --features ocr,ocr-provider --lib general::
gate cli cargo_gate test -p saccade --no-default-features --features ocr,ocr-provider,mcp --test ocr_contract
# Frozen generation precedes model inference; Python with Pillow must be provided explicitly.
gate generate "${SACCADE_OCR_PYTHON:-python3}" scripts/models/generate-ocr-accents.py "$scratch/accents"
export SACCADE_OCR_ACCENTS="$scratch/accents" SACCADE_OCR_CACHE="$cache"
gate local-inference cargo_gate test -p saccade --no-default-features --features ocr,ocr-provider,mcp --test ocr_contract -- --ignored
gate accents cargo_gate run -p saccade-core --features ocr --example ocr_accents -- "$scratch/accents" "$cache" "$scratch/accent-results.json"
gate docs python3 scripts/check-wave6-docs.py
gate genericity bash scripts/check-genericity.sh
exit "$fail"
