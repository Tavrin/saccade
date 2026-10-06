#!/usr/bin/env bash
# Portable defaults for contributor gates. Source from the repository root.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target}"
export SACCADE_MODEL_CACHE="${SACCADE_MODEL_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/saccade/models}"
saccade_headroom() {
  python3 - "$CARGO_TARGET_DIR" <<'PY'
import pathlib, shutil, sys
path = pathlib.Path(sys.argv[1]).resolve()
while not path.exists():
    path = path.parent
if shutil.disk_usage(path).free < 25 * 1024**3:
    sys.exit('build admission refused: need 25 GiB free on target filesystem')
PY
}
saccade_run() {
  if declare -F "$1" >/dev/null; then
    "$@"
  elif [[ -n "${SACCADE_HEAVY_WRAPPER:-}" ]]; then
    "$SACCADE_HEAVY_WRAPPER" "$@"
  else
    "$@"
  fi
}
