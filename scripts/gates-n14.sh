#!/usr/bin/env bash
# N14 lane gates: assigned disk target, bounded jobs; no models/providers/GPU.
set -u
cd "$(dirname "$0")/.."
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
source scripts/gate-env.sh
receipts="${SACCADE_GATE_RECEIPTS:-$CARGO_TARGET_DIR/n14-receipts}"
mkdir -p "$receipts"
export TMPDIR="${SACCADE_GATE_TMPDIR:-$receipts/tmp}"
mkdir -p "$TMPDIR"
status=0
gate() {
  local name="$1"; shift
  "$@" > "$receipts/$name.log" 2>&1
  local rc=$?
  last_rc=$rc
  echo "GATE $name exit=$rc"
  if ((rc != 0)); then tail -30 "$receipts/$name.log"; status=1; fi
}
build() {
  saccade_headroom || return 75
  if [[ -d "$CARGO_TARGET_DIR" ]] && (( $(du -sb "$CARGO_TARGET_DIR" | cut -f1) >= 9000000000 )); then
    echo 'N14 target approaching 10 GB; refuse build'; return 75
  fi
  local operation="$1"; shift
  timeout 900 nice -n 19 cargo "$operation" --locked -j 4 "$@"
}
prune_test_executables() {
  # Preserve libraries/receipts while removing completed test binaries for headroom.
  python3 - "$CARGO_TARGET_DIR" "$1" <<'PYTHON'
from pathlib import Path
import re, sys
base = Path(sys.argv[1]) / 'debug/deps'
log = Path(sys.argv[2]).read_text()
removed = 0
for name in re.findall(r'Running .* \(([^)]+)\)', log):
    path = Path(name)
    if path.parent == base and path.is_file():
        path.unlink()
        removed += 1
print(f'Removed {removed} completed test executables; libraries and receipts retained.')
PYTHON
}
features=$(python3 - <<'PY'
import tomllib
from pathlib import Path
print(','.join(sorted(set(tomllib.loads(Path('crates/saccade/Cargo.toml').read_text())['features']) - {'default','imgtune-avif'})))
PY
)
gate fmt cargo fmt --all -- --check
gate clippy-default build clippy --workspace --all-targets -- -D warnings
gate clippy-features build clippy --workspace --all-targets --features "$features" -- -D warnings
gate build-docs build build -p saccade --features "$features"
if ((last_rc != 0)); then
  echo "Dependent gates skipped: fresh binary build did not pass"
  exit 1
fi
# Spec requires the canonical all-features header, even on the AVIF-excepted host.
gate regenerate-docs python3 scripts/gen-docs.py --saccade "$CARGO_TARGET_DIR/debug/saccade" --allow-missing-imgtune-avif
python3 - <<'PY'
from pathlib import Path
import subprocess
p=Path('docs/cli.md'); lines=p.read_text().splitlines(keepends=True)
main=subprocess.check_output(['git','show','HEAD:docs/cli.md']).decode().splitlines(keepends=True)
lines[4:8]=main[4:8];p.write_text(''.join(lines))
PY
gate gen-docs python3 scripts/gen-docs.py --check
gate tests-core-default build test -p saccade-core
gate prune-core-default prune_test_executables "$receipts/tests-core-default.log"
gate sensitivity-proof python3 scripts/sensitivity/fixtures.py --out "$receipts/inputs" --binary "$CARGO_TARGET_DIR/debug/saccade" --receipts "$receipts/proof"
gate tests-cli-default build test -p saccade
gate prune-cli-default prune_test_executables "$receipts/tests-cli-default.log"
gate tests-core-features build test -p saccade-core --all-features
gate prune-core-features prune_test_executables "$receipts/tests-core-features.log"
gate tests-cli-features build test -p saccade --features "$features"
gate prune-cli-features prune_test_executables "$receipts/tests-cli-features.log"
gate public-hygiene bash scripts/check-public-hygiene.sh
gate guides python3 scripts/test-guides.py --bin "$CARGO_TARGET_DIR/debug/saccade"
exit "$status"
