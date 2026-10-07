#!/usr/bin/env python3
"""Run constructed native-raster and tile acceptance through the compiled CLI."""
import argparse
import json
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--saccade', required=True)
    parser.add_argument('--fixtures', required=True, type=Path)
    args = parser.parse_args()
    root = args.fixtures

    def run(command, before, after, out, code=0, options=()):
        result = subprocess.run([args.saccade, 'geo', command, str(root / before),
                                 str(root / after), '--out', str(root / out), '--json',
                                 *options], capture_output=True, text=True, timeout=60)
        assert result.returncode == code, (command, result.returncode, result.stdout, result.stderr)
        value = json.loads(result.stdout)
        if code != 2:
            saved = json.loads((root / out / (value['schema'] + '.json')).read_text())
            assert saved == value
            assert value['report_id'].startswith('sha256:')
        return value

    # Both illustrative domains use the same command; no domain selector exists.
    for before, after, out, index, changed, delta in [
        ('float-a.tiff', 'float-b.tiff', 'cli-float', 1, 4, 2.0),
        ('word-a.tiff', 'word-b.tiff', 'cli-word', 3, 1, 1024.0),
    ]:
        value = run('compare', before, after, out)
        assert value['bands'][index]['changed_pixels'] == changed
        assert value['bands'][index]['statistics']['max_delta'] == delta
        assert value['perceptual'] is None
        assert value['reference']['sha256'] != value['candidate']['sha256']
    preview = run('compare', 'float-a.tiff', 'float-b.tiff', 'cli-rgb', options=(
        '--rgb-bands', '1,2,3', '--rgb-min', '0,0,0', '--rgb-max', '64,64,64'))
    assert preview['perceptual']['metrics']['mean'] > 0
    assert preview['perceptual']['valid_pairs'] == 196
    assert preview['bands'][1]['nodata']['both'] == 60
    control = run('compare', 'float-a.tiff', 'float-a.tiff', 'cli-control')
    assert all(b['changed_pixels'] == 0 for b in control['bands'])
    for name in ('other-grid.tiff', 'other-crs.tiff'):
        refusal = run('compare', 'float-a.tiff', name, 'refused', 2)
        error = refusal['errors'][0]
        assert error['code'] == 'different_crs_grid'
        assert 'different CRS/grid' in error['message']
        assert 'reference:' in error['message'] and 'candidate:' in error['message']
        assert not (root / 'refused').exists()
    masks = run('mask-metrics', 'class-a.tiff', 'class-b.tiff', 'cli-class', options=('--each-label',))
    assert masks['excluded_pixels'] == 2
    target = next(c for c in masks['metrics']['classes'] if c['name'] == 'id=1000')
    assert target['iou'] == 63 / 64
    tiles = run('tiles', 'tiles-a', 'tiles-b', 'cli-tiles', 1)
    assert tiles['coverage'][0]['missing'] == ['2/1/1']
    assert tiles['coverage'][0]['extra'] == ['2/1/3']
    assert tiles['compared_tiles'] == 2 and tiles['changed_tiles'] == 1
    assert next(t for t in tiles['tiles'] if t['tile'] == '2/1/2')['metrics']['mean'] > 0
    assert run('tiles', 'tiles-a', 'tiles-a', 'cli-tiles-control')['verdict'] == 'identical'
    print('PASS: float32 3-band, uint16 4-band, nodata, RGB, classes, grids/CRS, tile coverage/change and controls')


if __name__ == '__main__':
    main()
