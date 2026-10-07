#!/usr/bin/env python3
"""Deterministic product-photo-style and map derivative proofs; no downloads."""
import argparse
import hashlib
import json
import shutil
from pathlib import Path
import subprocess
from PIL import Image, ImageDraw, ImageFont, __version__ as PIL_VERSION

FACE_LIMIT = ('Protect detected faces only. No detection does not certify that no face is present; '
              'small, occluded or out-of-domain faces may be missed. This is not identity recognition.')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--font', type=Path, required=True, help='locally installed OFL NotoSans-Regular.ttf')
    parser.add_argument('--bin', type=Path, help='also run and assert constructed truth')
    args = parser.parse_args()
    if args.font.name != 'NotoSans-Regular.ttf':
        raise ValueError('fixture provenance requires the locally installed OFL NotoSans-Regular.ttf')
    binary = (shutil.which(str(args.bin)) or str(args.bin.resolve())) if args.bin else None
    args.out.mkdir(parents=True, exist_ok=True)
    inputs = args.out / 'inputs'
    inputs.mkdir(exist_ok=True)
    font = ImageFont.truetype(str(args.font), 32)
    product = Image.new('RGB', (640, 480))
    # Deterministic shading, a protected face and a labelled package. Constructed
    # photo-style pixels do not qualify natural-image face-detector recall.
    for y in range(480):
        for x in range(640):
            shade = int(238 - 30 * y / 480 - 8 * x / 640)
            product.putpixel((x, y), (shade, shade - 2, shade - 6))
    draw = ImageDraw.Draw(product)
    draw.ellipse((45, 75, 205, 255), fill=(105, 74, 50))
    draw.ellipse((65, 95, 185, 245), fill=(210, 165, 125))
    draw.ellipse((85, 145, 98, 158), fill=(40, 30, 24))
    draw.ellipse((149, 145, 162, 158), fill=(40, 30, 24))
    draw.line((125, 164, 118, 192, 132, 192), fill=(156, 111, 80), width=4)
    draw.arc((98, 180, 154, 220), 0, 180, fill=(105, 55, 50), width=4)
    draw.rounded_rectangle((40, 252, 210, 465), 24, fill=(70, 102, 137))
    draw.ellipse((260, 398, 510, 450), fill=(170, 165, 157))
    draw.rounded_rectangle((270, 182, 500, 424), 15, fill=(35, 106, 94))
    draw.rectangle((278, 230, 492, 380), fill='white')
    draw.text((330, 278), 'SAFE', font=font, fill=(0, 0, 0))
    product.save(inputs / 'product.png')
    tile = Image.new('RGB', (512, 384), (225, 236, 218))
    draw = ImageDraw.Draw(tile)
    draw.polygon([(0, 180), (200, 100), (512, 200), (512, 240), (200, 142), (0, 220)], fill=(126, 189, 216))
    for offset in range(40, 512, 90):
        draw.line((offset, 0, offset + 100, 384), fill='white', width=12)
        draw.line((0, offset // 2, 512, offset // 2 + 80), fill=(248, 247, 237), width=10)
    draw.ellipse((90, 70, 120, 100), fill=(150, 42, 40))
    draw.rectangle((120, 220, 460, 355), fill='white')
    draw.text((170, 266), 'RIVER EAST', font=ImageFont.truetype(str(args.font), 28), fill=(0, 0, 0))
    tile.save(inputs / 'map.png')
    def write(name, value):
        (inputs / name).write_text(json.dumps(value, indent=2) + '\n')
    write('product.json', dict(schema='saccade-derivatives.v1',
          subjects=[dict(label='portrait subject', rect_px=[60, 80, 140, 180]), dict(label='package', rect_px=[270, 182, 230, 242])],
          text_regions=[dict(label='package label', rect_px=[304, 255, 160, 100])],
          derivatives=[dict(id='original display', display_size=[640, 480]),
                       dict(id='thumbnail', display_size=[96, 72]),
                       dict(id='face cut', display_size=[520, 480], crop_px=[120, 0, 520, 480]),
                       dict(id='face excluded', display_size=[400, 480], crop_px=[240, 0, 400, 480])]))
    write('map.json', dict(schema='saccade-derivatives.v1',
          subjects=[dict(label='location marker', rect_px=[90, 70, 30, 30])],
          text_regions=[dict(label='map label', rect_px=[144, 244, 292, 88])],
          derivatives=[dict(id='full tile', display_size=[512, 384]), dict(id='tile thumbnail', display_size=[96, 72]),
                       dict(id='label cut', display_size=[256, 384], crop_px=[0, 0, 256, 384])]))
    source_sha = hashlib.sha256((inputs / 'product.png').read_bytes()).hexdigest()
    replay = dict(schema='saccade-faces.v1', image_sha256=source_sha, image_size=[640, 480],
                  faces=[dict(bbox=dict(x=60., y=80., width=140., height=180.), score=1., landmarks=[])],
                  provenance=dict(model_id='generated-face-box', version='generated-v1', artifact_sha256=[], runtime='replay',
                                  input_resolution=[0, 0], resolution_handling='original-pixels', source_parity=False), limitations=FACE_LIMIT)
    write('faces-replay.json', replay)
    provenance = dict(generator='scripts/derivatives/fixtures.py', generator_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), license='MIT OR Apache-2.0',
                      imagery='Procedural shaded product/portrait scene and map; no downloaded imagery or real people.',
                      pillow_version=PIL_VERSION, font=dict(name=args.font.name, license='SIL Open Font License 1.1',
                      sha256=hashlib.sha256(args.font.read_bytes()).hexdigest()),
                      limitations='Synthetic face receipt tests geometry and source binding only; no face-model inference or human readability qualification.')
    provenance['files'] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(inputs.iterdir()) if p.is_file()}
    if binary:
        provenance['binary_sha256'] = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    (args.out / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
    if not args.bin:
        return
    results = {}
    for domain, expected_rows in [('product', 4), ('map', 3)]:
        command = [binary, 'derivative-sheet', str(inputs / f'{domain}.png'), str(inputs / f'{domain}.json'),
                   '--out', str(args.out / f'{domain}-review'), '--json']
        result = subprocess.run(command, capture_output=True, text=True)
        assert result.returncode == 1, (domain, result.returncode, result.stdout, result.stderr)
        report = json.loads(result.stdout)
        rows = report['rows']
        assert len(rows) == expected_rows
        assert rows[0]['verdict'] == 'pass', (domain, rows[0])
        assert rows[1]['text_legibility']['state'] == 'illegible', (domain, rows[1])
        assert rows[-1]['verdict'] == 'fail'
        if domain == 'product':
            assert rows[2]['subject_preservation']['subjects'][0]['state'] == 'cut'
            assert rows[3]['subject_preservation']['subjects'][0]['state'] == 'excluded'
        verified = subprocess.run([binary, 'manifest', 'verify', str(args.out / f'{domain}-review'), '--json'], capture_output=True, text=True)
        assert verified.returncode == 0, verified.stdout
        results[domain] = dict(exit=result.returncode, rows=len(rows), full=rows[0]['verdict'], tiny=rows[1]['text_legibility']['state'],
                               faces=report['faces']['state'], report_id=report['report_id'])
    d = json.loads((inputs / 'product.json').read_text())
    d['subjects'] = []
    write('replay-declaration.json', d)
    result = subprocess.run([binary, 'derivative-sheet', str(inputs / 'product.png'), str(inputs / 'replay-declaration.json'),
                             '--faces-report', str(inputs / 'faces-replay.json'), '--out', str(args.out / 'face-replay-review'), '--json'], capture_output=True, text=True)
    assert result.returncode == 1, result.stdout
    r = json.loads(result.stdout)
    assert r['faces']['report']['provenance']['runtime'] == 'replay'
    assert r['rows'][2]['crop_safety']['state'] == 'unsafe'
    results['face_replay'] = dict(exit=1, geometry='cut face fails', native_model_inference=False)
    (args.out / 'proof-results.json').write_text(json.dumps(results, indent=2) + '\n')
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
