#!/usr/bin/env python3
"""Keep bundles.json, release.yml, Cargo features and the install matrix in agreement."""
import json
import pathlib
import re
import subprocess
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
config = json.loads((ROOT / 'scripts/bundles.json').read_text())
workflow = (ROOT / '.github/workflows/release.yml').read_text()
matrix_doc = (ROOT / 'docs/install-matrix.md').read_text()
cargo = tomllib.loads((ROOT / 'crates/saccade/Cargo.toml').read_text())['features']

assert config['bundles']['default']['asset'] == 'saccade-{target}', 'legacy asset name must keep publishing'
for name, spec in config['bundles'].items():
    for feature in spec['features']:
        assert feature in cargo, f'{name}: {feature} is not a saccade Cargo feature'
        assert feature not in config['excluded_everywhere'], f'{name} enables an excluded feature'
    assert set(spec['features']) <= set(spec['expect_features']) | {'media-http', 'ocr-provider'}, name
    assert f'saccade-{name}-' in matrix_doc, f'{name} missing from docs/install-matrix.md'

# Every non-default bundle x platform has a matrix row in release.yml, and no extras.
rows = set(re.findall(r'\{bundle: (\w+), os: [\w.-]+, target: ([\w-]+),', workflow))
wanted = {(n, t) for n, s in config['bundles'].items() if n != 'default' for t in s['platforms']}
assert rows == wanted, (rows ^ wanted)
# The default bundle is produced inside the legacy build job, beside the unchanged assets.
assert 'bundles.py smoke default' in workflow and 'saccade-*.tar.gz dist/saccade-*.zip dist/SHA256SUMS' in workflow
# Publishing jobs stay intact.
for needle in ('crates-io-auth-action', 'mcp-publisher login github-oidc', 'cargo publish --locked --registry crates-io -p saccade-core'):
    assert needle in workflow, needle

# Helper output.
out = subprocess.run([sys.executable, str(ROOT / 'scripts/bundles.py'), 'expected'], capture_output=True, text=True, check=True).stdout.split()
assert len(out) == len(set(out)) == 3 * 3 * 4, len(out)
assert subprocess.run([sys.executable, str(ROOT / 'scripts/bundles.py'), 'features', 'media'], capture_output=True, text=True, check=True).stdout.strip() == ','.join(config['bundles']['media']['features'])
print('bundle configuration consistent')
