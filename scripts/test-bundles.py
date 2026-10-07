#!/usr/bin/env python3
"""Keep bundles.json, release.yml, Cargo features and the install matrix in agreement."""
import json
import importlib.util
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
core = tomllib.loads((ROOT / 'crates/saccade-core/Cargo.toml').read_text())['features']

assert config['bundles']['default']['asset'] == 'saccade-{target}', 'legacy asset name must keep publishing'
for name, spec in config['bundles'].items():
    for feature in spec['features']:
        assert feature in cargo, f'{name}: {feature} is not a saccade Cargo feature'
        assert feature not in config['excluded_everywhere'], f'{name} enables an excluded feature'
    # Resolve local Cargo feature edges, including defaults and implied local-models.
    actual = set()
    visited = set()
    def enable(feature, package='cli'):
        graph = cargo if package == 'cli' else core
        if (package, feature) in visited or feature not in graph:
            return
        visited.add((package, feature))
        if feature != 'default':
            actual.add(feature)
        for child in graph[feature]:
            if child.startswith('saccade-core/'):
                enable(child.split('/', 1)[1], 'core')
            elif child in graph:
                enable(child, package)
    enable('default')
    for feature in spec['features']:
        enable(feature)
    assert actual == set(spec['expect_features']), (name, actual ^ set(spec['expect_features']))
    assert f'saccade-{name}-' in matrix_doc, f'{name} missing from docs/install-matrix.md'

# Every non-default bundle x platform has a matrix row in release.yml, and no extras.
rows = set(re.findall(r'\{bundle: (\w+), os: [\w.-]+, target: ([\w-]+),', workflow))
wanted = {(n, t) for n, s in config['bundles'].items() if n != 'default' for t in s['platforms']}
assert rows == wanted, (rows ^ wanted)
# The default bundle is produced inside the legacy build job, beside the unchanged assets.
assert 'bundles.py smoke default' in workflow and 'saccade-*.tar.gz dist/saccade-*.zip dist/SHA256SUMS' in workflow
# Both build jobs must use the same JSON feature source as the inventory.
assert 'bundles.py features default' in workflow
assert 'bundles.py features "$BUNDLE"' in workflow
assert not re.search(r'cargo build[^\n]+--features prechecks\b', workflow)
# Publishing jobs stay intact.
for needle in ('crates-io-auth-action', 'mcp-publisher login github-oidc', 'cargo publish --locked --registry crates-io -p saccade-core'):
    assert needle in workflow, needle

# Helper output.
out = subprocess.run([sys.executable, str(ROOT / 'scripts/bundles.py'), 'expected'], capture_output=True, text=True, check=True).stdout.split()
assert len(out) == len(set(out)) == 3 * 3 * 4, len(out)
assert subprocess.run([sys.executable, str(ROOT / 'scripts/bundles.py'), 'features', 'media'], capture_output=True, text=True, check=True).stdout.strip() == ','.join(config['bundles']['media']['features'])
# The runtime smoke and inventory guard must reject both missing and extra features.
module_spec = importlib.util.spec_from_file_location('bundles', ROOT / 'scripts/bundles.py')
helper = importlib.util.module_from_spec(module_spec)
module_spec.loader.exec_module(helper)
for name, spec in config['bundles'].items():
    helper.validate_features(name, spec['expect_features'])
    for mismatch in (spec['expect_features'][1:], spec['expect_features'] + ['unexpected']):
        try:
            helper.validate_features(name, mismatch)
        except SystemExit:
            pass
        else:
            raise AssertionError(f'{name}: mismatched runtime inventory accepted')
# Reject the stale PyPI distribution spelling while preserving import saccade.
for path in ROOT.rglob('*.md'):
    if '.git' not in path.parts:
        assert not re.search(r'pip(?:3)? install(?:[^\n`]* )?saccade(?=[`\s\[]|$)', path.read_text()), path
print('bundle configuration consistent')
