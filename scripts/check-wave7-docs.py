#!/usr/bin/env python3
"""Light documentation/schema/ignored-test coverage check. No network or models."""
import json
from pathlib import Path
root = Path(__file__).resolve().parent.parent
text = (root / 'docs/wave7.md').read_text()
for command in ('models list', 'models pull', 'locate', 'observe-local', 'quality-score',
                'watermark', 'faces', 'crop-check', 'provider-map'):
    assert command in text, f'missing command docs: {command}'
for name in ('model-registry', 'model-status', 'locate', 'vision-observation',
             'learned-quality', 'watermark', 'faces', 'crop-check', 'provider-mapping'):
    path = root / f'crates/saccade-core/schemas/saccade-{name}.v1.schema.json'
    value = json.loads(path.read_text())
    assert value['$id'].endswith(path.name), path
    assert value['properties']['schema']['const'] == f'saccade-{name}.v1', path
heavy = (root / 'crates/saccade-core/src/wave7/heavy_tests.rs').read_text()
assert heavy.count('#[ignore = "heavy: models"]') == 5
assert '#[ignore = "heavy: network"]' in heavy
assert 'wave7' in (root / 'integrations/agent-guide.md').read_text().lower()
print('wave7 documentation, schema identities and heavy-test inventory checked')
