#!/usr/bin/env bash
# Coordinator only: run via the shared heavy queue after integration. No live providers.
set -u
cd "$(dirname "$0")/.." || exit 1
export CARGO_TARGET_DIR=/mnt/linux-extra/moss-cargo-targets/codex-saccade-w7
features=local-models,local-vlm,vision-providers,schema
failed=0
gate() {
    local name=$1
    shift
    case "$name" in
        clippy|default-tests|minimal-tests|models|local-network|schemas)
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
# Require explicit installed artifacts/receipts. No implicit downloads or changed pins.
gate models nice -n 19 cargo test -j 4 -p saccade-core --lib wave7::heavy_tests --features "$features" -- --ignored --skip local_vlm_endpoint_smoke
# Only a loopback local server, never Claude/GPT/Gemini or credential-dependent calls.
gate local-network nice -n 19 cargo test -j 4 -p saccade-core --lib wave7::heavy_tests::local_vlm_endpoint_smoke --features "$features" -- --ignored
gate docs python3 scripts/check-wave7-docs.py
gate schemas nice -n 19 cargo test -j 4 -p saccade-core --lib wave7::schemas --features "$features"
# Real selected native tokenizer/SAM/TrustMark adapters are explicitly deferred.
# Contract receipt tests above do not qualify those missing implementations.
printf 'GATE selected-native-adapters FAIL (deferred: detector-tokenizer/SAM/TrustMark-ECC)\n'
failed=1
exit "$failed"
