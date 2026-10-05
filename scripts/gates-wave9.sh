#!/usr/bin/env bash
# Coordinator only: run through shared heavy admission after integration. No providers/GPU.
set -u
cd "$(dirname "$0")/.." || exit 1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/mnt/linux-extra/moss-cargo-targets/codex-saccade-w9}"
export CARGO_INCREMENTAL=0
features=graphics,mcp,compression,schema
failed=0
headroom() {
    local available
    available=$(df -BG --output=avail /mnt/linux-extra | tail -n 1 | tr -cd '0-9')
    test "${available:-0}" -ge 25
}
gate() {
    local name=$1
    shift
    case "$name" in
        clippy|full-tests|minimal-tests|cli|schemas|realworld)
            if ! headroom; then printf 'GATE %s FAIL (disk-headroom)\n' "$name"; failed=1; return; fi ;;
    esac
    if "$@"; then printf 'GATE %s PASS\n' "$name"; else printf 'GATE %s FAIL\n' "$name"; failed=1; fi
}
gate fmt cargo fmt --all -- --check
gate clippy nice -n 19 cargo clippy -j 4 -p saccade -p saccade-core --features "$features" --all-targets -- -D warnings
gate full-tests env -u WAVE9_READONLY_INVENTORY -u WAVE9_EFFECT_INVENTORY nice -n 19 cargo test -j 4 -p saccade -p saccade-core --features "$features"
gate minimal-tests env -u WAVE9_READONLY_INVENTORY -u WAVE9_EFFECT_INVENTORY nice -n 19 cargo test -j 4 -p saccade-core --no-default-features
gate cli nice -n 19 cargo test -j 4 -p saccade --test wave9_evidence --features "$features" -- --ignored
gate schemas nice -n 19 cargo test -j 4 -p saccade-core --test schemas --features "$features"
gate docs python3 scripts/check-wave9-docs.py
# Held-out project images never become repository fixtures; lane records contain the receipts.
if test -n "${WAVE9_READONLY_INVENTORY:-}" || test -n "${WAVE9_EFFECT_INVENTORY:-}"; then
    gate realworld nice -n 19 cargo test -j 4 -p saccade-core --lib evidence_quality::realworld --features "$features"
else
    printf 'GATE realworld DEFERRED (external read-only inventories not supplied; lane receipts in WAVE9-NOTES.md)\n'
fi
exit "$failed"
