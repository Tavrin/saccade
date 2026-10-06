#!/usr/bin/env bash
# Focused CPU OCR gates; no providers or broad GPU/workspace gates.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
source scripts/gate-env.sh
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
scratch=${SACCADE_OCR_SCRATCH:-$CARGO_TARGET_DIR/ocr-evidence}
cache=${SACCADE_OCR_CACHE:-$SACCADE_MODEL_CACHE}
mkdir -p "$scratch"
fail=0
gate() { local name=$1; shift; if saccade_run "$@"; then echo "GATE $name PASS"; else echo "GATE $name FAIL"; fail=1; return 1; fi; }
cargo_gate() {
  while ! saccade_headroom; do
    echo "OCR build admission PAUSED: waiting for 25 GB free"
    sleep 30
  done
  saccade_run nice -n 19 cargo "$1" -j 4 "${@:2}"
}
gate fmt cargo fmt --all -- --check
gate minimal cargo_gate check -p saccade --no-default-features
gate clippy cargo_gate clippy -p saccade -p saccade-core --no-default-features --features ocr,ocr-provider,mcp --lib --bin saccade -- -D warnings
gate core cargo_gate test -p saccade-core --features ocr,ocr-provider --lib general::
gate cli cargo_gate test -p saccade --no-default-features --features ocr,ocr-provider,mcp --test ocr_contract
# Frozen generation precedes model inference; Python with Pillow must be provided explicitly.
fixtures_ready=0
if gate generate "${SACCADE_OCR_PYTHON:-python3}" scripts/models/generate-ocr-accents.py "$scratch/accents" &&
   gate frozen-contracts cmp scripts/models/ocr-accent-contracts.json "$scratch/accents/contracts.json"; then
  fixtures_ready=1
fi
export SACCADE_OCR_ACCENTS="$scratch/accents" SACCADE_OCR_CACHE="$cache"
if (( fixtures_ready )); then
gate local-inference cargo_gate test -p saccade --no-default-features --features ocr,ocr-provider,mcp --test ocr_contract -- --ignored
gate accents cargo_gate run -p saccade-core --features ocr --example ocr_accents -- "$scratch/accents" "$cache" "$scratch/accent-results.json"
else
  echo "GATE local-inference FAIL (fixtures not frozen)"
  echo "GATE accents FAIL (fixtures not frozen)"
  fail=1
fi
gate docs python3 scripts/check-wave6-docs.py
gate genericity bash scripts/check-genericity.sh
exit "$fail"
