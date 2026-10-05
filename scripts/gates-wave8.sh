#!/usr/bin/env bash
# Coordinator only: invoke through the shared CPU-heavy queue after integration.
# Every command has a <=15-minute limit; no live hosted providers or publication.
set -u
cd "$(dirname "$0")/.." || exit 1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/mnt/linux-extra/moss-cargo-targets/codex-saccade-w8}"
export CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export SACCADE_W8_EVIDENCE="${SACCADE_W8_EVIDENCE:-/mnt/linux-extra/moss-scratch/saccade-wave8/heavy}"
failed=0
features=local-models,embeddings,ocr,vision-providers,media-http,workbench,mcp,schema
gate() {
    local name=$1
    shift
    if timeout 900 "$@"; then printf 'GATE %s PASS\n' "$name"; else printf 'GATE %s FAIL\n' "$name"; failed=1; fi
}
cargo_gate() {
    local name=$1
    shift
    gate "$name" python3 scripts/wave8-dev-cargo.py cargo "$@"
}
# The complete coordinator-owned gates are written here, not run in development.
gate fmt cargo fmt --all -- --check
gate docs python3 scripts/wave8/check-docs.py
cargo_gate clippy clippy -j 4 -p saccade -p saccade-core --features "$features" --all-targets -- -D warnings
cargo_gate python-clippy clippy -j 4 -p saccade-py --features models,http,python-tests --all-targets -- -D warnings
cargo_gate core-tests test -j 4 -p saccade-core --features "$features"
cargo_gate cli-tests test -j 4 -p saccade --features "$features"
cargo_gate minimal-tests test -j 4 -p saccade-core --no-default-features
# No model downloads unless this explicit gate provisions immutable wave 7 assets.
export SACCADE_W8_MODEL_DIR="${SACCADE_W8_MODEL_DIR:-/mnt/linux-extra/saccade-models}"
: "${SACCADE_W8_PYTHON:=python3}"
export SACCADE_W8_PYTHON
cargo_gate runtime-cli-build test -j 4 -p saccade --features "$features" --test wave8_contract --no-run
gate runtime-pull "$CARGO_TARGET_DIR/debug/saccade" models pull runtime --cache "$SACCADE_W8_MODEL_DIR" --json
gate face-model-pull "$CARGO_TARGET_DIR/debug/saccade" models pull yunet-2026may --cache "$SACCADE_W8_MODEL_DIR" --json
# Registry must contain the reviewed image embedding and Rust OCR exports; never guess pins.
if test -n "${SACCADE_W8_REGISTRY:-}"; then
    export SACCADE_W8_REGISTRY
    cargo_gate models test -j 4 -p saccade-core --features "$features" --lib media::heavy_tests -- --ignored
else
    printf 'GATE models FAIL (reviewed embedding/OCR registry required)\n'; failed=1
fi
cargo_gate ffmpeg test -j 4 -p saccade-core --features "$features" --lib media::video -- --ignored
cargo_gate python-light test -j 4 -p saccade-py --features python-tests --test python_package -- --nocapture
# Release/manylinux wheel construction is deliberately heavy and artifact-only.
gate wheel-release python3 scripts/wave8-dev-cargo.py "$SACCADE_W8_PYTHON" -m maturin build --release --locked -j 4 --manifest-path crates/saccade-py/Cargo.toml --features models,http --out "$SACCADE_W8_EVIDENCE/wheels"
gate wheel-install "$SACCADE_W8_PYTHON" -m pip install --force-reinstall "$SACCADE_W8_EVIDENCE"/wheels/*.whl
gate python-models "$SACCADE_W8_PYTHON" -m pytest -q crates/saccade-py/tests/test_models.py
# CI builds both native architectures; require downloaded CI wheel artifacts here.
gate manylinux-artifacts python3 scripts/wave8/check-wheels.py
gate docker-build python3 scripts/wave8-dev-cargo.py docker build -f Dockerfile.wave8 -t saccade-wave8-gate:local .
gate docker-smoke python3 scripts/wave8/docker-smoke.py
# An unpinned joint checkpoint cannot be qualified by vector arithmetic or fake provider fixtures.
printf 'GATE text-image-model FAIL (deferred official checkpoint/tokenizer/export licence and pins; bands uncalibrated)\n'
failed=1
exit "$failed"
