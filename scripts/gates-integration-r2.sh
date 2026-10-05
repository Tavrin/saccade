#!/usr/bin/env bash
# All required round 2 gates in one shared admitted batch; no live providers.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/integration-r2-env.sh
if test "$#" -ne 1; then echo 'usage: gates-integration-r2.sh EVIDENCE_DIRECTORY' >&2; exit 2; fi
if test "$(git branch --show-current)" != integ/waves-4-6; then echo 'wrong integration branch' >&2; exit 2; fi
if test -n "$(git status --porcelain)"; then echo 'commit generated outputs before final qualification' >&2; exit 2; fi
exec python3 scripts/gates-integration.py --round2 --evidence "$1"
