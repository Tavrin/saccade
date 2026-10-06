#!/usr/bin/env python3
"""Offline Wave 10 discovery, field contracts and pack-budget checks."""
from pathlib import Path
import json
import tomllib
root = Path(__file__).resolve().parents[1]
version = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']['version']
checks = {
    'docs/render-evidence.md': ['whole_frame_unmasked', '--require-scope', '--mask-dump',
        '--export-maps', '--noise-from', 'noise build', 'p95', 'lower confidence',
        'RGB RMS', 'saccade-error-maps.v1', 'per_id'],
    'docs/arm-validity.md': ['--allow-unreached', 'unreached_policy', 'record_files',
        'null', 'baseline_state', 'preparer'],
    'docs/identity-and-performance.md': ['schema list', 'schema get', 'perf validate'],
    'docs/captures.md': ['schema get', 'perf validate'],
    'docs/python.md': ['__version__', 'compare_maps', 'NumPy'],
    'docs/wave7.md': ['fix_command', 'doctor', 'before reading pixels'],
    'CHANGELOG.md': [f'## {version}\n'],
}
assert tomllib.loads((root / 'crates/saccade-py/pyproject.toml').read_text())['project']['version'] == version
server = json.loads((root / 'server.json').read_text())
assert server['version'] == version
assert all(package['version'] == version for package in server['packages'])
for file, terms in checks.items():
    text = (root / file).read_text()
    for term in terms:
        assert term in text, f'{file}: missing {term}'
for file in ['integrations/agent-guide.md', 'integrations/codex/AGENTS.saccade.md',
             'integrations/codex/skills/saccade/SKILL.md',
             'integrations/claude-code/skills/saccade/SKILL.md']:
    assert (root / file).stat().st_size <= 4800, f'{file}: pack budget'
for id in ['saccade-render-evidence.v1', 'saccade-repeat-noise.v1', 'saccade-error-maps.v1']:
    file = root / f'crates/saccade-core/schemas/{id}.schema.json'
    assert json.loads(file.read_text())['type'] == 'object'
    assert file.name in (root / 'docs/contracts.md').read_text()
print('wave10 documentation and pack budgets: PASS')
