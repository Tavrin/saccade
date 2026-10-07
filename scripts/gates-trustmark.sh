#!/usr/bin/env bash
# Cached-only TrustMark qualification. All model provisioning is operator-owned.
set -u
cd "$(dirname "$0")/.." || exit 1
source scripts/gate-env.sh
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
failed=0
gate() {
    local name=$1
    shift
    if [[ "$name" == clippy-* || "$name" == tests-* || "$name" == schemas || "$name" == acceptance ]]; then
        if ! saccade_headroom; then printf 'GATE %s FAIL (disk-headroom)\n' "$name"; failed=1; return; fi
    fi
    if saccade_run "$@"; then printf 'GATE %s PASS\n' "$name"; else printf 'GATE %s FAIL\n' "$name"; failed=1; fi
}
gate fmt cargo fmt --all -- --check
gate clippy-default nice -n 19 cargo clippy -j 4 -p saccade -p saccade-core --all-targets -- -D warnings
gate clippy-local-models nice -n 19 cargo clippy -j 4 -p saccade -p saccade-core --features local-models --all-targets -- -D warnings
gate tests-default nice -n 19 cargo test -j 4 -p saccade -p saccade-core
gate tests-local-models nice -n 19 cargo test -j 4 -p saccade -p saccade-core --features local-models
gate schemas nice -n 19 cargo test -j 4 -p saccade-core --features local-models,schema --lib wave7::schemas
if [[ -n "${G25_TRUSTMARK_CACHE:-}" && -n "${G25_TRUSTMARK_RUNTIME:-}" ]]; then
    gate acceptance timeout 120 nice -n 19 cargo test -j 4 -p saccade-core --features local-models --test trustmark_decode -- --ignored --nocapture
else
    printf 'GATE acceptance FAIL (explicit G25_TRUSTMARK_CACHE and G25_TRUSTMARK_RUNTIME required)\n'
    failed=1
fi
if [[ -n "${SACCADE_DOCS_BIN:-}" ]]; then
    gate docs python3 scripts/gen-docs.py --saccade "$SACCADE_DOCS_BIN" --allow-missing-imgtune-avif --check
else
    gate docs python3 scripts/gen-docs.py --check
fi
gate public-hygiene bash scripts/check-public-hygiene.sh
gate plugin-manifests python3 scripts/validate-plugin-manifests.py
exit "$failed"
