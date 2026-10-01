#!/usr/bin/env bash
# Regenerate datasets, execute their documented commands and compare real stdout.
set -euo pipefail
TASK_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$TASK_ROOT"
command -v saccade >/dev/null || { echo 'Put the release saccade binary on PATH.' >&2; exit 2; }
python3 scripts/gen-showcases.py
python3 - <<'PY'
import difflib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path.cwd()
reports = Path('/mnt/linux-extra/moss-cargo-targets/saccade-showcase-reports')
reports.mkdir(parents=True, exist_ok=True)
env = dict(os.environ, LC_ALL='C', NO_COLOR='1')
failed = False
for manifest in sorted((root / 'showcases').glob('*/commands.json')):
    case = manifest.parent
    out = reports / case.name
    out.mkdir(parents=True, exist_ok=True)
    transcript = []
    for command in json.loads(manifest.read_text()):
        args = ['saccade'] + [a.replace('@REPORTS@', str(out)) for a in command['args']]
        if command.get('out'):
            args += ['--out', str(out / command['out'])]
        result = subprocess.run(args, cwd=case, env=env, capture_output=True, text=True)
        transcript += [f"=== {command['name']} (exit {result.returncode}) ===\n", result.stdout]
        (out / (command['name'] + '.stderr.txt')).write_text(result.stderr)
        if result.returncode != command['exit']:
            print(f"{case.name}/{command['name']}: expected exit {command['exit']}, got {result.returncode}", file=sys.stderr)
            print(result.stderr, file=sys.stderr)
            failed = True
    actual = ''.join(transcript)
    (out / 'ACTUAL.txt').write_text(actual)
    expected_path = case / 'EXPECTED.txt'
    if not expected_path.exists():
        print(f'{case.name}: missing EXPECTED.txt; actual output is {out / "ACTUAL.txt"}', file=sys.stderr)
        failed = True
    elif expected_path.read_text() != actual:
        print(''.join(difflib.unified_diff(expected_path.read_text().splitlines(True),
                     actual.splitlines(True), fromfile=str(expected_path), tofile=str(out / 'ACTUAL.txt'))), file=sys.stderr)
        failed = True
    else:
        print(f'{case.name}: reproduced EXPECTED.txt')
sys.exit(1 if failed else 0)
PY
