#!/usr/bin/env python3
"""Generate an offline OFL glyph-edit pack; optionally assert real CLI gate receipts."""
import argparse
import hashlib
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
FONT = pathlib.Path('/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf')
LICENCE = pathlib.Path('/usr/share/doc/fonts-liberation/copyright')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')


def generate(out):
    from PIL import Image, ImageDraw, ImageFont, __version__ as pillow_version

    out.mkdir(parents=True, exist_ok=True)
    # Use only this installed OFL package, never download or silently substitute fonts.
    license_text = LICENCE.read_text()
    assert 'Files: *\n' in license_text and 'License: SIL-OFL-1.1' in license_text
    assert ImageFont.truetype(str(FONT), 24).getname() == ('Liberation Sans', 'Regular')
    provenance = {
        'generated_assets_licence': 'MIT OR Apache-2.0',
        'generator': 'scripts/critical-text/fixtures.py',
        'pillow_version': pillow_version,
        'font': {'file': FONT.name, 'sha256': sha(FONT), 'licence': 'SIL-OFL-1.1',
                 'licence_evidence': str(LICENCE), 'licence_evidence_sha256': sha(LICENCE),
                 'source': 'installed fonts-liberation package; no downloads; font not redistributed'},
        'construction': 'procedural app capture, scientific figure and raster document page; exact image-bound source exports',
    }
    cases = []
    specs = [
        ('app-decimal', 'app', 'Total: 12.50', 'Total: 1250', 24, '.'),
        ('app-currency', 'app', '12.50 €', '12.50€', 24, None),
        ('figure-minus', 'scientific_figure', 'Value: −0.5', 'Value: 0.5', 24, '−'),
        ('document-warning', 'document_page', 'Warning: Do NOT mix', 'Warning: Do mix', 12, None),
    ]
    for name, domain, before, after, size, erase in specs:
        image = Image.new('RGB', (900, 700), 'white')
        draw = ImageDraw.Draw(image)
        heading = ImageFont.truetype(str(FONT), 30)
        body = ImageFont.truetype(str(FONT), 16)
        if domain == 'app':
            draw.rectangle((0, 0, 899, 65), fill=(35, 50, 70))
            draw.text((24, 18), 'Summary', font=heading, fill='white')
            draw.rectangle((40, 100, 850, 600), outline=(170, 170, 170), width=2)
            draw.text((70, 130), 'Review the selected values', font=body, fill='black')
        elif domain == 'scientific_figure':
            draw.text((50, 24), 'Measured response', font=heading, fill='black')
            draw.line((80, 500, 800, 500), fill='black', width=2)
            draw.line((80, 120, 80, 500), fill='black', width=2)
            points = [(100 + i * 60, 450 - (i * 29) % 260) for i in range(11)]
            draw.line(points, fill=(30, 90, 180), width=2)
            for x, y in points:
                draw.ellipse((x-3, y-3, x+3, y+3), fill=(30, 90, 180))
        else:
            draw.text((50, 24), 'Handling instructions', font=heading, fill='black')
            for i in range(7):
                draw.text((50, 100 + i * 25), 'Follow the listed steps before use.', font=body, fill='black')
        font = ImageFont.truetype(str(FONT), size)
        x, y = 100, 550
        box = [x-6, y-6, int(draw.textlength(before, font=font)) + 16, size+18]
        draw.text((x, y), before, font=font, fill='black')
        # Displayed tiny warning contrast is measured directly.
        variants = [('same', image.copy(), before, 0)]
        bad = image.copy()
        bd = ImageDraw.Draw(bad)
        if erase:
            offset = before.index(erase)
            left = x + draw.textlength(before[:offset], font=font)
            bounds = bd.textbbox((left, y), erase, font=font)
            bd.rectangle(bounds, fill='white')
        else:
            bd.rectangle((box[0], box[1], box[0]+box[2]-1, box[1]+box[3]-1), fill='white')
            bd.text((x, y), after, font=font, fill='black')
        variants.append(('edited', bad, after, 1))
        accepted = [before]
        if name == 'app-currency':
            alternate = '12.50\u00a0€'
            accepted.append(alternate)
            typ = image.copy()
            td = ImageDraw.Draw(typ)
            td.rectangle((box[0], box[1], box[0]+box[2]-1, box[1]+box[3]-1), fill='white')
            td.text((x, y), alternate, font=font, fill='black')
            variants.append(('typography', typ, alternate, 0))
        if name == 'figure-minus':
            alternate = 'Value: -0.5'
            accepted.append(alternate)
            typ = image.copy()
            td = ImageDraw.Draw(typ)
            td.rectangle((box[0], box[1], box[0]+box[2]-1, box[1]+box[3]-1), fill='white')
            td.text((x, y), alternate, font=font, fill='black')
            variants.append(('typography', typ, alternate, 0))
        if name == 'document-warning':
            faded = image.copy()
            fd = ImageDraw.Draw(faded)
            fd.rectangle((box[0], box[1], box[0]+box[2]-1, box[1]+box[3]-1), fill='white')
            fd.text((x, y), before, font=font, fill=(210, 210, 210))
            variants.append(('faded', faded, before, 1))
        folder = out / name
        folder.mkdir(exist_ok=True)
        image.save(folder / 'baseline.png', compress_level=9)
        policy = {'schema': 'saccade-critical-text-policy.v1', 'dimensions': [900,700],
                  'minimum_ocr_confidence': 80,
                  'regions': [{'id': 'critical', 'rect_px': box, 'accepted_text': accepted,
                               'legibility': {'minimum_contrast': 4.5, 'minimum_x_height_px': 3,
                                              'minimum_sharpness': .35, 'minimum_stroke_px': 1}}]}
        write(folder / 'policy.json', policy)

        def source(path, content):
            return {'schema':'saccade-ui-source.v1', 'capture_sha256':sha(path),
                    'dimensions':[900,700], 'kind':'dom', 'producer':{'generator':provenance['generator'], 'scope':'declared critical region'},
                    'complete':True, 'nodes':[{'id':'critical', 'text':content, 'bounds':box}]}
        write(folder / 'baseline-source.json', source(folder / 'baseline.png', before))
        for variant, pixels, content, exit_code in variants:
            pixels.save(folder / f'{variant}.png', compress_level=9)
            write(folder / f'{variant}-source.json', source(folder / f'{variant}.png', content))
            cases.append({'name':f'{name}/{variant}', 'domain':domain,
                          'baseline':f'{name}/baseline.png', 'candidate':f'{name}/{variant}.png',
                          'a_source':f'{name}/baseline-source.json', 'b_source':f'{name}/{variant}-source.json',
                          'policy':f'{name}/policy.json', 'mean_threshold':.02,
                          'mean_exit':0, 'critical_exit':exit_code,
                          'expected_reason':'critical_pixel_threshold_failed' if variant == 'faded' else
                                            'critical_string_mismatch' if exit_code else None})
    files = {str(p.relative_to(out)):sha(p) for p in sorted(out.rglob('*')) if p.is_file() and p.name != 'fixtures.json'}
    write(out / 'fixtures.json', {'provenance':provenance, 'files':files, 'cases':cases})


