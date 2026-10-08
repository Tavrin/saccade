#!/usr/bin/env python3
"""Generated text-quality truth fixtures. Pillow, installed fonts; no downloads."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import subprocess
from PIL import Image, ImageDraw, ImageFilter, ImageFont, __version__ as PIL_VERSION


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--binary', type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    fonts = {
        'latin': Path('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'),
        'arabic': Path('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'),
        'cjk': Path('/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc'),
    }
    for path in fonts.values():
        if not path.is_file():
            raise RuntimeError(f'required installed font missing: {path.name}')
    packages = subprocess.run(
        ['dpkg-query', '-W', '-f=${Package}\t${Version}\n',
         'fonts-dejavu-core', 'fonts-noto-cjk'],
        check=True, capture_output=True, text=True,
    )
    font_packages = dict(line.split('\t', 1) for line in packages.stdout.splitlines())
    font = ImageFont.truetype(str(fonts['latin']), 26)
    region = [12, 12, 396, 62]
    cases = []
    def save(name, image):
        path = args.out / (name + '.png')
        image.save(path)
        return path
    def text_image(text='Sample text', fg=(20, 20, 20), bg=(250, 250, 250), size=26, selected=font):
        image = Image.new('RGB', (420, 88), bg)
        ImageDraw.Draw(image).text((24, 20), text, font=selected if size == 26 else ImageFont.truetype(str(fonts['latin']), size), fill=fg)
        return image
    def case(command, name, expected, **fields):
        cases.append(dict(command=command, name=name, expected=expected, **fields))
    base = save('baseline', text_image())
    for name, image, expected, reason in [
        ('legible', text_image(), 'legible', None),
        ('low-contrast', text_image(fg=(220, 220, 220)), 'illegible', 'low_contrast'),
        ('small', text_image(size=7), 'illegible', 'too_small'),
        ('blurred', text_image().filter(ImageFilter.GaussianBlur(2.4)), 'illegible', 'blurred'),
        ('dark', text_image(fg=(245, 245, 245), bg=(15, 15, 15)), 'legible', None),
        ('empty', Image.new('RGB', (420, 88), (250, 250, 250)), 'insufficient_evidence', None),
        ('double-size', text_image().resize((840, 176), Image.Resampling.NEAREST), 'legible', None),
    ]:
        save(name, image)
        case('text-legibility', name, expected, baseline='baseline', regions=[region], reason=reason)
    # Three unrelated capture constructions use identical public command options.
    for number in range(3):
        image = Image.new('RGB', (420, 180), [(250, 250, 250), (18, 22, 30), (248, 246, 240)][number])
        draw = ImageDraw.Draw(image)
        draw.text((24, 22), 'Sample text', font=font, fill=(20, 20, 20) if number != 1 else (245, 245, 245))
        draw.text((24, 110), 'Additional line', font=font, fill=(30, 30, 30) if number != 1 else (230, 230, 230))
        injected = image.copy()
        ImageDraw.Draw(injected).rectangle((290, 26, 303, 49), outline=(5, 5, 5) if number != 1 else (250, 250, 250), width=2)
        if number == 2:
            injected = injected.transform(injected.size, Image.Transform.PERSPECTIVE, (1, .025, 0, .01, 1, 0, .0001, .0001), Image.Resampling.BICUBIC, fillcolor=(248, 246, 240))
            image = image.transform(image.size, Image.Transform.PERSPECTIVE, (1, .025, 0, .01, 1, 0, .0001, .0001), Image.Resampling.BICUBIC, fillcolor=(248, 246, 240))
            rng = random.Random(17)
            for _ in range(400):
                x, y = rng.randrange(420), rng.randrange(180)
                if y > 80:
                    image.putpixel((x, y), (235, 233, 227))
                    injected.putpixel((x, y), (235, 233, 227))
        baseline_name = f'capture-{number}-baseline'
        save(baseline_name, image)
        r = [12, 12, 396, 68]
        name = f'capture-{number}'
        save(name, image)
        # Perspective interpolation introduces thin edge cores without a plateau.
        case('text-legibility', name, 'insufficient_evidence' if number == 2 else 'legible', baseline=baseline_name, regions=[r], reason='contrast_lower_bound_below_target' if number == 2 else None)
        save(name + '-box', injected)
        case('tofu', name + '-box', 'candidates', witnesses=[[290, 26, 14, 24]] if number != 2 else [], inside=[280, 16, 48, 54])
    # Real font .notdef (DejaVu lacks CJK), injected boxes, actual replacement glyph.
    for name, image in [('missing-font', text_image('A\u4e00B')), ('replacement', text_image('A\ufffdB'))]:
        save(name, image)
        case('tofu', name, 'candidates')
    image = text_image()
    ImageDraw.Draw(image).rectangle((290, 26, 303, 49), outline=(20, 20, 20), width=2)
    save('injected', image)
    case('tofu', 'injected', 'candidates', witnesses=[[290, 26, 14, 24]])
    mask = Image.new('L', image.size, 0)
    ImageDraw.Draw(mask).rectangle((280, 16, 313, 59), fill=255)
    save('mask', mask)
    case('tofu', 'injected', 'candidates', mask='mask', witnesses=[[290, 26, 14, 24]])
    save('empty-mask', Image.new('L', image.size, 0))
    case('tofu', 'injected', 'insufficient_evidence', mask='empty-mask')
    for name, text, kind in [('latin', 'Ordinary readable text', 'latin'), ('cjk', '\u4e2d\u6587\u65e5\u672c\u8a9e\u53e3\u56de\u7530', 'cjk'), ('arabic', '\u0645\u0631\u062d\u0628\u0627 \u0628\u0627\u0644\u0639\u0627\u0644\u0645', 'arabic')]:
        selected = ImageFont.truetype(str(fonts[kind]), 26)
        save(name, text_image(text, selected=selected))
        case('tofu', name, 'insufficient_evidence', no_candidates=True)
    # Square boxes intentionally abstain: indistinguishable from legitimate glyphs.
    image = Image.new('RGB', (420, 88), 'white')
    ImageDraw.Draw(image).rectangle((30, 25, 49, 44), outline='black', width=2)
    save('ambiguous-square', image)
    case('tofu', 'ambiguous-square', 'insufficient_evidence')
    # Mixed background, transparency and clipping may not give reassuring verdicts.
    mixed = text_image()
    ImageDraw.Draw(mixed).rectangle((12, 12, 407, 73), fill=(100, 100, 100))
    for y in range(12, 74):
        for x in range(12, 408):
            mixed.putpixel((x, y), ((x + y) % 256,) * 3)
    save('mixed', mixed)
    case('text-legibility', 'mixed', 'insufficient_evidence', baseline='baseline', regions=[region])
    transparent = text_image().convert('RGBA')
    transparent.putpixel((15, 15), (250, 250, 250, 0))
    save('transparent', transparent)
    case('text-legibility', 'transparent', 'insufficient_evidence', baseline='baseline', regions=[region])
    matrix = Image.new('RGB', (420, 180), (250, 250, 250))
    draw = ImageDraw.Draw(matrix)
    for y in (20, 100):
        draw.text((24, y), 'Sample text', font=font, fill=(20, 20, 20))
    save('matrix-baseline', matrix)
    save('matrix-good', matrix)
    bad = matrix.copy()
    ImageDraw.Draw(bad).rectangle((0, 88, 419, 179), fill=(250, 250, 250))
    ImageDraw.Draw(bad).text((24, 100), 'Sample text', font=font, fill=(220, 220, 220))
    save('matrix-bad', bad)
    empty = matrix.copy()
    ImageDraw.Draw(empty).rectangle((0, 88, 419, 179), fill=(250, 250, 250))
    save('matrix-empty', empty)
    case('text-legibility', 'matrix', 'illegible', baseline='matrix-baseline', regions=[[12, 12, 396, 62], [12, 92, 396, 62]], variant_names=['matrix-good', 'matrix-bad', 'matrix-empty'], region_states=[['legible', 'legible'], ['legible', 'illegible'], ['legible', 'insufficient_evidence']])
    provenance = dict(pillow=PIL_VERSION, font_packages=font_packages, fonts={k: dict(file=p.name, sha256=hashlib.sha256(p.read_bytes()).hexdigest(), license='Bitstream Vera font license; DejaVu changes public domain' if k != 'cjk' else 'SIL Open Font License 1.1') for k, p in fonts.items()}, seed=17, cases=cases)
    (args.out / 'fixtures.json').write_text(json.dumps(provenance, ensure_ascii=False, indent=2) + '\n')
    if not args.binary:
        return
    receipts = []
    defective = false_reassurance = negatives = false_tofu = 0
    for index, c in enumerate(cases):
        argv = [str(args.binary), c['command']]
        if c['command'] == 'text-legibility':
            argv += [str(args.out / (c['baseline'] + '.png'))]
            argv += [str(args.out / (name + '.png')) for name in c.get('variant_names', [c['name']])]
            for r in c['regions']:
                argv += ['--region', ','.join(map(str, r))]
        else:
            argv += [str(args.out / (c['name'] + '.png'))]
            if 'mask' in c:
                argv += ['--mask', str(args.out / (c['mask'] + '.png'))]
        result = subprocess.run(argv + ['--json'], capture_output=True, text=True)
        report = json.loads(result.stdout)
        (args.out / f'report-{index}.json').write_text(result.stdout)
        assert report['state'] == c['expected'], (c, result.returncode, report, result.stderr)
        assert result.returncode == {'legible': 0, 'candidates': 1, 'illegible': 1, 'insufficient_evidence': 4}[c['expected']], (c, result)
        if c.get('region_states'):
            assert [[r['state'] for r in v['regions']] for v in report['variants']] == c['region_states'], (c, report)
        if c.get('reason'):
            assert c['reason'] in report['variants'][0]['regions'][0]['reasons'], (c, report)
        if c.get('inside'):
            x, y, w, h = c['inside']
            assert any(v['rect_px'][0] >= x and v['rect_px'][1] >= y and v['rect_px'][0] + v['rect_px'][2] <= x + w and v['rect_px'][1] + v['rect_px'][3] <= y + h for v in report['regions']), (c, report)
        if c.get('no_candidates'):
            negatives += 1
            false_tofu += bool(report['regions'])
            assert not report['regions'], (c, report)
        for witness in c.get('witnesses', []):
            assert witness in [v['rect_px'] for v in report['regions']], (c, report)
        if c['expected'] in ('candidates', 'illegible'):
            defective += 1
            false_reassurance += report['state'] == 'legible'
        receipts.append(dict(case=index, command=c['command'], state=report['state'], exit=result.returncode))
    # OCR absence is observable and does not alter pixel observations.
    result = subprocess.run([str(args.binary), 'tofu', str(base), '--ocr', '--json'], capture_output=True, text=True)
    report = json.loads(result.stdout)
    if report['ocr']['state'] == 'unavailable':
        print('SKIP cached OCR agreement: ' + report['ocr']['reason'])
    else:
        print('Cached OCR observed; model accuracy outside generated pixel acceptance')
    receipt = dict(cases=len(receipts), defective=defective, false_reassurance=false_reassurance, negative_scripts=negatives, false_tofu=false_tofu, results=receipts)
    (args.out / 'qualification.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    main()
