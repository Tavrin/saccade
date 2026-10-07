#!/usr/bin/env python3
"""Offline generated motion proofs. All artifacts go to an explicit scratch directory."""
import argparse
import hashlib
import json
import pathlib
import subprocess
import numpy as np
from PIL import Image


def run(binary, *args):
    result = subprocess.run([str(binary), *map(str, args)], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'command failed ({result.returncode}): {result.stderr}')
    return result


def generate(root, domain, count=90):
    root.mkdir(parents=True)
    y, x = np.mgrid[:48, :64]
    frames = []
    for t in range(count):
        if domain == 'texture':
            v = np.clip(128 + 48*np.sin((x-t)*0.43) + 35*np.cos((y-t*0.2)*0.71)
                        + 22*np.sin((x+y-t*0.7)*0.29), 0, 255).astype('uint8')
            im = Image.fromarray(v).convert('RGB')
        else:
            # Scrolling rows, text-like marks, and a fixed navigation stripe.
            yy = (y+t) % 48
            v = np.where((yy % 12 < 6) & (x % 19 < 14), 210, 35).astype('uint8')
            v[:, :8] = 100
            im = Image.fromarray(v).convert('RGB')
        path = root / f'{t:04}.png'
        im.save(path)
        frames.append(dict(index=t, timestamp_s=t/30, file=path.name,
                           sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    path = root/'frames.json'
    path.write_text(json.dumps(dict(schema='saccade-frame-map.v1', nominal_fps=30, frames=frames)))
    return path


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--bin', type=pathlib.Path, required=True)
    ap.add_argument('--out', type=pathlib.Path, required=True)
    ap.add_argument('--phase', choices=('all', 'generate', 'verify'), default='all')
    ap.add_argument('--domain', choices=('all', 'texture', 'scrolling'), default='all')
    a = ap.parse_args()
    binary = a.bin.resolve()
    domains = ('texture', 'scrolling')
    calibration = a.out/'calibration'
    if a.phase != 'verify':
        if a.out.exists():
            raise SystemExit('proof output must be a new directory')
        a.out.mkdir(parents=True)
        maps = [generate(a.out/domain, domain) for domain in domains]
        for domain, path in zip(domains, maps):
            same = a.out/f'{domain}-same.json'
            run(binary, 'experiment', 'motion-stats', path, path, '--out', same)
            r = json.loads(same.read_text())
            assert all(v == (1 if k.startswith('spectral_band_') else 0)
                       for k, v in r['distances'].items()), 'self-distance is not identity'
        run(binary, 'experiment', 'calibrate-degradations', '--positive', maps[0],
            '--positive', maps[1], '--strengths', '0.5,1', '--threshold', '0.8',
            '--seed', '42', '--out', calibration)
        if a.phase == 'generate':
            print('PASS: generated both domains and all negatives; verification remains')
            return
    maps = [a.out/domain/'frames.json' for domain in domains]
    manifest_bytes = (calibration/'manifest.json').read_bytes()
    manifest = json.loads(manifest_bytes)
    r = json.loads((calibration/'calibration.json').read_text())
    manifest_hash = hashlib.sha256(manifest_bytes).hexdigest()
    assert r['manifest_sha256'] == manifest_hash, 'calibration manifest drift'
    classes = ('frozen', 'flicker', 'speed_up', 'speed_down', 'looped_hitch',
               'overlay_blobs', 'temporal_blur', 'frame_drops', 'spatial_blur', 'spatial_noise')
    assert len(manifest['entries']) == 40
    assert {(e['positive_source'], e['class'], e['strength']) for e in manifest['entries']} == {
        (i, c, strength) for i in range(2) for c in classes for strength in (0.5, 1)}
    for i, path in enumerate(maps):
        assert hashlib.sha256(path.read_bytes()).hexdigest() == manifest['positive_sources'][i]['frame_map_sha256']
    proof_id = dict(manifest_sha256=manifest_hash,
                    binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
    summary = {}
    for source, domain in enumerate(domains):
        if a.domain != 'all' and a.domain != domain:
            continue
        base = json.loads((a.out/f'{domain}-same.json').read_text())['reference']['values']
        measured = {}
        for entry in manifest['entries']:
            if entry['positive_source'] != source:
                continue
            assert hashlib.sha256((calibration/entry['id']/'frames.json').read_bytes()).hexdigest() == entry['generated']['frame_map_sha256']
            report = json.loads((calibration/entry['id']/'motion-stats.json').read_text())
            measured[(entry['class'], entry['strength'])] = report
            # Exercise motion-stats on generated negatives through the public command too.
            target = a.out/f"{entry['id']}-paired.json"
            run(binary, 'experiment', 'motion-stats', maps[source],
                calibration/entry['id']/'frames.json', '--out', target)
            public = json.loads(target.read_text())
            assert public['distances'] == report['distances'], 'calibration/public metric drift'
        for strength in (0.5, 1):
            frozen = measured[('frozen', strength)]['candidate']['values']
            assert frozen['duplicate_fraction'] > base['duplicate_fraction'] + 0.2
            flicker = measured[('flicker', strength)]['candidate']['values']
            assert flicker['flicker_second_difference_intensity_squared'] > base['flicker_second_difference_intensity_squared'] * 1.5
            hitch = measured[('looped_hitch', strength)]
            assert hitch['candidate']['values']['hitch_fraction'] > 0
            assert hitch['candidate']['spectrum'] is None
            for speed in ('speed_up', 'speed_down'):
                assert measured[(speed, strength)]['distances']['median_interval_s'] > 1e-6
                v = measured[(speed, strength)]['candidate']['values']
                if base.get('flow_mean_px_per_s', 0) > 0:
                    factor = (1+strength) if speed == 'speed_up' else 1/(1+strength)
                    assert abs(v['flow_mean_px_per_s']/base['flow_mean_px_per_s']-factor) < 1e-6
        summary[domain] = {f'{cls}@{strength}': sorted(k for k, v in report['distances'].items()
                           if (abs(v-1) if k.startswith('spectral_band_') else v) > 1e-9)
                           for (cls, strength), report in measured.items()}
    assert all(not s['trusted_for'] for s in r['scorers']), 'two sources cannot clear exact lower bound 0.8'
    for domain, measured in summary.items():
        (a.out/f'acceptance-{domain}.json').write_text(json.dumps(
            dict(proof_id=proof_id, domain=domain, separation=measured), indent=2)+'\n')
    combined = {}
    for domain in domains:
        marker = a.out/f'acceptance-{domain}.json'
        if marker.exists():
            data = json.loads(marker.read_text())
            assert data['proof_id'] == proof_id, 'verification marker belongs to different evidence'
            combined[domain] = data['separation']
    if len(combined) == 2:
        combined.update(trust=r['scorers'], proof_id=proof_id,
                        acceptance='PASS: asserted effects in both generated domains; per-metric separated classes recorded, no perceptual qualification')
        (a.out/'acceptance.json').write_text(json.dumps(combined, indent=2)+'\n')
        print(combined['acceptance'])
    else:
        print('PASS: verification for '+', '.join(summary)+'; other domain remains')



if __name__ == '__main__':
    main()
