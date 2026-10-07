#!/usr/bin/env python3
"""Run generated acceptance proofs, retaining inputs and full reports at --out."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

ap = argparse.ArgumentParser(description=__doc__)
ap.add_argument('--bin', required=True)
ap.add_argument('--out', required=True)
args = ap.parse_args()
root = Path(args.out).resolve()
subprocess.run([sys.executable, Path(__file__).with_name('gen-split-fixtures.py'), root / 'inputs'], check=True)
binary = Path(args.bin).resolve()


def review(manifest, name, exit_code):
    proc = subprocess.run([binary, 'split-review', manifest, '--out', root / name, '--json'],
                          capture_output=True, text=True)
    assert proc.returncode == exit_code, (proc.returncode, proc.stdout, proc.stderr)
    report = json.loads(proc.stdout)
    (root / f'{name}-stdout.json').write_text(json.dumps(report, indent=2)+'\n')
    assert report['schema'] == 'saccade-split-review.v2'
    assert report['routes'][2]['status'] == 'not_enabled'
    return report


report = review(root / 'inputs/dataset/splits.json', 'dataset-review', 1)
assert report['injected_recall']['total'] == 8
assert report['injected_recall']['found'] == 8, report['injected_recall']
assert all(r['recall'] == 1 for r in report['injected_recall']['by_transform'])
assert not report['injected_recall']['missed_pairs']
for pair in report['cross_split_pairs']:
    assert pair['a_split'] != pair['b_split']
    assert 'unrelated.png' not in pair['a'] + pair['b'], pair
entries = {e['path']: e for e in report['entries']}
for i in range(2):
    source_hash = int(entries[f'train/source-{i}.png']['phash'], 16)
    crop_hash = int(entries[f'test/cropped-{i}.png']['phash'], 16)
    assert (source_hash ^ crop_hash).bit_count() > report['routes'][0]['threshold']
    pair = next(p for p in report['cross_split_pairs']
                if {p['a'], p['b']} == {f'train/source-{i}.png', f'test/cropped-{i}.png'})
    assert any(e['route'] == 'geometric' and e['inliers'] >= 6 for e in pair['evidence']), pair
burst = review(root / 'inputs/burst/splits.json', 'burst-review', 0)
assert not burst['cross_split_pairs']
assert len(burst['groups']) == 1, burst['groups']
assert set(burst['groups'][0]['members']) == {'shot-0.png', 'shot-1.png', 'shot-2.jpg'}
assert burst['injected_recall']['recall'] is None
assert 'not proof of no leakage' in burst['summary']
assert (root / 'dataset-review/pairs.csv').read_text().startswith('a,b,a_split,b_split,evidence\n')
print('PASS: 8/8 injected pairs (2 each exact/recompressed/resized/cropped); crop geometry; independent negative; 3-shot burst; clean-list caveat.')
