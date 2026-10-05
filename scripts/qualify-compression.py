#!/usr/bin/env python3
"""Remeasure the committed synthetic pairs with independently built official tools.

No tool downloads or Rust-generated golden values. See the fixture README for
pinned versions and build instructions; ordinary Cargo tests need no C++ tools.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ssimulacra2', type=Path, required=True)
    parser.add_argument('--butteraugli', type=Path, required=True)
    parser.add_argument('--cloudinary', type=Path, required=True)
    parser.add_argument('--jxl-source', type=Path, required=True)
    parser.add_argument('--cloudinary-source', type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'crates/saccade-core/tests/fixtures/compression-reference'
    path = root / 'values.json'
    values = json.loads(path.read_text())
    tools = {'ssimulacra2': args.ssimulacra2, 'butteraugli': args.butteraugli,
             'cloudinary_ssimulacra2': args.cloudinary}
    for pair in values['pairs']:
        for metric, tool in tools.items():
            output = subprocess.check_output([str(tool.resolve()), str(root / pair['reference']),
                                              str(root / pair['distorted'])], text=True, timeout=60)
            pair[metric] = float(output.splitlines()[0])
    values['files_sha256'] = {p.name: digest(p) for p in sorted(root.glob('*.png'))}
    values['reference_source_sha256'] = {
        'libjxl_ssimulacra2': digest(args.jxl_source / 'tools/ssimulacra2.cc'),
        'libjxl_butteraugli': digest(args.jxl_source / 'lib/jxl/butteraugli/butteraugli.cc'),
        'cloudinary_ssimulacra2': digest(args.cloudinary_source / 'src/ssimulacra2.cc'),
    }
    values['tool_sha256'] = {'libjxl_ssimulacra2': digest(args.ssimulacra2),
                             'libjxl_butteraugli': digest(args.butteraugli),
                             'cloudinary_ssimulacra2': digest(args.cloudinary)}
    path.write_text(json.dumps(values, indent=2) + '\n')
    print(path)


if __name__ == '__main__':
    main()
