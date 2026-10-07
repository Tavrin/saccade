#!/usr/bin/env bash
# Offline capture conformance qualification; no producer, browser or provider run.
set -u
cd "$(dirname "$0")/.." || exit 1
source scripts/gate-env.sh
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
features=$(python3 - <<'PY'
import pathlib, tomllib
features = []
for name in ('saccade-core', 'saccade'):
    data = tomllib.loads((pathlib.Path('crates') / name / 'Cargo.toml').read_text())
    features.extend(name + '/' + feature for feature in data.get('features', {})
                    if feature not in ('default', 'imgtune-avif'))
print(','.join(sorted(features)))
PY
)
workspace_features="$features,saccade-py/http,saccade-py/models,saccade-py/python-tests"
failed=0
gate() {
    local name=$1
    shift
    if [[ "$name" == clippy-* || "$name" == tests-* || "$name" == build-* ]]; then
        if ! saccade_headroom; then printf 'GATE %s exit=2 (disk-headroom)\n' "$name"; failed=1; return; fi
        if [[ -d "$CARGO_TARGET_DIR" ]] && (( $(du -sk "$CARGO_TARGET_DIR" | cut -f1) >= 9765625 )); then
            printf 'GATE %s exit=2 (target-size)\n' "$name"; failed=1; return
        fi
    fi
    saccade_run "$@"
    local code=$?
    printf 'GATE %s exit=%s\n' "$name" "$code"
    if (( code != 0 )); then failed=1; fi
}
gate fmt cargo fmt --all -- --check
gate fixtures python3 scripts/gen-capture-kit.py --check
gate docs-generator-tests python3 scripts/test-gen-docs.py
gate clippy-default nice -n 19 cargo clippy -j 4 --workspace --all-targets --locked -- -D warnings
gate clippy-features nice -n 19 cargo clippy -j 4 --workspace --all-targets --features "$workspace_features" --locked -- -D warnings
gate tests-default nice -n 19 cargo test -j 4 -p saccade-core -p saccade --locked
gate tests-features nice -n 19 cargo test -j 4 -p saccade-core -p saccade --test capture_conformance --features "$features" --locked
gate tests-schemas nice -n 19 cargo test -j 4 -p saccade-core -p saccade --lib evidence_quality::schemas --features "$features" --locked
gate build-features nice -n 19 cargo build -j 4 -p saccade --features "$features" --locked
gate docs-generate python3 scripts/gen-docs.py --allow-missing-imgtune-avif --saccade "$CARGO_TARGET_DIR/debug/saccade"
gate docs-check python3 scripts/gen-docs.py --check --allow-missing-imgtune-avif --saccade "$CARGO_TARGET_DIR/debug/saccade"
gate public-hygiene bash scripts/check-public-hygiene.sh
gate genericity bash scripts/check-genericity.sh
gate guides python3 scripts/test-guides.py --bin "$CARGO_TARGET_DIR/debug/saccade"
exit "$failed"
