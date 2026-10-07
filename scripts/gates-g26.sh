#!/usr/bin/env bash
# Reproduce the CPU-only captured-sequence acceptance and public repository gates.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${CARGO_TARGET_DIR:?Set the dedicated lane Cargo target directory}"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 RUST_TEST_THREADS=4
gate() {
    local name="$1"
    shift
    if [[ "$1" == cargo && "$2" != fmt ]]; then
        local available used=0
        available=$(df -Pk "$(dirname "$CARGO_TARGET_DIR")" | awk 'NR==2 {print $4}')
        if [[ -d "$CARGO_TARGET_DIR" ]]; then used=$(du -sk "$CARGO_TARGET_DIR" | cut -f1); fi
        if (( available < 25*1024*1024 || used >= 8*1024*1024 )); then
            printf 'GATE %s BLOCKED storage limits
' "$name"
            return 1
        fi
    fi
    if nice -n 19 "$@"; then
        printf 'GATE %s PASS (exit 0)\n' "$name"
    else
        local code=$?
        printf 'GATE %s FAIL (exit %s)\n' "$name" "$code"
        return "$code"
    fi
}
gate fmt cargo fmt --all -- --check
gate clippy cargo clippy -j 4 -p saccade-core -p saccade --features dense-motion,schema --all-targets --locked -- -D warnings
gate core-tests cargo test -j 4 -p saccade-core --features graphics,dense-motion,schema --locked
gate cli-tests cargo test -j 4 -p saccade --features dense-motion,schema --locked
gate generated-docs python3 scripts/gen-docs.py --check
gate public-hygiene bash scripts/check-public-hygiene.sh
