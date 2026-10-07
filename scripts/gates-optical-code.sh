#!/usr/bin/env bash
# Offline gates for optical-code; target/resource policy is caller-owned.
set -uo pipefail
source scripts/gate-env.sh
scratch="${SACCADE_QR_GATE_LOGS:-$(mktemp -d)}"
mkdir -p "$scratch"
features=$(python3 - <<'PY'
import tomllib
with open('crates/saccade/Cargo.toml','rb') as f:
    features=tomllib.load(f)['features']
print(','.join(sorted(set(features)-{'default','imgtune-avif'})))
PY
)
failed=0
run() {
    local name="$1"; shift
    "$@" >"$scratch/$name.log" 2>&1
    local code=$?
    printf 'GATE %s %s exit=%s\n' "$name" "$([[ $code == 0 ]] && echo PASS || echo FAIL)" "$code"
    if [[ $code != 0 ]]; then tail -n 35 "$scratch/$name.log"; failed=1; fi
}
build_gate() {
    local name="$1"; shift
    if ! saccade_headroom >"$scratch/$name.log" 2>&1; then
        printf 'GATE %s BLOCKED exit=1\n' "$name"; failed=1; return
    fi
    run "$name" nice -n 19 cargo "$@" -j 4 --locked
}
run fmt cargo fmt --all --check
if saccade_headroom; then
    run clippy-default nice -n 19 cargo clippy -j 4 --locked --workspace --all-targets -- -D warnings
else failed=1; printf 'GATE clippy-default BLOCKED exit=1\n'; fi
if saccade_headroom; then
    run clippy-features nice -n 19 cargo clippy -j 4 --locked --workspace --all-targets --features "$features" -- -D warnings
else failed=1; printf 'GATE clippy-features BLOCKED exit=1\n'; fi
build_gate tests-default test -p saccade-core -p saccade
build_gate tests-features test -p saccade-core -p saccade --features "$features"
build_gate binary build -p saccade --features "$features"
binary="$CARGO_TARGET_DIR/debug/saccade"
if [[ -x "$binary" ]]; then
    run gen-docs python3 scripts/gen-docs.py --saccade "$binary" --allow-missing-imgtune-avif --check
    run guides python3 scripts/test-guides.py --bin "$binary"
else failed=1; printf 'GATE gen-docs BLOCKED exit=1\nGATE guides BLOCKED exit=1\n'; fi
run docs-generator python3 scripts/test-gen-docs.py
run public-hygiene bash scripts/check-public-hygiene.sh
exit "$failed"
