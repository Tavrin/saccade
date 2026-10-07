#!/usr/bin/env python3
"""Generate two unrelated procedural sets and assert frozen sensitivity receipts offline."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import shutil

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def generate(out):
    from PIL import Image, ImageDraw, ImageChops, __version__
    out.mkdir(parents=True, exist_ok=True)
    files = {}
    for family in ('set-a', 'set-b'):
        folder = out / family / 'nested'
        folder.mkdir(parents=True, exist_ok=True)
        for index in range(2):
            im = Image.new('RGB', (192, 128))
            for y in range(128):
                for x in range(192):
                    if family == 'set-a':
                        im.putpixel((x, y), (35 + x // 4, 60 + y // 3, 95 + (x + y) // 6))
                    else:
                        im.putpixel((x, y), (225 - y // 8, 222 - y // 7, 214 - y // 6))
            d = ImageDraw.Draw(im)
            if family == 'set-a':
                d.polygon([(0, 95), (70, 65), (191, 88), (191, 127), (0, 127)], fill=(45, 85, 50))
                d.rectangle((110, 25, 145, 75), fill=(105, 90, 75))
                d.polygon([(105, 25), (127, 9 + index), (150, 25)], fill=(120, 55, 45))
                for x in range(6, 190, 20):
                    d.line((x, 110, x + 5, 98), fill=(95, 145, 85), width=2)
            else:
                d.ellipse((80, 85, 160, 110), fill=(180, 180, 173))
                d.rounded_rectangle((100, 30, 142, 96), radius=8, fill=(40, 85 + index * 10, 145))
                d.rectangle((112, 20, 129, 32), fill=(55, 55, 55))
                d.rectangle((104, 48, 138, 73), fill=(235, 235, 220))
            # A declared removable element, common geometry in both unrelated sets.
            d.rectangle((57, 38, 86, 63), fill=(220, 220, 215))
            d.rectangle((58, 39, 85, 62), fill=(230, 45, 35))
            path = folder / f'{index}.png'
            im.save(path)
            files[str(path.relative_to(out))] = sha(path)
    injections = [
        {'id': 'local', 'defect': {'class': 'local_edit', 'rect': [0.3, 0.3, 0.15, 0.2], 'colour': [10, 220, 80]}, 'magnitudes': [0.25, 0.5, 1]},
        {'id': 'colour', 'defect': {'class': 'colour_shift', 'delta': [50, -30, 20]}, 'magnitudes': [0.25, 0.5, 1]},
        {'id': 'missing', 'defect': {'class': 'missing_element', 'rect': [0.3, 0.3, 0.15, 0.2], 'background': [220, 220, 215]}, 'magnitudes': [0.25, 0.5, 1]},
        {'id': 'blur', 'defect': {'class': 'blur'}, 'magnitudes': [0.5, 1, 2]},
        {'id': 'shift', 'defect': {'class': 'misalignment'}, 'magnitudes': [1, 2, 4]},
    ]
    patches = out / 'patches'
    patches.mkdir(exist_ok=True)
    glyph_manifest = json.loads((ROOT / 'testdata/critical-text/fixtures.json').read_text())
    glyph_provenance = glyph_manifest['provenance']
    for name in ('app-decimal', 'app-currency', 'figure-minus', 'document-warning'):
        pack = ROOT / 'testdata/critical-text' / name
        for file in ('baseline.png', 'edited.png'):
            assert sha(pack / file) == glyph_manifest['files'][f'{name}/{file}'], 'N13 input pin changed'
        pair = [Image.open(pack / file).convert('RGB') for file in ('baseline.png', 'edited.png')]
        box = ImageChops.difference(*pair).getbbox()
        assert box, name
        paths = []
        for kind, image in zip(('before', 'after'), pair):
            path = patches / f'{name}-{kind}.png'
            image.crop(box).save(path)
            paths.append(path)
            files[str(path.relative_to(out))] = sha(path)
        injections.append({'id': name, 'defect': {'class': 'glyph_edit',
            'before': str(paths[0].relative_to(out)), 'after': str(paths[1].relative_to(out)),
            'before_sha256': sha(paths[0]), 'after_sha256': sha(paths[1]), 'origin': [0.1, 0.65]},
            'magnitudes': [0.25, 0.5, 1]})
    write(out / 'catalogue.json', {'schema': 'saccade-sensitivity-catalogue.v1',
        'provenance': 'Procedural controls: MIT OR Apache-2.0. Glyph patches derived from the pinned N13 pack: generated imagery MIT OR Apache-2.0, Liberation Sans SIL-OFL-1.1; see provenance.json.', 'injections': injections})
    (out / 'configured.toml').write_text('threshold = 0.02\nmetric = "mean"\nhotspot_fail = 0.45\n')
    (out / 'before.toml').write_text('threshold = 1.0\nmetric = "mean"\n')
    write(out / 'provenance.json', {'generated_assets_licence': 'MIT OR Apache-2.0',
        'generator': 'scripts/sensitivity/fixtures.py', 'pillow_version': __version__,
        'construction': {'set-a': 'procedural perspective scene captures with coloured objects and fine edges',
                         'set-b': 'procedural still-life product images with smooth illumination and shadows'},
        'glyph_source': glyph_provenance, 'files': files})


def qualify(binary, inputs, receipts):
    receipts.mkdir(parents=True, exist_ok=True)
    qualifications = []
    for family in ('set-a', 'set-b'):
        before = {str(p.relative_to(inputs / family)): sha(p) for p in (inputs / family).rglob('*.png')}
        argv = [str(binary), 'sensitivity', str(inputs / family), '--catalogue', str(inputs / 'catalogue.json'),
                '--config', str(inputs / 'configured.toml'), '--before-config', str(inputs / 'before.toml'),
                '--out', str(receipts / family), '--json']
        result = subprocess.run(argv, capture_output=True, text=True)
        assert result.returncode in (0, 1), result.stdout + result.stderr
        report = json.loads(result.stdout)
        assert report['schema'] == 'saccade-sensitivity.v2' and report['state'] == 'complete'
        assert len(report['trials']) == 54
        assert {str(p.relative_to(inputs / family)): sha(p) for p in (inputs / family).rglob('*.png')} == before
        classes = {'local_edit', 'colour_shift', 'missing_element', 'glyph_edit', 'blur', 'misalignment'}
        assert {r['class'] for r in report['summaries']} == classes
        for row in report['summaries']:
            counts = row['counts']
            assert counts['excluded'] == counts['ineffective'] == counts['unavailable'] == 0
            assert row['miss_rate'] == counts['missed'] / (counts['detected'] + counts['missed'])
            if row['gate'] == 'before':
                assert counts['detected'] == 0 and row['miss_rate'] == 1
        for cls in classes:
            assert any(r['gate'] == 'configured' and r['class'] == cls and r['counts']['detected'] > 0 for r in report['summaries']), cls
        assert any(r['gate'] == 'configured' and r['counts']['missed'] > 0 for r in report['summaries'])
        for minimum in report['minima']:
            seen = [r['magnitude'] for r in report['summaries'] if r['class'] == minimum['class'] and r['gate'] == minimum['gate'] and r['counts']['detected'] > 0]
            assert minimum['smallest_detected_magnitude'] == (min(seen) if seen else None)
        for trial in report['trials']:
            assert trial['changed_pixels'] > 0
            for path, pin in zip(trial['images'], trial['image_sha256']):
                assert sha(receipts / family / path) == pin
        assert 'Sensitivity on injected defects is not field recall.' in report['limitations']
        qualifications.append({'set': family, 'exit': result.returncode, 'command': argv,
                               'report_sha256': sha(receipts / family / 'saccade-sensitivity.v2.json'),
                               'summaries': report['summaries'], 'minima': report['minima'], 'unchanged_sources': before})
    write(receipts / 'qualification.json', qualifications)
    print('PASS: both generated sets, six classes, frozen before/after gates, rates, minima and unchanged sources')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--receipts', type=Path)
    args = parser.parse_args()
    generate(args.out)
    if args.binary:
        assert args.receipts, '--receipts required with --binary'
        binary = Path(shutil.which(str(args.binary)) or args.binary).resolve()
        qualify(binary, args.out.resolve(), args.receipts.resolve())
