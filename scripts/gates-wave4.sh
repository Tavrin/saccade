#!/usr/bin/env bash
# Explicit feature qualification gates; see docs/releasing.md.
set -u -o pipefail
cd "$(dirname "$0")/.." || exit 2
source scripts/gate-env.sh
failed=0
source_before=$(python3 -c 'import sys;sys.path.insert(0,"scripts/assist");from corpus import gate_source_hash;print(gate_source_hash())') || exit 1
counts=$(mktemp) || exit 1
trap 'rm -f "$counts"' EXIT
gate() {
    local name="$1" log status; shift
    log=$(mktemp) || { failed=1; return; }
    saccade_run "$@" 2>&1 | tee "$log"
    status=${PIPESTATUS[0]}
    case "$name" in
        core-tests|cli-tests|wave4-heavy-cli|wave4-batch-routing)
            if [[ "$status" -eq 0 ]]; then
                python3 - "$name" "$log" "$counts" <<'PYCOUNT'
import json,re,sys
from pathlib import Path
n=sum(int(m) for m in re.findall(r'test result: ok\. (\d+) passed;',Path(sys.argv[2]).read_text()))
if not n: raise SystemExit('empty test gate refused')
with open(sys.argv[3],'a') as f: f.write(json.dumps({sys.argv[1]:n})+'\n')
PYCOUNT
                status=$?
            fi;;
    esac
    rm -f "$log"
    if [[ "$status" -eq 0 ]]; then printf 'GATE %s PASS\n' "$name"; else printf 'GATE %s FAIL\n' "$name"; failed=1; fi
}
preflight() { saccade_headroom; }
cargo_gate() { preflight && saccade_run nice -n 19 cargo "$@"; }
gate fmt cargo fmt --all -- --check
gate check cargo_gate check -j 4 --workspace --all-targets --features assist,schema
gate clippy cargo_gate clippy -j 4 --workspace --all-targets --features assist,schema -- -D warnings
gate core-tests cargo_gate test -j 4 -p saccade-core --features assist,schema
gate cli-tests cargo_gate test -j 4 -p saccade --features assist,schema
gate wave4-heavy-cli cargo_gate test -j 4 -p saccade --features assist,schema --test assist_contract -- --ignored
gate wave4-batch-routing cargo_gate test -j 4 -p saccade --features assist,schema --test assist_batch_contract -- --ignored
gate qualification-preflight python3 scripts/assist/test_preflight.py
gate constructed-python python3 scripts/assist/test_constructed.py
# Includes passing metric fixtures, failure mutations and separately persisted receipts.
gate g12-regressions python3 scripts/assist/test_g12.py
gate docs python3 scripts/assist/check_docs.py
gate shell bash -n scripts/gates-wave4.sh scripts/qualify-wave4.sh
if [[ "$failed" -eq 0 ]]; then
    python3 - "${SACCADE_WAVE4_GATE_RECEIPT:-$CARGO_TARGET_DIR/gates-wave4-receipt.json}" "$source_before" "$counts" "$CARGO_TARGET_DIR/debug/saccade" <<'PYRECEIPT'
import sys
from pathlib import Path
import json,os,subprocess
sys.path.insert(0,'scripts/assist')
from corpus import gate_source_hash,put,digest
if gate_source_hash()!=sys.argv[2]: raise SystemExit("sources changed during gates")
from policy import GATES
path=Path(sys.argv[1]);path.parent.mkdir(parents=True,exist_ok=True)
counts={}
for line in Path(sys.argv[3]).read_text().splitlines(): counts.update(json.loads(line))
required={"core-tests","cli-tests","wave4-heavy-cli","wave4-batch-routing"}
if set(counts)!=required or not all(counts.values()): raise SystemExit("missing meaningful test counts")
binary=Path(sys.argv[4]).resolve()
put(path,{"schema":"saccade-assist-gates.v1","source_hash":gate_source_hash(),"gates":{g:"PASS" for g in GATES},
          "execution":{"source_before":sys.argv[2],"test_counts":counts,"binary":str(binary),"binary_hash":digest(binary.read_bytes()),
                       "toolchain":subprocess.check_output(["rustc","-Vv"],text=True),"features":"assist,schema",
                       "build_env":{k:os.environ.get(k) for k in ("CARGO_INCREMENTAL","CARGO_PROFILE_DEV_DEBUG","RUSTFLAGS","CARGO_BUILD_TARGET")}}})
PYRECEIPT
    [[ $? -eq 0 ]] || failed=1
fi
exit "$failed"
