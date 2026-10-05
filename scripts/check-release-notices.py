#!/usr/bin/env python3
"""Verify copied-source attribution inside each binary release archive."""
import pathlib
import sys
import tarfile
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
DALTON_NOTICE = ROOT / 'crates/saccade-core/assets/licenses/daltonlens-MIT.txt'


def check_archive(path):
    """Both release formats must contain the complete retained DaltonLens notice."""
    path = pathlib.Path(path)
    if path.suffix == '.zip':
        with zipfile.ZipFile(path) as archive:
            notices = archive.read('THIRD_PARTY_NOTICES.md').decode('utf-8')
    else:
        with tarfile.open(path, 'r:gz') as archive:
            notices = archive.extractfile('THIRD_PARTY_NOTICES.md').read().decode('utf-8')
    notices = notices.replace('\r\n', '\n')
    assert 'DaltonLens' in notices, f'{path}: missing DaltonLens attribution'
    assert DALTON_NOTICE.read_text().strip() in notices, f'{path}: missing full DaltonLens MIT notice'
    print(f'{path.name}: copied-source notices retained')


if __name__ == '__main__':
    for name in sys.argv[1:]:
        check_archive(name)
    if len(sys.argv) < 2:
        raise SystemExit('usage: check-release-notices.py ARCHIVE...')
