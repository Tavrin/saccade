#!/usr/bin/env bash
# Coordinator only: run via the shared heavy queue after integration. No live providers.
set -u
cd "$(dirname "$0")/.." || exit 1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/mnt/linux-extra/moss-cargo-targets/codex-saccade-w7}"
features=local-models,local-vlm,vision-providers,schema
failed=0
gate() {
    local name=$1
    shift
    case "$name" in
        clippy|default-tests|minimal-tests|model-build|models|runtime-cli|local-network|schemas)
            if ! headroom; then printf 'GATE %s FAIL (disk-headroom)\n' "$name"; failed=1; return; fi ;;
    esac
    if "$@"; then printf 'GATE %s PASS\n' "$name"; else printf 'GATE %s FAIL\n' "$name"; failed=1; fi
}
headroom() {
    local available
    available=$(df -BG --output=avail /mnt/linux-extra | tail -n 1 | tr -cd '0-9')
    test "${available:-0}" -ge 25
}
if ! headroom; then printf 'GATE disk-headroom FAIL\n'; exit 1; fi
printf 'GATE disk-headroom PASS\n'
gate fmt cargo fmt --all -- --check
# These are deliberately full touched-crate gates, never run during implementation.
gate clippy nice -n 19 cargo clippy -j 4 -p saccade -p saccade-core --features "$features" --all-targets -- -D warnings
gate default-tests nice -n 19 cargo test -j 4 -p saccade -p saccade-core --features "$features"
gate minimal-tests nice -n 19 cargo test -j 4 -p saccade-core --no-default-features
# Only immutable URLs and exact hashes from the lane's frozen pin catalog.
export WAVE7_MODEL_CACHE=/mnt/linux-extra/saccade-models
export WAVE7_MODEL_REGISTRY="${WAVE7_MODEL_REGISTRY:-$PWD/crates/saccade-core/assets/wave7-models.json}"
gate model-pins python3 scripts/wave7/pull.py
gate model-fixtures python3 scripts/wave7/fixtures.py "$WAVE7_MODEL_CACHE/fixtures"
# Runtime provisioning is explicit in this coordinator-operated gate; inference never pulls it.
gate runtime-pull "$CARGO_TARGET_DIR/debug/saccade" models pull runtime --cache "$WAVE7_MODEL_CACHE" --json
if test -z "${WAVE7_RUNTIME_LIBRARY:-}"; then
    export WAVE7_RUNTIME_LIBRARY
    WAVE7_RUNTIME_LIBRARY=$(python3 -c 'import json,os; p=json.load(open("crates/saccade-core/assets/wave7-runtime.json")); print(os.environ["WAVE7_MODEL_CACHE"]+"/runtime/"+p["sha256"]+"/lib/libonnxruntime.so.1.22.0")')
fi
export ORT_DYLIB_PATH="$WAVE7_RUNTIME_LIBRARY"
gate runtime-cli nice -n 19 cargo test -j 4 -p saccade --test wave7_runtime --features "$features" -- --ignored
model_build() {
    mkdir -p "$WAVE7_MODEL_CACHE/evidence"
    nice -n 19 cargo test -j 4 -p saccade-core --lib wave7 --features "$features" --no-run --message-format=json > "$WAVE7_MODEL_CACHE/evidence/model-build.jsonl"
}
model_smokes() {
    local binary
    binary=$(python3 -c 'import json,os; rows=[json.loads(x) for x in open(os.environ["WAVE7_MODEL_CACHE"]+"/evidence/model-build.jsonl")]; print(next(x["executable"] for x in rows if x.get("executable") and x.get("target",{}).get("name")=="saccade_core"))') || return 1
    WAVE7_SMOKE_BIN="$binary" python3 scripts/models/smoke-rust.py || return 1
    WAVE7_SMOKE_BIN="$binary" timeout 55 nice -n 19 "$binary" wave7::heavy_tests::pinned_dino_rectangle_maps_to_original_image --exact --ignored
}
gate model-build model_build
gate models model_smokes
# Frozen legacy/source parity is a separate qualification, beyond generated behavior.
if test -n "${WAVE7_HEAVY_FIXTURES:-}"; then
    gate source-parity nice -n 19 cargo test -j 4 -p saccade-core --lib wave7::heavy_tests --features "$features" -- --ignored --skip pinned_ --skip local_vlm_endpoint_smoke
else
    printf 'GATE source-parity DEFERRED (no reviewed parity bundle)\n'
fi
# Only a loopback local server, never Claude/GPT/Gemini or credential-dependent calls.
if test -n "${WAVE7_LOCAL_VLM_ENDPOINT:-}"; then
    gate local-network nice -n 19 cargo test -j 4 -p saccade-core --lib wave7::heavy_tests::local_vlm_endpoint_smoke --features "$features" -- --ignored
else
    printf 'GATE local-network DEFERRED (no explicit loopback model server)\n'
fi
gate docs python3 scripts/check-wave7-docs.py
gate schemas nice -n 19 cargo test -j 4 -p saccade-core --lib wave7::schemas --features "$features"
# These explicit research gaps cannot be turned green by generated or replay evidence.
printf 'GATE selected-native-adapters DEFERRED (SAM2 offline export tooling; LPIPS/DISTS weight grants; MUSIQ checkpoint; TrustMark ECC/resize parity)\n'
# Opt-in release qualification may require every selected adapter. Deferrals are explicit.
if test "${WAVE7_REQUIRE_ALL_MODELS:-0}" = 1; then
    printf 'GATE required-all-models FAIL (recorded deferrals)\n'; failed=1
fi
exit "$failed"
