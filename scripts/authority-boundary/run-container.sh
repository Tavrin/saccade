#!/usr/bin/env bash
# Runtime has loopback only, no host credentials, no capabilities or Docker socket.
set -euo pipefail
if [[ $# -lt 2 ]]; then
  echo 'usage: run-container.sh BINARY REPORT_DIR [--allow-known-findings]' >&2
  exit 2
fi
binary=$(realpath "$1")
mkdir -p "$2"
reports=$(realpath "$2")
shift 2
script_dir=$(cd -- "$(dirname -- "$0")" && pwd)
context=$(mktemp -d "${TMPDIR:-/tmp}/authority-container.XXXXXX")
trap 'rm -rf -- "$context"' EXIT
cp -- "$binary" "$context/saccade"
cp -- "$script_dir/harness.py" "$script_dir/Dockerfile" "$context/"
python3 - "$script_dir/harness.py" "$context/protected-baseline/sample.png" <<'PY'
import importlib.util
from pathlib import Path
import sys
spec = importlib.util.spec_from_file_location('authority', sys.argv[1])
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)
harness.png(Path(sys.argv[2]), 20)
PY
image="saccade-authority:local"
docker build --network none --tag "$image" "$context"
# Caller owns the report mount. Do not chown it or grant a root container access.
docker run --rm --network none --read-only --cap-drop ALL \
  --security-opt no-new-privileges --pids-limit 128 --memory 512m --cpus 2 \
  --user "$(id -u):$(id -g)" --tmpfs /tmp:rw,nosuid,nodev,size=128m,mode=1777 \
  --mount "type=bind,src=$reports,dst=/reports" \
  --mount "type=bind,src=$context/protected-baseline,dst=/protected-baseline,readonly" \
  "$image" --report /reports/authority.v1.json --protected-baseline /protected-baseline "$@"
