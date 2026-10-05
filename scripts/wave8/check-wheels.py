#!/usr/bin/env python3
"""Require retained manylinux abi3 wheels for both architectures; never publish."""
import os
from pathlib import Path
import zipfile

folder = Path(os.environ['SACCADE_W8_WHEELS'])
for arch in ['x86_64', 'aarch64']:
    matches = [p for p in folder.glob('*.whl') if arch in p.name and 'abi3' in p.name and 'manylinux' in p.name]
    assert len(matches) == 1, f'require exactly one qualified {arch} manylinux abi3 wheel'
    with zipfile.ZipFile(matches[0]) as wheel:
        names = wheel.namelist()
        assert 'saccade/__init__.pyi' in names and 'saccade/py.typed' in names
        assert any(name.startswith('saccade/_native') and name.endswith('.so') for name in names)
print('Retained x86_64 and aarch64 manylinux abi3 artifacts: PASS')
