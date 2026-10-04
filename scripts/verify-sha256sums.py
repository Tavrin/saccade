#!/usr/bin/env python3
"""Verify the complete release checksum manifest on every runner OS."""
import hashlib
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
entries = {}
for line in (root / 'SHA256SUMS').read_text().splitlines():
    match = re.fullmatch(r'([0-9a-f]{64})  (saccade-[A-Za-z0-9_.-]+\.(?:tar\.gz|zip))', line)
    if not match or match[2] in entries:
        raise SystemExit(f'invalid SHA256SUMS line: {line!r}')
    entries[match[2]] = match[1]
expected = {
    'saccade-x86_64-unknown-linux-gnu.tar.gz',
    'saccade-aarch64-unknown-linux-gnu.tar.gz',
    'saccade-aarch64-apple-darwin.tar.gz',
    'saccade-x86_64-pc-windows-msvc.zip',
}
if entries.keys() != expected:
    raise SystemExit(f'wrong archive set: {entries.keys()}')
for name, digest in entries.items():
    actual = hashlib.sha256((root / name).read_bytes()).hexdigest()
    if actual != digest:
        raise SystemExit(f'checksum mismatch: {name}')
print(f'verified {len(entries)} release archives')
