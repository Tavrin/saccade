#!/usr/bin/env bash
# Run the composite action's steps locally against examples/, without GitHub.
# Builds the CLI, compares, writes the Markdown summary to a temp
# GITHUB_STEP_SUMMARY and prints the exit code. Upload and PR-comment steps
# are skipped.
#
# Usage: scripts/action-dry-run.sh

set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root" || exit 2

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export GITHUB_STEP_SUMMARY="$tmp/step-summary.md"
: > "$GITHUB_STEP_SUMMARY"
report_dir="$tmp/saccade-report"

echo "== step 1: install (cargo build -p saccade)"
if ! cargo build -p saccade; then
  echo "build failed" >&2
  exit 2
fi
# `cargo run` finds the binary wherever the target directory is configured.
saccade() { cargo run -q -p saccade -- "$@"; }

echo "== step 2: saccade compare"
saccade compare examples/baseline examples/capture --out "$report_dir"
code=$?

echo "== step 3: upload artifact (skipped: needs GitHub)"

echo "== step 4: job summary"
if [ -f "$report_dir/saccade-report.v1.json" ]; then
  saccade inspect export "$report_dir/saccade-report.v1.json" --format markdown --out "$GITHUB_STEP_SUMMARY" \
    --artifact-url "file://$report_dir/index.html"
  cat "$GITHUB_STEP_SUMMARY"
else
  echo "no report written"
fi

echo "== step 5: PR comment (skipped: needs GitHub)"

echo "== step 6: exit code"
echo "saccade compare exit code: $code (expected 1: examples/ holds a regression and a missing image)"
exit "$code"
