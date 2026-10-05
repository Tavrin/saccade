#!/usr/bin/env python3
"""Generate a lockfile-bound license inventory and available upstream notices."""
import argparse
import json
import pathlib
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('output', type=pathlib.Path)
args = parser.parse_args()
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--all-features', '--format-version', '1']))
packages = sorted((p for p in metadata['packages'] if p['source']), key=lambda p: (p['name'], p['version']))
lines = ['# Third-party notices', '', 'Generated from Cargo.lock via cargo metadata --locked --all-features.',
         'This inventory includes every resolved external crate, including build, optional,',
         'development and platform-specific dependencies, so no target is omitted.',
         'SPDX expressions identify the upstream license choices. The project MIT and',
         'Apache-2.0 texts are shipped beside this file.', '']
for p in packages:
    name, version = p['name'], p['version']
    license_id = p.get('license') or ('SEE LICENSE FILE: ' + str(p.get('license_file')))
    if not license_id:
        raise SystemExit(f'{name} {version}: missing license metadata')
    lines += [f'## {name} {version}', '', f'License: {license_id}', f'Source: {p["source"]}', '']
    root = pathlib.Path(p['manifest_path']).parent
    files = sorted(f for f in root.iterdir() if f.is_file() and f.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE')))
    license_file = p.get('license_file')
    if license_file:
        f = root / license_file
        if f.is_file() and f not in files:
            files.append(f)
    for f in files:
        content = f.read_text(encoding='utf-8', errors='replace').strip()
        if content:
            lines += [f'### {f.name}', '', '```text', content, '```', '']
    if not files:
        lines += ['No separate license text is present in the published crate; see the SPDX expression above.', '']
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text('\n'.join(lines), encoding='utf-8')
print(f'{len(packages)} external crates: {args.output}')
