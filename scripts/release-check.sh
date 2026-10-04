#!/usr/bin/env bash
# Local, exact-revision release gates. No publishing or Git state changes.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 2
export CARGO_BUILD_RUSTC_WRAPPER='' RUSTC_WRAPPER=''
failed=0
run() {
  local name="$1"
  shift
  if "$@"; then echo "PASS $name"; return 0; else echo "FAIL $name"; failed=1; return 1; fi
}
run formatting cargo fmt --check
run clippy cargo clippy --workspace --all-targets --locked -- -D warnings
if run workspace-tests cargo test --workspace --locked; then
  echo 'PASS native-depth identity, strict metadata and complete pairing (workspace tests)'
  echo 'PASS hash-bound approvals, workbench attestation, stale inputs and legacy model decisions (workspace tests)'
  echo 'PASS MCP containment, pagination, response limits and next actions (workspace tests)'
  echo 'PASS provider no-egress, endpoint isolation, concurrent budgets and failure replay (workspace tests)'
else
  echo 'FAIL native-depth/approval/MCP/provider contract groups (workspace tests)'
fi
run core-no-default-build cargo build -p saccade-core --no-default-features --locked
run core-no-default-tests cargo test -p saccade-core --no-default-features --locked
run cli-default-build cargo build -p saccade --locked
run cli-default-tests cargo test -p saccade --locked
run all-features-build cargo build -p saccade --all-features --locked
run all-features-tests cargo test -p saccade --all-features --locked
run historical-readers cargo test -p saccade-core --test evidence_contracts --locked historical_
run package-inventory python3 scripts/check-packages.py
notices="$(mktemp)"
run dependency-notices python3 scripts/generate-third-party-notices.py "$notices"
rm -f "$notices"
if command -v actionlint >/dev/null 2>&1; then
  run actionlint actionlint .github/workflows/*.yml
else
  echo 'FAIL actionlint (install actionlint v1.7.7)'
  failed=1
fi
run shellcheck shellcheck scripts/release-check.sh scripts/run-showcases.sh
run showcase-binary cargo build --release -p saccade --features prechecks --locked
target_dir="$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
binary="$target_dir/release/saccade"
export PATH="$target_dir/release:$PATH"
export SACCADE_SHOWCASE_REPORTS="$target_dir/showcase-reports"
run showcase-expected bash scripts/run-showcases.sh
run showcase-schemas python3 scripts/validate-showcase-schemas.py "$SACCADE_SHOWCASE_REPORTS"
run docs-smoke python3 scripts/gen-docs.py --saccade "$binary" --check
printf '%s\n' \
  'CI-ONLY clean-machine installation and demo on four targets: qualify.yml' \
  'CI-ONLY fork PR and trusted baseline-update PR demonstration: example-usage.yml, example-update-baselines.yml' \
  'CI-ONLY tagged archive hash and build identity: release.yml, qualify.yml' \
  'MANUAL narrow/wide dark/light browser and offline relocation: release checklist' \
  'MANUAL current Moss consumer and preserved integration evidence: upstream Moss qualification' \
  'MANUAL live pilot publication: separately authorized evaluation, never ordinary CI'
exit "$failed"
