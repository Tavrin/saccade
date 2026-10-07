#!/usr/bin/env bash
# Offline incremental-index lane gates; the caller supplies an isolated target directory.
set -uo pipefail
: "${CARGO_TARGET_DIR:?set an isolated Cargo target directory}"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
logs=${SACCADE_GATE_LOGS:-$(mktemp -d)}
mkdir -p "$logs" "$CARGO_TARGET_DIR"
failed=0
resource_check() {
    local free used
    free=$(df -B1 --output=avail "$CARGO_TARGET_DIR" | tail -1)
    used=$(du -sb "$CARGO_TARGET_DIR" | cut -f1)
    (( free >= 25 * 1024 * 1024 * 1024 && used < 8 * 1024 * 1024 * 1024 ))
}
gate() {
    local name=$1 rc
    shift
    if [[ $name == clippy-* || $name == tests-* ]]; then
        if ! resource_check; then
            printf 'GATE %s FAIL resource-bound\n' "$name"
            failed=1
            return
        fi
    fi
    timeout 900 "$@" >"$logs/$name.log" 2>&1
    rc=$?
    if (( rc == 0 )); then printf 'GATE %s PASS exit=%s\n' "$name" "$rc"; else
        printf 'GATE %s FAIL exit=%s\n' "$name" "$rc"
        tail -30 "$logs/$name.log"
        failed=1
    fi
}
gate fmt cargo fmt --all --check
gate clippy-default nice -n 19 cargo clippy -j 4 -p saccade-core -p saccade --all-targets -- -D warnings
gate clippy-embeddings nice -n 19 cargo clippy -j 4 -p saccade-core -p saccade --all-targets --features embeddings -- -D warnings
gate tests-default nice -n 19 cargo test -j 4 -p saccade-core -p saccade --no-fail-fast
gate tests-embeddings nice -n 19 cargo test -j 4 -p saccade-core -p saccade --no-fail-fast --features embeddings
gate gen-docs python3 scripts/gen-docs.py --check
gate public-hygiene bash scripts/check-public-hygiene.sh
gate genericity bash scripts/check-genericity.sh
if ! resource_check; then printf 'GATE final-resource-bound FAIL\n'; failed=1; fi
exit "$failed"
