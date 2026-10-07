#!/usr/bin/env bash
# Offline gates for declared-case coverage and reference health.
set -euo pipefail
: "${CARGO_TARGET_DIR:?set an external Cargo target directory}"
: "${SACCADE_GATE_LOG_DIR:?set a gate-log directory outside the checkout}"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
source scripts/gate-env.sh
mkdir -p "$SACCADE_GATE_LOG_DIR"
features="$(python3 - <<'PY'
import tomllib
from pathlib import Path
features = tomllib.loads(Path('crates/saccade/Cargo.toml').read_text())['features']
print(','.join(sorted(set(features) - {'default', 'imgtune-avif'})))
PY
)"
gate() {
  local name="$1"
  shift
  saccade_headroom
  if [[ -d "$CARGO_TARGET_DIR" ]] && (( $(du -sb "$CARGO_TARGET_DIR" | cut -f1) >= 10000000000 )); then
    echo "GATE $name FAIL: target budget exceeded"
    return 1
  fi
  set +e
  timeout 900 "$@" > "$SACCADE_GATE_LOG_DIR/$name.log" 2>&1
  local code=$?
  set -e
  echo "$name $code" >> "$SACCADE_GATE_LOG_DIR/exits.txt"
  if (( code )); then
    echo "GATE $name FAIL (exit $code)"
    tail -40 "$SACCADE_GATE_LOG_DIR/$name.log"
    return "$code"
  fi
  echo "GATE $name PASS (exit 0)"
}
gate fmt cargo fmt --all --check
gate clippy-default nice -n 19 cargo clippy -j 4 --workspace --all-targets -- -D warnings
gate clippy-features nice -n 19 cargo clippy -j 4 --workspace --all-targets --features "$features" -- -D warnings
gate schemas nice -n 19 cargo test -j 4 -p saccade-core --features schema --test schemas
gate build-features nice -n 19 cargo build -j 4 -p saccade --features "$features"
# Keep the feature-complete producer while default-feature tests replace debug/saccade.
cp "$CARGO_TARGET_DIR/debug/saccade" "$SACCADE_GATE_LOG_DIR/saccade"
binary="$SACCADE_GATE_LOG_DIR/saccade"
gate docs-generate python3 scripts/gen-docs.py --allow-missing-imgtune-avif --saccade "$binary"
gate tests nice -n 19 cargo test -j 4 -p saccade-core -p saccade
gate docs-check python3 scripts/gen-docs.py --check --allow-missing-imgtune-avif --saccade "$binary"
gate docs-tests python3 scripts/test-gen-docs.py
gate hygiene bash scripts/check-public-hygiene.sh
gate guides python3 scripts/test-guides.py --bin "$binary"
gate proofs python3 scripts/gen-coverage-fixtures.py "$SACCADE_GATE_LOG_DIR/proofs" --bin "$binary"
