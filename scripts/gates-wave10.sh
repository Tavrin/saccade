#!/usr/bin/env bash
# Coordinator gate: generated fixtures, no network/model downloads or GPU.
set -u
cd "$(dirname "$0")/.." || exit 1
source scripts/gate-env.sh
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
features=graphics,mcp,schema,ai
failed=0
gate() {
  local name=$1; shift
  if saccade_run timeout --signal=TERM --kill-after=10 840 "$@"; then
    printf 'GATE %s PASS\n' "$name"
  else
    printf 'GATE %s FAIL\n' "$name"; failed=1
  fi
}
cargo_gate() {
  local name=$1; shift
  if ! saccade_headroom; then printf 'GATE %s FAIL (disk-headroom)\n' "$name"; failed=1; return; fi
  gate "$name" nice -n 19 cargo "$@"
}
gate fmt cargo fmt --all -- --check
gate docs python3 scripts/check-wave10-docs.py
gate genericity bash scripts/check-genericity.sh
cargo_gate clippy clippy -j4 -p saccade -p saccade-core --no-default-features --features "$features" --all-targets -- -D warnings
cargo_gate optional-clippy clippy -j4 -p saccade --no-default-features --features "$features,local-models,embeddings,ocr" --all-targets -- -D warnings
cargo_gate full-core-tests test -j4 -p saccade-core --no-default-features --features graphics,schema
cargo_gate full-cli-tests test -j4 -p saccade --no-default-features --features "$features"
cargo_gate minimal-tests test -j4 -p saccade-core --no-default-features
cargo_gate wave10-cli test -j4 -p saccade --no-default-features --features "$features" --test wave10_field -- --ignored
cargo_gate schemas test -j4 -p saccade-core --no-default-features --features graphics,schema --test schemas
cargo_gate python-clippy clippy -j4 -p saccade-py --features python-tests --all-targets -- -D warnings
cargo_gate python-arrays test -j4 -p saccade-py --features python-tests --test python_package
# Existing optional-runtime qualification uses installed pinned assets; never pulls them here.
cargo_gate video test -j4 -p saccade-core --lib media::video -- --ignored
if test "${SACCADE_W10_QUALIFY_MODELS:-0}" = 1; then
  # Explicit installed pins only; use the existing fixture/cache/runtime variables.
  cargo_gate model-runtime test -j4 -p saccade-core --lib wave7::heavy_tests --no-default-features --features graphics,local-models -- --ignored
  cargo_gate media-runtime test -j4 -p saccade-core --lib media::heavy_tests::installed_media_sections_reuse_sessions --no-default-features --features graphics,local-models,embeddings,ocr -- --ignored
else
  printf 'GATE model-runtime DEFERRED (opt in with SACCADE_W10_QUALIFY_MODELS=1 and installed frozen fixtures; no downloads)\n'
fi
# The wheel smoke checks distribution metadata against the native version.
# Supply an installed distribution in this interpreter; no implicit package installs.
if test -n "${SACCADE_W10_WHEEL_PYTHON:-}"; then
  gate installed-wheel "$SACCADE_W10_WHEEL_PYTHON" crates/saccade-py/tests/smoke_wheel.py
else
  printf 'GATE installed-wheel DEFERRED (set SACCADE_W10_WHEEL_PYTHON to an interpreter with the wheel installed)\n'
fi
# Release wheel builds/platforms and model parity remain in existing release/runtime gates.
exit "$failed"
