#!/usr/bin/env bash
# Local, exact-revision release gates. No publishing or Git state changes.
# Text-quality fixtures require Python 3 with Pillow and installed Linux fonts:
# apt-get install fonts-dejavu-core fonts-noto-cjk (DejaVu covers Latin/Arabic;
# Noto Sans CJK covers CJK). Missing fonts fail the all-features tests.
set -uo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 2
export CARGO_BUILD_RUSTC_WRAPPER='' RUSTC_WRAPPER=''
source scripts/gate-env.sh
failed=0
run() {
  local name="$1"
  shift
  local -a command=("$@")
  if ! saccade_headroom; then
    echo "FAIL $name (less than 25 GiB free; not executed)"
    failed=1
    return 75
  fi
  if saccade_run timeout 900 "${command[@]}"; then echo "PASS $name"; return 0; else echo "FAIL $name"; failed=1; return 1; fi
}
run public-hygiene scripts/check-public-hygiene.sh
run public-hygiene-tests python3 scripts/test-public-hygiene.py
run genericity scripts/check-genericity.sh
run renderdoc-worker-boundaries python3 scripts/test-renderdoc-worker.py
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
run release-archive-notices python3 scripts/test-release-notices.py
run bundle-config python3 scripts/test-bundles.py
run packaged-readme-tests python3 scripts/test-check-packages.py
run package-inventory-and-readmes python3 scripts/check-packages.py
run packaged-workspace cargo package --workspace --all-features --allow-dirty --locked
notices="$(mktemp)"
run dependency-notices python3 scripts/generate-third-party-notices.py "$notices"
rm -f "$notices"
if command -v actionlint >/dev/null 2>&1; then
  run actionlint actionlint .github/workflows/*.yml
else
  echo 'FAIL actionlint (install actionlint v1.7.7)'
  failed=1
fi
run shellcheck shellcheck scripts/gate-env.sh scripts/release-check.sh scripts/run-showcases.sh
run showcase-binary cargo build --release -p saccade --all-features --locked
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
  'MANUAL downstream consumer integration: consumer qualification' \
  'MANUAL live pilot publication: separately authorized evaluation, never ordinary CI'
exit "$failed"
