#!/usr/bin/env bash
# Permanent offline extension gate. No downloads, providers, browsers or GPU.
set -uo pipefail
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
source scripts/gate-env.sh
features="$(python3 - <<'PY'
import tomllib
with open('crates/saccade/Cargo.toml','rb') as f:
    print(','.join(k for k in tomllib.load(f)['features'] if k not in ('default','imgtune-avif')))
PY
)"
failed=0
gate() {
  local name="$1"; shift
  if ! saccade_headroom; then
    failed=1
    printf 'GATE %s BLOCKED exit=75\n' "$name"
    return 75
  fi
  "$@"
  local code=$?
  printf 'GATE %s %s exit=%s\n' "$name" "$([[ $code = 0 ]] && echo PASS || echo FAIL)" "$code"
  [[ $code = 0 ]] || failed=1
}
gate fmt cargo fmt --all --check
gate clippy-default nice -n 19 cargo clippy --offline -j 4 --workspace --all-targets -- -D warnings
gate clippy-features nice -n 19 cargo clippy --offline -j 4 --workspace --all-targets --features "$features" -- -D warnings
gate workspace-tests nice -n 19 cargo test --offline -j 4 --workspace --no-fail-fast --features "$features"
gate generated-evaluation nice -n 19 cargo test --offline -j 4 -p saccade-a11y --test automatic -- --nocapture
gate binary nice -n 19 cargo build --offline -j 4 -p saccade --features "$features"
gate docs-generate python3 scripts/gen-docs.py --saccade "$CARGO_TARGET_DIR/debug/saccade" --allow-missing-imgtune-avif --preserve-all-features-header
gate gen-docs python3 scripts/gen-docs.py --check --saccade "$CARGO_TARGET_DIR/debug/saccade" --allow-missing-imgtune-avif --preserve-all-features-header
gate public-hygiene bash scripts/check-public-hygiene.sh
gate guides python3 scripts/test-guides.py --bin "$CARGO_TARGET_DIR/debug/saccade"
gate package-list cargo package --offline -p saccade-a11y --list --allow-dirty
gate package-inventory python3 scripts/test-check-packages.py
exit "$failed"
