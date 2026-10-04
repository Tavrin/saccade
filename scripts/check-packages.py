#!/usr/bin/env python3
"""Reject unintended material in both crates.io source archives."""
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
allowed = ('.cargo_vcs_info.json', 'Cargo.lock', 'Cargo.toml', 'Cargo.toml.orig', 'README.md')
for crate in ('saccade-core', 'saccade'):
    files = subprocess.check_output(['cargo', 'package', '-p', crate, '--list', '--allow-dirty', '--locked'], text=True).splitlines()
    assert files, crate
    assert 'README.md' in files, crate
    if crate == 'saccade-core':
        expected = {f'schemas/{p.name}' for p in (root / 'crates/saccade-core/schemas').glob('*.schema.json')}
        packaged = {p for p in files if p.startswith('schemas/')}
        assert expected and packaged == expected, (crate, expected - packaged, packaged - expected)
        assert 'examples/panel.toml' in files, crate
    for path in files:
        assert path in allowed or path.startswith(('src/', 'tests/', 'examples/', 'assets/')) or path == 'build.rs' or (crate == 'saccade-core' and path.startswith('schemas/')), (crate, path)
        assert not any(part in ('target', 'report', 'evaluation', 'private', 'node_modules') for part in path.split('/')), (crate, path)
        if path.endswith(('.png', '.jpg', '.jpeg', '.gif', '.webp')):
            assert crate == 'saccade' and path.startswith('assets/demo/'), (crate, path)
    print(f'{crate}: {len(files)} approved package files')
