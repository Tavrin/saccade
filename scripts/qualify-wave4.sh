#!/usr/bin/env bash
# Explicit provider-cost opt-in; never called by gates-wave4.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
max_spend=''; corpus=''; output=''; receipt=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --max-spend-usd) max_spend="${2:?missing spend cap}"; shift 2;;
        --corpus) corpus="${2:?missing frozen corpus}"; shift 2;;
        --gate-receipt) receipt="${2:?missing heavy gate receipt}"; shift 2;;
        --out) output="${2:?missing output directory}"; shift 2;;
        *) printf '%s\n' 'Usage: qualify-wave4.sh --max-spend-usd N --corpus DIR --out DIR --gate-receipt FILE' >&2; exit 4;;
    esac
done
[[ -n "$max_spend" && -n "$corpus" && -n "$output" && -n "$receipt" ]] || { printf '%s\n' 'A hard spend cap and frozen corpus/output are required.' >&2; exit 4; }
python3 - "$max_spend" <<'PY'
import math,sys
cap=float(sys.argv[1])
if not math.isfinite(cap) or cap <= 0 or cap > 250: raise SystemExit('Spend cap must be in (0,250] USD; no automatic top-up.')
PY
python3 scripts/assist/corpus.py --out "$corpus" --verify
python3 - "$receipt" <<'PYRECEIPT'
import sys
from pathlib import Path
sys.path.insert(0,'scripts/assist')
from score import verify_gate_receipt
verify_gate_receipt(Path(sys.argv[1]))
PYRECEIPT
[[ ! -e "$output" ]] || { printf '%s\n' 'Qualification output must be new; never silently retry a frozen epoch.' >&2; exit 4; }
mkdir -p "$output"
available=$(df -BG --output=avail /mnt/linux-extra | tail -n 1 | tr -dc '0-9')
[[ -n "$available" && "$available" -ge 25 ]] || { printf '%s\n' 'Below 25 GB free: provider runner build refused.' >&2; exit 4; }
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/mnt/linux-extra/moss-cargo-targets/codex-saccade-w4}"
# All payloads use the existing authorization, egress, attempt and monetary ledgers.
# No Python HTTP, ambient credentials, cache reuse, fallback models or retries.
nice -n 19 cargo run -j 4 -p saccade-core --features assist --example assist_qualify -- \
    --manifest "$corpus/manifest.json" --out "$output/results.jsonl" \
    --max-spend-usd "$max_spend" --run true
python3 scripts/assist/score.py --corpus "$corpus" --results "$output/results.jsonl" --gate-receipt "$receipt" --out "$output/saccade-constructed-truth.v1.json"
