#!/usr/bin/env bash
# Explicit feature qualification gates; see docs/releasing.md.
# Every command has a <=15-minute limit; no live hosted providers or publication.
set -u
cd "$(dirname "$0")/.." || exit 1
source scripts/gate-env.sh
export CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export SACCADE_W8_EVIDENCE="${SACCADE_W8_EVIDENCE:-$CARGO_TARGET_DIR/wave8-evidence}"
failed=0
features=local-models,embeddings,ocr,vision-providers,media-http,workbench,mcp,schema
gate() {
    local name=$1
    shift
    local -a command=(timeout 900 "$@")
    if saccade_run "${command[@]}"; then printf 'GATE %s PASS\n' "$name"; else printf 'GATE %s FAIL\n' "$name"; failed=1; fi
}
cargo_gate() {
    local name=$1
    shift
    saccade_headroom || { failed=1; return 1; }
    gate "$name" nice -n 19 cargo "$@"
}
# These gates require installed runtime/model inputs for model qualification.
gate fmt cargo fmt --all -- --check
gate docs python3 scripts/wave8/check-docs.py
cargo_gate clippy clippy -j 4 -p saccade -p saccade-core --features "$features" --all-targets -- -D warnings
cargo_gate python-clippy clippy -j 4 -p saccade-py --features models,http,python-tests --all-targets -- -D warnings
cargo_gate core-tests test --no-fail-fast -j 4 -p saccade-core --features "${features//,mcp/}"
cargo_gate cli-tests test --no-fail-fast -j 4 -p saccade --features "$features"
cargo_gate minimal-tests test -j 4 -p saccade-core --no-default-features
# No model downloads unless this explicit gate provisions immutable wave 7 assets.
export SACCADE_W8_MODEL_DIR="${SACCADE_W8_MODEL_DIR:-$SACCADE_MODEL_CACHE}"
: "${SACCADE_W8_PYTHON:=python3}"
export SACCADE_W8_PYTHON
cargo_gate runtime-cli-build test -j 4 -p saccade --features "$features" --test wave8_contract --no-run
gate runtime-pull "$CARGO_TARGET_DIR/debug/saccade" models pull runtime --cache "$SACCADE_W8_MODEL_DIR" --json
gate face-model-pull "$CARGO_TARGET_DIR/debug/saccade" models pull yunet-2026may --cache "$SACCADE_W8_MODEL_DIR" --json
# Registry must contain the reviewed image embedding and Rust OCR exports; never guess pins.
if test -n "${SACCADE_W8_REGISTRY:-}"; then
    export SACCADE_W8_REGISTRY
    cargo_gate models test -j 4 -p saccade-core --features "${features//,mcp/}" --lib media::heavy_tests::installed_media_sections_reuse_sessions -- --ignored
else
    printf 'GATE models FAIL (reviewed embedding/OCR registry required)\n'; failed=1
fi
cargo_gate ffmpeg test -j 4 -p saccade-core --features "${features//,mcp/}" --lib media::video -- --ignored
cargo_gate python-light test -j 4 -p saccade-py --features python-tests --test python_package -- --nocapture
if "$SACCADE_W8_PYTHON" -c 'import maturin' >/dev/null 2>&1; then
# Release/manylinux wheel construction is deliberately heavy and artifact-only.
gate wheel-release nice -n 19 "$SACCADE_W8_PYTHON" -m maturin build --release --locked -j 4 --manifest-path crates/saccade-py/Cargo.toml --features models,http --out "$SACCADE_W8_EVIDENCE/wheels"
gate wheel-install "$SACCADE_W8_PYTHON" -m pip install --force-reinstall "$SACCADE_W8_EVIDENCE"/wheels/*.whl
gate python-models "$SACCADE_W8_PYTHON" -m pytest -q crates/saccade-py/tests/test_models.py
else
    printf 'GATE wheel-release CI-ONLY (maturin unavailable locally)\n'
    printf 'GATE wheel-install CI-ONLY (release wheel unavailable)\n'
    # Compile the model-enabled package via its existing fixture target.
    cargo_gate python-model-build test -j 4 -p saccade-py --features models,python-tests --test python_package
    mkdir -p "$SACCADE_W8_EVIDENCE/python-model-package/saccade"
    cp crates/saccade-py/python/saccade/* "$SACCADE_W8_EVIDENCE/python-model-package/saccade/"
    cp "$CARGO_TARGET_DIR/debug/deps/lib_native.so" "$SACCADE_W8_EVIDENCE/python-model-package/saccade/_native.abi3.so"
    gate python-models env "PYTHONPATH=$SACCADE_W8_EVIDENCE/python-model-package" "$SACCADE_W8_PYTHON" -m pytest -q crates/saccade-py/tests/test_models.py
fi
# CI builds both native architectures; require downloaded CI wheel artifacts here.
if test -n "${SACCADE_W8_WHEELS:-}"; then
    gate manylinux-artifacts python3 scripts/wave8/check-wheels.py
else
    printf 'GATE manylinux-artifacts CI-ONLY (both architecture archives require CI)\n'
fi
if command -v docker >/dev/null 2>&1; then
gate docker-build docker build -f Dockerfile.wave8 -t saccade-wave8-gate:local .
gate docker-smoke python3 scripts/wave8/docker-smoke.py
else
    printf 'GATE docker-build CI-ONLY (Docker unavailable locally)\n'
    printf 'GATE docker-smoke CI-ONLY (Docker unavailable locally)\n'
fi
# Qualification requires the reviewed installed joint registry, never fake vectors.
if test -n "${SACCADE_W8_JOINT_REGISTRY:-}"; then
    export SACCADE_W8_JOINT_REGISTRY
    cargo_gate text-image-model test -j 4 -p saccade-core --features "${features//,mcp/}" --lib media::heavy_tests::installed_joint_text_index_roundtrip -- --ignored
else
    printf 'GATE text-image-model FAIL (pinned installed joint registry required; bands uncalibrated)\n'
    failed=1
fi
exit "$failed"
