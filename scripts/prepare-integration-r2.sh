#!/usr/bin/env bash
# Run through moss-heavy.sh; prepares assets and generated documents before final gates.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/integration-r2-env.sh
/mnt/linux-extra/saccade-models/venv/bin/python scripts/models/prepare-r2.py
python3 - <<'PYCHECK'
import json
from pathlib import Path
r=Path('/mnt/linux-extra/saccade-models/r2')
registry=json.loads((r/'registry.json').read_text())['contracts']
assert registry['dinov2-small']==json.loads((r/'model.json').read_text()),'export projection differs from shared registry'
assert registry['tesseract-fra']==json.loads((r/'tesseract-contract.json').read_text()),'OCR projection differs from shared registry'
PYCHECK
nice -n 19 cargo run -j 4 -p saccade-core --features credentials --example r2_credentials
UPDATE_SCHEMAS=1 nice -n 19 cargo test -j 4 -p saccade-core --all-features --test schemas committed_schemas_match_the_rust_types
UPDATE_WAVE7_SCHEMAS=1 nice -n 19 cargo test -j 4 -p saccade-core --all-features --lib wave7::schemas
nice -n 19 cargo test -j 4 -p saccade-core --lib compare::mask_mode_tests
nice -n 19 cargo build -j 4 -p saccade --all-features
python3 scripts/gen-docs.py --saccade "$CARGO_TARGET_DIR/debug/saccade" --skip-readme
