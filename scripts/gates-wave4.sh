#!/usr/bin/env bash
# Explicit feature qualification gates; see docs/releasing.md.
set -u -o pipefail
cd "$(dirname "$0")/.." || exit 2
source scripts/gate-env.sh
failed=0
gate() {
    local name="$1"; shift
    if saccade_run "$@"; then printf 'GATE %s PASS\n' "$name"; else printf 'GATE %s FAIL\n' "$name"; failed=1; fi
}
preflight() { saccade_headroom; }
cargo_gate() { preflight && saccade_run nice -n 19 cargo "$@"; }
gate fmt cargo fmt --all -- --check
gate check cargo_gate check -j 4 --workspace --all-targets --features assist,schema
gate clippy cargo_gate clippy -j 4 --workspace --all-targets --features assist,schema -- -D warnings
gate core-tests cargo_gate test -j 4 -p saccade-core --features assist,schema
gate cli-tests cargo_gate test -j 4 -p saccade --features assist,schema
gate wave4-heavy-core cargo_gate test -j 4 -p saccade-core --features assist,schema -- --ignored wave4
gate wave4-heavy-cli cargo_gate test -j 4 -p saccade --features assist,schema --test assist_contract -- --ignored
gate wave4-batch-routing cargo_gate test -j 4 -p saccade --features assist,schema --test assist_batch_contract -- --ignored
gate qualification-preflight python3 scripts/assist/test_preflight.py
gate constructed-python python3 scripts/assist/test_constructed.py
gate docs python3 scripts/assist/check_docs.py
gate shell bash -n scripts/gates-wave4.sh scripts/qualify-wave4.sh
if [[ "$failed" -eq 0 ]]; then
    python3 - "${SACCADE_WAVE4_GATE_RECEIPT:-$CARGO_TARGET_DIR/gates-wave4-receipt.json}" <<'PYRECEIPT'
import sys
from pathlib import Path
sys.path.insert(0,'scripts/assist')
from corpus import gate_source_hash,put
from policy import GATES
path=Path(sys.argv[1]);path.parent.mkdir(parents=True,exist_ok=True)
put(path,{"schema":"saccade-assist-gates.v1","source_hash":gate_source_hash(),"gates":{g:"PASS" for g in GATES}})
PYRECEIPT
fi
exit "$failed"
