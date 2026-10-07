#!/usr/bin/env bash
# Document intake and correspondence proof; no providers, downloads or GPU.
set -uo pipefail
source scripts/gate-env.sh
features="$(python3 - <<'PY'
import tomllib
with open('crates/saccade/Cargo.toml','rb') as f:
    print(','.join(k for k in tomllib.load(f)['features'] if k not in ('default','imgtune-avif')))
PY
)"
status=0
gate() {
  local name="$1"; shift
  "$@"
  local code=$?
  printf 'GATE %s %s exit=%s\n' "$name" "$([[ $code = 0 ]] && echo PASS || echo FAIL)" "$code"
  [[ $code = 0 ]] || status=1
}
gate fmt cargo fmt --all --check
saccade_headroom || exit 1
gate clippy-default nice -n 19 cargo clippy -j 4 --workspace --all-targets -- -D warnings
saccade_headroom || exit 1
gate clippy-features nice -n 19 cargo clippy -j 4 --workspace --all-targets --features "$features" -- -D warnings
saccade_headroom || exit 1
gate tests nice -n 19 cargo test -j 4 -p saccade-core -p saccade --features "$features"
saccade_headroom || exit 1
gate documents-regression nice -n 19 cargo test -j 4 -p saccade --features "$features" --test wave6_contract generated_svg_pdf_inputs_require_real_rendering_and_page_summary -- --ignored
saccade_headroom || exit 1
gate documents-mcp nice -n 19 cargo test -j 4 -p saccade --features "$features" --test wave6_contract mcp_documents_preserve_roots_and_page_summary -- --ignored
saccade_headroom || exit 1
gate binary nice -n 19 cargo build -j 4 -p saccade --features "$features"
gate gen-docs python3 scripts/gen-docs.py --check
# Preserve the canonical all-features header while verifying regenerated help.
gate cli-reference python3 - "$CARGO_TARGET_DIR/debug/saccade" <<'PYCLI'
import importlib.util
import pathlib
import sys
spec = importlib.util.spec_from_file_location('gen_docs', 'scripts/gen-docs.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
actual = pathlib.Path('docs/cli.md').read_text().splitlines(keepends=True)
expected = module.generated(sys.argv[1], True)['docs/cli.md'].splitlines(keepends=True)
expected[4:8] = actual[4:8]
assert actual == expected, 'CLI help body differs from compiled operations'
assert '--all-features' in actual[4] and 'all-features binary' in actual[5]
print('Canonical header retained; generated CLI body matches')
PYCLI
gate public-hygiene bash scripts/check-public-hygiene.sh
gate guides python3 scripts/test-guides.py --bin "$CARGO_TARGET_DIR/debug/saccade"
exit "$status"
