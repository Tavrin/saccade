#!/usr/bin/env python3
"""Offline contract, discovery and agent-pack checks for wave11."""
from pathlib import Path
import json
root = Path(__file__).resolve().parents[1]
text = (root/'docs/experiments-wave11.md').read_text()
for term in ['timing ab', 'hyperfine', 'CSV', 'JSON path', 'same-session', 'diagnostic-only',
             'Bonferroni', 'experiment settle', 'ghosting', 'ablate', 'IQR', 'No CI',
             '--mask-layer', '--mask-from-dump', '--require-effect', '--source-ref',
             'index export', 'subtrees', 'mapped_keys', 'parse_layer_spec', 'parse_effect_spec', 'Measurement::report_identity', 'migration', '--max-record-bytes', 'max_record_bytes', '64 MiB']:
    assert term.casefold() in text.casefold(), term
assert '## Unreleased' in (root/'CHANGELOG.md').read_text()
for name in ['saccade-timing-session.v1', 'saccade-timing-ab.v1', 'saccade-settling.v1', 'saccade-report-index-row.v1']:
    path = root/f'crates/saccade-core/schemas/{name}.schema.json'
    schema = json.loads(path.read_text())
    assert schema['type'] == 'object'
    assert schema['properties']['schema']['const'] == name
    assert path.name in (root/'docs/contracts.md').read_text()
for name in ['integrations/agent-guide.md', 'integrations/codex/AGENTS.saccade.md',
             'integrations/codex/skills/saccade/SKILL.md',
             'integrations/claude-code/skills/saccade/SKILL.md']:
    assert (root/name).stat().st_size <= 4800, f'{name}: pack budget'
assert 'experiments-wave11.md' in (root/'integrations/agent-guide.md').read_text()
print('wave11 documentation, schema discovery and pack budgets: PASS')
