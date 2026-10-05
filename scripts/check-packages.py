#!/usr/bin/env python3
"""Reject unintended material and README drift in crates.io source archives."""
import json
import re
import subprocess
import tarfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
READMES = {
    'saccade-core': ROOT / 'crates/saccade-core/README.md',
    'saccade': ROOT / 'README.md',
}
ALLOWED = ('.cargo_vcs_info.json', 'Cargo.lock', 'Cargo.toml', 'Cargo.toml.orig', 'README.md')
MODEL_MANIFEST = 'models/semantic-regions.json'
COMPRESSION_IMAGES = {f'tests/fixtures/compression-reference/{name}.png'
                      for name in ('reference', 'brightness', 'blocks', 'patch')}


def approved_image(crate, path):
    """Only demo assets and the four project-authored compression fixtures ship."""
    return ((crate == 'saccade' and path.startswith('assets/demo/'))
            or (crate == 'saccade-core' and path in COMPRESSION_IMAGES))


def approved_file(crate, path):
    """Allow the pinned model manifest, without allowing a directory of weights."""
    return (path in ALLOWED or path.startswith(('src/', 'tests/', 'examples/', 'assets/'))
            or path == 'build.rs' or (crate == 'saccade-core'
            and (path.startswith('schemas/') or path == MODEL_MANIFEST)))


def check_readme(package, archive, intended, marker=None):
    """Check both the selected source and the README actually shipped by Cargo."""
    crate = package['name']
    manifest = Path(package['manifest_path'])
    source = tomllib.loads(manifest.read_text())['package']['readme']
    assert (manifest.parent / source).resolve() == intended.resolve(), (crate, 'wrong README source')
    prefix = f"{crate}-{package['version']}/"
    with tarfile.open(archive, 'r:gz') as contents:
        normalized = tomllib.loads(contents.extractfile(prefix + 'Cargo.toml').read().decode())
        assert normalized['package']['readme'] == 'README.md', (crate, 'wrong packaged README path')
        readme = contents.extractfile(prefix + 'README.md').read()
    assert len(readme.splitlines()) >= 30, (crate, 'packaged README has fewer than 30 lines')
    assert readme == intended.read_bytes(), (crate, 'packaged README differs from intended source')
    if marker:
        visible = re.sub(r'<!--.*?-->', '', readme.decode(), flags=re.S)
        assert marker in visible, (crate, 'packaged README is missing the visible MCP ownership marker')
    print(f'{crate}: packaged README matches {intended.relative_to(ROOT)} ({len(readme.splitlines())} lines)')


def main():
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--format-version', '1', '--locked'], cwd=ROOT, text=True))
    packages = [p for p in metadata['packages']
                if p['id'] in metadata['workspace_members'] and p['publish'] != []]
    assert {p['name'] for p in packages} == set(READMES), 'update the intended README inventory'
    # Package the workspace together so an unpublished core version resolves locally.
    # Recreate archives on every invocation; a stale archive cannot satisfy this guard.
    subprocess.run(['cargo', 'package', '--workspace', '--all-features', '--no-verify',
                    '--allow-dirty', '--locked'], cwd=ROOT, check=True)
    server = json.loads((ROOT / 'server.json').read_text())
    registry_crates = {p['identifier'] for p in server['packages'] if p['registryType'] == 'cargo'}
    assert registry_crates <= set(READMES), 'registry package is outside the README inventory'
    for package in packages:
        crate = package['name']
        files = subprocess.check_output(['cargo', 'package', '-p', crate, '--list', '--allow-dirty', '--locked'],
                                        cwd=ROOT, text=True).splitlines()
        assert files and 'README.md' in files, crate
        if crate == 'saccade-core':
            expected = {f'schemas/{p.name}' for p in (ROOT / 'crates/saccade-core/schemas').glob('*.schema.json')}
            packaged = {p for p in files if p.startswith('schemas/')}
            assert expected and packaged == expected, (crate, expected - packaged, packaged - expected)
            assert 'examples/panel.toml' in files, crate
            assert {p for p in files if p.startswith('models/')} == {MODEL_MANIFEST}, crate
        for path in files:
            assert approved_file(crate, path), (crate, path)
            assert not any(part in ('target', 'report', 'evaluation', 'private', 'node_modules') for part in path.split('/')), (crate, path)
            if path.endswith(('.png', '.jpg', '.jpeg', '.gif', '.webp')):
                assert approved_image(crate, path), (crate, path)
        archive = Path(metadata['target_directory']) / 'package' / f"{crate}-{package['version']}.crate"
        marker = f"mcp-name: {server['name']}" if crate in registry_crates else None
        check_readme(package, archive, READMES[crate], marker)
        print(f'{crate}: {len(files)} approved package files')


if __name__ == '__main__':
    main()
