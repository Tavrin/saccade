#!/usr/bin/env bash
# Arm validity qualification with generated local records and images only.
set -u
cd "$(dirname "$0")/.." || exit 1
source scripts/gate-env.sh
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
features=graphics,mcp,schema,ai
failed=0
gate() {
    local name=$1
    shift
    case "$name" in
        check|clippy-core|clippy-cli|core|metadata|config|cli|schemas)
            if ! saccade_headroom; then printf 'GATE %s FAIL (disk-headroom)\n' "$name"; failed=1; return; fi ;;
    esac
    if saccade_run "$@"; then printf 'GATE %s PASS\n' "$name"; else printf 'GATE %s FAIL\n' "$name"; failed=1; fi
}
gate fmt cargo fmt --all -- --check
gate check nice -n 19 cargo check -j 4 -p saccade --no-default-features --features "$features" --locked
gate clippy-core nice -n 19 cargo clippy -j 4 -p saccade-core --no-default-features --features graphics,schema --lib --locked -- -D warnings
gate clippy-cli nice -n 19 cargo clippy -j 4 -p saccade --no-default-features --features "$features" --bin saccade --test arm_validity --locked -- -D warnings
gate core nice -n 19 cargo test -j 4 -p saccade-core --lib arms:: --no-default-features --features graphics,schema --locked
gate metadata nice -n 19 cargo test -j 4 -p saccade-core --lib meta:: --no-default-features --features graphics,schema --locked
gate config nice -n 19 cargo test -j 4 -p saccade-core --lib config:: --no-default-features --features graphics,schema --locked
gate cli nice -n 19 cargo test -j 4 -p saccade --test arm_validity --no-default-features --features "$features" --locked
gate schemas nice -n 19 cargo test -j 4 -p saccade-core --lib evidence_quality::schemas --no-default-features --features graphics,schema --locked
gate docs python3 scripts/gen-docs.py --check
gate rendering-docs python3 scripts/check-wave9-docs.py
gate public-hygiene bash scripts/check-public-hygiene.sh
exit "$failed"
