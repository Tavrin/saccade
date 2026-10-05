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
assert heavy.count('#[ignore = "heavy: models"]') == 11
for name in ('pinned_detection_finds_generated_bottle', 'pinned_efficientsam_segments_generated_bottle',
             'pinned_faces_detect_generated_portrait_and_reject_blank', 'pinned_pair_metrics_identity_and_distortion'):
    assert f'#[ignore = "heavy: models"]\nfn {name}(' in heavy, name
manifest = json.loads((root / 'crates/saccade-core/assets/wave7-models.json').read_text())
assert {m['id'] for m in manifest['models']} == {'grounding-dino-tiny', 'owlv2-base', 'efficientsam-ti', 'yunet-2026may', 'ultraface-rfb', 'trustmark'}
assert all(m['parity_sha256'] is None for m in manifest['models'])
assert '#[ignore = "heavy: network"]' in heavy
assert 'wave7' in (root / 'integrations/agent-guide.md').read_text().lower()
print('wave7 documentation, schema identities and heavy-test inventory checked')

runtime = json.loads((root/'crates/saccade-core/assets/wave7-runtime.json').read_text())
assert runtime['version'] == '1.22.0' and runtime['license'] == 'MIT'
assert runtime['sha256'] == '8344d55f93d5bc5021ce342db50f62079daf39aaafb5d311a451846228be49b3'
assert 'models pull runtime' in text
assert json.loads((root/'scripts/wave7/disposition.json').read_text()) == json.loads((root/'crates/saccade-core/assets/wave7-disposition.json').read_text())
