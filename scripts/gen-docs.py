#!/usr/bin/env python3
"""Generate agent packs, CLI help, schema links and showcase counts offline."""
import argparse
import difflib
import itertools
import json
import re
import shutil
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACK_HEADER = '<!-- Generated from integrations/agent-guide.md by scripts/gen-docs.py. -->\n'


def replace_section(text, name, content):
    start, end = f'<!-- {name}:start -->', f'<!-- {name}:end -->'
    before, rest = text.split(start, 1)
    _, after = rest.split(end, 1)
    return before + start + '\n' + content + '\n' + end + after


def generated(binary=None, allow_missing_imgtune_avif=False):
    guide = (ROOT / 'integrations/agent-guide.md').read_text(encoding="utf-8")
    packs = {
        'integrations/codex/AGENTS.saccade.md': PACK_HEADER + guide,
        'integrations/claude-code/skills/saccade/SKILL.md':
            '---\nname: saccade\ndescription: Measure visual changes and prepare human review within authorized scope.\n---\n\n' + PACK_HEADER + guide,
    }
    packs['integrations/codex/skills/saccade/SKILL.md'] = packs[
        'integrations/claude-code/skills/saccade/SKILL.md']
    prompt = (ROOT / 'integrations/codex/check-visual-change.prompt.md').read_text(encoding="utf-8")
    packs['integrations/codex/skills/check-visual-change/SKILL.md'] = (
        '---\nname: check-visual-change\n'
        'description: Check supplied baseline and candidate captures with the installed Saccade CLI for visual, identity, or performance claims.\n---\n\n'
        '<!-- Generated from integrations/codex/check-visual-change.prompt.md by scripts/gen-docs.py. -->\n'
        + prompt)
    for name, body in packs.items():
        if len(body.encode()) > 4800:
            raise ValueError(f'{name}: exceeds estimated 1200-token budget (4800 UTF-8 bytes)')
    manifests = sorted((ROOT / 'showcases').glob('*/commands.json'))
    count = len(manifests)
    packs['README.md'] = replace_section((ROOT / 'README.md').read_text(encoding="utf-8"), 'showcase-count',
        f'[{count} reproducible cases](showcases/README.md) with commands, expected exits and measured output.\n'
        '[Pages gallery](https://tavrin.github.io/saccade/showcase/).')
    schemas = []
    for source in sorted((ROOT / 'crates').glob('*/schemas/*.schema.json')):
        data = json.loads(source.read_text(encoding="utf-8"))
        schemas.append(f'- [{source.name}](../{source.relative_to(ROOT).as_posix()}) — {data.get("title", "Historical reader contract")}')
    packs['docs/contracts.md'] = replace_section((ROOT / 'docs/contracts.md').read_text(encoding="utf-8"), 'schema-index', '\n'.join(schemas))
    lines = ['# Reproducible showcases', '', f'{count} cases discovered from `*/commands.json`.', '',
        'Generate with `python3 scripts/gen-showcases.py` and `python3 scripts/gen-photosensitivity.py`.',
        'Python 3, Pillow and numpy are required. Images are procedural; no imagery is downloaded.', '',
        'Validation requires `cargo build --release -p saccade --features prechecks`, with the binary on PATH.',
        '`scripts/run-showcases.sh` checks exits and reproduces each measured `EXPECTED.txt` byte for byte.',
        'Set `SACCADE_SHOWCASE_REPORTS` to choose the output directory. The default is `target/showcase-reports`.',
        'Changed expectations fail; the runner never accepts them automatically.', '',
        '| Case | Commands | Expected exits | Asset scope |', '| --- | --- | --- | --- |']
    for manifest in manifests:
        commands = json.loads(manifest.read_text(encoding="utf-8"))
        scope = 'Procedural; illustrative timings' if manifest.parent.name == 'perf-identity' else 'Procedural'
        if manifest.parent.name == 'photosensitivity':
            scope += '; experimental static previews'
        names = ', '.join('`' + c['name'] + '`' for c in commands)
        exits = ', '.join(str(c['exit']) for c in commands)
        lines.append(f'| [{manifest.parent.name}]({manifest.parent.name}/README.md) | {names} | {exits} | {scope} |')
    lines += ['', 'Bytes are stable for fixed Pillow/numpy versions; seeds and simulated timestamps are fixed.',
        'The runner does not qualify native platform installation, model quality or renderer timing.', '']
    packs['showcases/README.md'] = '\n'.join(lines)
    if binary:
        binary = shutil.which(binary) or str(Path(binary).resolve())
        result = subprocess.run([binary, 'inspect', 'capabilities', '--json'], check=True, capture_output=True, text=True, encoding="utf-8")
        data = json.loads(result.stdout)['data']
        # The family catalogue owns the complete compiled-feature inventory;
        # inspect capabilities supplies the recursively discovered CLI operations.
        catalogue = subprocess.run([binary, 'capabilities', '--json'], check=True, capture_output=True, text=True, encoding="utf-8")
        features = json.loads(catalogue.stdout)['compiled_features']
        manifest = tomllib.loads((ROOT / 'crates/saccade/Cargo.toml').read_text(encoding="utf-8"))
        missing = set(manifest['features']) - {'default'} - set(features)
        if allow_missing_imgtune_avif:
            missing.discard('imgtune-avif')
        if missing:
            raise ValueError('CLI reference requires an --all-features binary; missing: ' + ', '.join(sorted(missing)))
        operations = data['operations']
        lines = ['# Command reference', '', 'Generated from compiled capabilities and `--help`; do not edit by hand.', '',
            'Generation: `cargo build --release -p saccade --all-features`, then `python3 scripts/gen-docs.py --saccade target/release/saccade`.',
            'The all-features binary includes every supported operation.', '',
            'Compiled features: ' + ', '.join(f'`{f}`' for f in sorted(set(manifest['features']) - {'default'})) + '.', '',
            'Exit 1 means a failed image measurement/evaluation gate or located divergence.',
            'Exit 0 for compare/identity means no image regression; inspect `performance` for qualification.',
            'Inspection, review, rank and ablation completion grant no acceptance authority.',
            'Exit 2 means the operation cannot run. Demo intentionally exits 1.', '']
