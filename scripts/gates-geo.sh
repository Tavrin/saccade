#!/usr/bin/env bash
# Bounded CPU gates for the optional native-raster extension.
set -u
cd "$(dirname "$0")/.."
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:?set the assigned external target directory}"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
source scripts/gate-env.sh
status=0
gate() {
  local name="$1"; shift
  if "$@"; then echo "GATE $name PASS exit=0";
  else local rc=$?; echo "GATE $name FAIL exit=$rc"; status=1; fi
}
build() {
  saccade_headroom || return 75
  if [[ -d "$CARGO_TARGET_DIR" ]] && (( $(du -sb "$CARGO_TARGET_DIR" | cut -f1) >= 9000000000 )); then
    echo 'raster target exceeds build admission ceiling'; return 75
  fi
  local subcommand="$1"; shift
  timeout 900 nice -n 19 cargo "$subcommand" -j 4 "$@"
}
features=$(python3 - <<'PY'
import tomllib
with open('crates/saccade/Cargo.toml','rb') as file:
    print(','.join(sorted(set(tomllib.load(file)['features'])-{'default','imgtune-avif'})))
PY
)
gate fmt cargo fmt --all -- --check
gate clippy-default build clippy --workspace --all-targets -- -D warnings
gate clippy-features build clippy --workspace --all-targets --features "$features" -- -D warnings
gate tests-extension build test -p saccade-geo
gate tests-cli-default build test -p saccade
gate tests-cli-features build test -p saccade --features "$features"
gate tests-core-schemas build test -p saccade-core --features schema --test schemas
gate tests-core-discovery build test -p saccade-core --lib schema_catalog
gate build-cli build build -p saccade --features "$features"
fixtures=$(mktemp -d)
trap 'rm -rf "$fixtures"' EXIT
gate fixtures build run -p saccade-geo --example fixtures -- "$fixtures"
gate acceptance python3 scripts/test-geo.py --saccade "$CARGO_TARGET_DIR/debug/saccade" --fixtures "$fixtures"
gate docs python3 scripts/gen-docs.py --check
gate public-hygiene bash scripts/check-public-hygiene.sh
gate genericity bash scripts/check-genericity.sh
gate package-list cargo package -p saccade-geo --list --allow-dirty --locked
gate packages python3 scripts/check-packages.py
exit "$status"
