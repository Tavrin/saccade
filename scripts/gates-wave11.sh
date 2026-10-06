#!/usr/bin/env bash
# Coordinator-only complete gates; CPU analysis, generated fixtures, no downloads/GPU.
set -u
cd "$(dirname "$0")/.." || exit 1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/mnt/linux-extra/moss-cargo-targets/codex-saccade-w11}"
source scripts/gate-env.sh
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 MOSS_HEAVY_GPU=0
failed=0
# Coordinator disk-emergency amendment requires 25 GiB free and a target below 5 GiB.
headroom() {
  python3 - "$CARGO_TARGET_DIR" <<'PY'
import pathlib, shutil, sys
path = pathlib.Path(sys.argv[1]).resolve()
while not path.exists(): path = path.parent
size = sum(p.stat().st_size for p in pathlib.Path(sys.argv[1]).rglob('*') if p.is_file() and not p.is_symlink())
if size >= 5 * 1024**3:
    sys.exit('wave11 admission refused: target reached 5 GiB cap')
if shutil.disk_usage(path).free < 25 * 1024**3:
    sys.exit('wave11 admission refused: need 25 GiB free')
PY
}
gate() {
  local name=$1; shift
  if saccade_run timeout --signal=TERM --kill-after=10 840 "$@"; then
    printf 'GATE %s PASS\n' "$name"
  else
    printf 'GATE %s FAIL\n' "$name"; failed=1
  fi
}
cargo_gate() {
  local name=$1; shift
  if ! headroom; then printf 'GATE %s FAIL (disk-headroom)\n' "$name"; failed=1; return; fi
  gate "$name" nice -n 19 cargo "$@"
}
features=graphics,mcp,schema,ai,workbench
gate fmt cargo fmt --all -- --check
gate docs python3 scripts/check-wave11-docs.py
gate genericity bash scripts/check-genericity.sh
cargo_gate core-clippy clippy -j4 -p saccade-core --no-default-features --features graphics,schema,ai --all-targets -- -D warnings
cargo_gate cli-clippy clippy -j4 -p saccade --no-default-features --features "$features" --all-targets -- -D warnings
cargo_gate minimal-core-clippy clippy -j4 -p saccade-core --no-default-features --all-targets -- -D warnings
cargo_gate full-core-tests test -j4 -p saccade-core --no-default-features --features graphics,schema,ai
cargo_gate full-cli-tests test -j4 -p saccade --no-default-features --features "$features"
cargo_gate minimal-core-tests test -j4 -p saccade-core --no-default-features
cargo_gate wave11-cli test -j4 -p saccade --no-default-features --features "$features" --test wave11 -- --ignored
cargo_gate null-peeking test -j4 -p saccade-core --no-default-features --features graphics,schema,ai --lib repeated_peeking_null_familywise_directional_rate_is_bounded -- --ignored --nocapture
cargo_gate schemas test -j4 -p saccade-core --no-default-features --features graphics,schema,ai --test schemas
if test -n "${SACCADE_W11_REAL_INVENTORY:-}"; then
  cargo_gate real-record test -j4 -p saccade-core --no-default-features --features graphics,schema,ai --lib ablation_timing::external_inventory_tests -- --ignored
else
  printf 'GATE real-record DEFERRED (set SACCADE_W11_REAL_INVENTORY to an external read-only inventory)\n'
fi
exit "$failed"
