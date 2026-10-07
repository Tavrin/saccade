#!/usr/bin/env bash
# Explicit provider-cost opt-in; never called by gates-wave4.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "${1:-}" == "--dry-run" ]]; then
    shift
    exec python3 scripts/assist/dry_run.py "$@"
fi
if [[ "${1:-}" == "--plan" ]]; then
    shift
    exec python3 scripts/assist/plan.py "$@"
fi
if [[ "${1:-}" == "--openrouter-stage2" ]]; then
    shift
    exec cargo run --locked -p saccade-core --features assist --example assist_openrouter_smoke -- --stage2 "$@"
fi
printf '%s\n' 'Paid qualification refused: verified provider billing ceilings are unavailable. Use --dry-run for fixture-only pipeline verification.' >&2
exit 4