def qualify(pack, binary, receipts):
    receipts.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((pack / 'fixtures.json').read_text())
    for name, expected in manifest['files'].items():
        assert sha(pack / name) == expected, f'fixture changed: {name}'
    results = []
    for case in manifest['cases']:
        paths = {key:str((pack / case[key]).resolve()) for key in ('baseline','candidate','policy','a_source','b_source')}
        argv = [str(binary), 'critical-text', paths['baseline'], paths['candidate'], '--policy', paths['policy'],
                '--a-source', paths['a_source'], '--b-source', paths['b_source'], '--json']
        critical = subprocess.run(argv, capture_output=True, text=True)
        report = json.loads(critical.stdout)
        assert critical.returncode == case['critical_exit'], (case['name'], critical.returncode, report, critical.stderr)
        assert report['state'] == ('fail' if case['critical_exit'] else 'pass'), case['name']
        if case['expected_reason']:
            assert case['expected_reason'] in report['regions'][0]['reasons'], case['name']
        assert report['image_sha256'] == [sha(pack / case['baseline']), sha(pack / case['candidate'])]
        # An actual FLIP mean-only gate, no percentile/cluster constraints; every known edit must pass.
        mean_out = receipts / case['name'].replace('/', '-')
        mean = subprocess.run([str(binary), 'compare', paths['baseline'], paths['candidate'],
                               '--threshold', str(case['mean_threshold']), '--out', str(mean_out), '--json'],
                              capture_output=True, text=True)
        assert mean.returncode == case['mean_exit'], (case['name'], mean.returncode, mean.stdout, mean.stderr)
        mean_report_path = mean_out / 'saccade-report.v1.json'
        mean_report = json.loads(mean_report_path.read_text())
        entry, = mean_report['entries']
        assert entry['metric_used'] == 'mean' and entry['status'] == 'pass'
        assert entry['value'] <= case['mean_threshold']
        if case['critical_exit']:
            assert entry['metrics']['mean'] > 0, 'known edit not measured'
            assert sha(pack / case['baseline']) != sha(pack / case['candidate']), 'known edit absent'
        write(receipts / (case['name'].replace('/', '-') + '.json'), report)
        results.append({'case':case['name'], 'critical_exit':critical.returncode,
                        'mean_exit':mean.returncode, 'flip_mean':entry['metrics']['mean'],
                        'mean_report_sha256':sha(mean_report_path), 'critical_report_sha256':sha(receipts / (case['name'].replace('/', '-') + '.json'))})
        print(f"PASS {case['name']}: mean={mean.returncode}, critical={critical.returncode}")
    write(receipts / 'qualification.json', {'manifest_sha256':sha(pack / 'fixtures.json'), 'binary_sha256':sha(binary), 'results':results})


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out', type=pathlib.Path, help='generate the pack here (requires installed OFL font and Pillow)')
    ap.add_argument('--pack', type=pathlib.Path, default=ROOT / 'testdata/critical-text')
    ap.add_argument('--binary', type=pathlib.Path, help='assert the declared gates without generating/downloading')
    ap.add_argument('--receipts', type=pathlib.Path, help='empty report destination for qualification')
    args = ap.parse_args()
    if args.out:
        generate(args.out)
    if args.binary:
        if not args.receipts:
            ap.error('--binary requires --receipts')
        qualify(args.out or args.pack, args.binary.resolve(), args.receipts)
    elif not args.out:
        ap.error('pass --out or --binary')


if __name__ == '__main__':
    main()
