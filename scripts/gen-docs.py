#!/usr/bin/env python3
"""Generate agent packs, CLI help, schema links and showcase counts offline."""
import argparse
import json
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACK_HEADER = '<!-- Generated from integrations/agent-guide.md by scripts/gen-docs.py. -->\n'


def replace_section(text, name, content):
    start, end = f'<!-- {name}:start -->', f'<!-- {name}:end -->'
    before, rest = text.split(start, 1)
    _, after = rest.split(end, 1)
    return before + start + '\n' + content + '\n' + end + after


def generated(binary=None):
    guide = (ROOT / 'integrations/agent-guide.md').read_text(encoding="utf-8")
    packs = {
        'integrations/codex/AGENTS.saccade.md': PACK_HEADER + guide,
        'integrations/claude-code/skills/saccade/SKILL.md':
            '---\nname: saccade\ndescription: Measure visual changes and prepare human review within authorized scope.\n---\n\n' + PACK_HEADER + guide,
    }
    for name, body in packs.items():
        if len(body.encode()) > 4800:
            raise ValueError(f'{name}: exceeds estimated 1200-token budget (4800 UTF-8 bytes)')
    manifests = sorted((ROOT / 'showcases').glob('*/commands.json'))
    count = len(manifests)
    packs['README.md'] = replace_section((ROOT / 'README.md').read_text(encoding="utf-8"), 'showcase-count',
        f'[{count} reproducible cases](showcases/README.md) with commands, expected exits and measured output.\n'
        '[Pages gallery](https://tavrin.github.io/saccade/showcase/).')
    schemas = []
    for source in sorted((ROOT / 'schemas').glob('*.schema.json')):
        data = json.loads(source.read_text(encoding="utf-8"))
        schemas.append(f'- [{source.name}](../schemas/{source.name}) — {data.get("title", "Historical reader contract")}')
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
        operations = data['operations']
        lines = ['# Command reference', '', 'Generated from compiled capabilities and `--help`; do not edit by hand.', '',
            'Generation: `python3 scripts/gen-docs.py --saccade target/release/saccade`.',
            'Use the official default features plus `prechecks` to include every supported operation.', '',
            'Compiled features: ' + ', '.join(f'`{f}`' for f in data['features']) + '.', '',
            'Exit 1 means a failed measurement/evaluation gate or located divergence.',
            'Inspection, review, rank and ablation completion grant no acceptance authority.',
            'Exit 2 means the operation cannot run. Demo intentionally exits 1.', '']
        for op in [''] + operations:
            help_result = subprocess.run([binary] + op.split() + ['--help'], check=True, capture_output=True, text=True, encoding="utf-8")
            lines += [f'## saccade {op}'.rstrip(), '', '```text', help_result.stdout.rstrip(), '```', '']
        packs['docs/cli.md'] = '\n'.join(lines)
    return packs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--saccade', help='also generate the command reference from this binary')
    parser.add_argument('--check', action='store_true', help='reject drift without rewriting files')
    args = parser.parse_args()
    stale = []
    for name, body in generated(args.saccade).items():
        dest = ROOT / name
        if args.check:
            if not dest.is_file() or dest.read_text(encoding="utf-8") != body:
                stale.append(name)
        else:
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text(body, encoding="utf-8", newline="\n")
    if stale:
        parser.exit(1, 'Generated files differ: ' + ', '.join(stale) + '\n')
    print('Generated documentation matches.' if args.check else 'Generated documentation written.')


if __name__ == '__main__':
    main()
