#!/usr/bin/env python3
"""Cheap contract/docs/gating checks, never model or external provider execution."""
import ast
import json
from pathlib import Path
import re
import tomllib

root = Path(__file__).resolve().parents[2]
for filename in ['docs/media.md', 'docs/python.md', 'docs/api.md', 'docs/api-openapi.json',
                 'crates/saccade-py/python/saccade/__init__.pyi', 'Dockerfile.wave8',
                 '.github/workflows/python-wheels.yml', 'scripts/gates-wave8.sh']:
    assert (root / filename).is_file(), filename
ast.parse((root / 'crates/saccade-py/python/saccade/__init__.pyi').read_text())
manifest = tomllib.loads((root / 'crates/saccade-py/Cargo.toml').read_text())
assert 'abi3-py310' in manifest['dependencies']['pyo3']['features']
workflow = (root / '.github/workflows/python-wheels.yml').read_text()
assert 'aarch64' in workflow and 'x86_64' in workflow and 'upload-artifact' in workflow
assert not re.search(r'(?i)(pypi|twine|maturin publish)', workflow)
api = json.loads((root / 'docs/api-openapi.json').read_text())
assert set(api['paths']) == {'/v1/health', '/v1/analyze-media', '/v1/compare', '/v1/search'}
for file in (root / 'crates/saccade-core/schemas').glob('*.json'):
    value = json.loads(file.read_text())
    assert value.get('$schema') and value.get('$id'), file
model = json.loads((root / 'crates/saccade-core/assets/wave8-model-disposition.json').read_text())
assert model['text_image']['status'] == 'deferred' and model['text_image']['calibration'] == 'uncalibrated'
assert (root / 'crates/saccade-py/python/saccade/py.typed').is_file()
print('Wave 8 docs, schemas, stubs, offline model disposition and artifact-only CI: PASS')
