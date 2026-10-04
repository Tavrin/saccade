#!/usr/bin/env python3
"""Reject unintended material in both crates.io source archives."""
import subprocess

allowed = ('.cargo_vcs_info.json', 'Cargo.lock', 'Cargo.toml', 'Cargo.toml.orig')
for crate in ('saccade-core', 'saccade'):
    files = subprocess.check_output(['cargo', 'package', '-p', crate, '--list', '--allow-dirty', '--locked'], text=True).splitlines()
    assert files, crate
    for path in files:
        assert path in allowed or path.startswith(('src/', 'tests/', 'examples/', 'assets/')) or path == 'build.rs', (crate, path)
        assert not any(part in ('target', 'report', 'evaluation', 'private', 'node_modules') for part in path.split('/')), (crate, path)
        if path.endswith(('.png', '.jpg', '.jpeg', '.gif', '.webp')):
            assert crate == 'saccade' and path.startswith('assets/demo/'), (crate, path)
    print(f'{crate}: {len(files)} approved package files')
