#!/usr/bin/env python3
"""Generated CC0 replay acceptance: offline compare + real OCR in a clean container.

Requires an existing compatible container image and already provisioned PP-OCRv5
artifacts. No network/image/model provisioning and no OCR accuracy qualification.
"""
import argparse
import hashlib
import json
import pathlib
import struct
import subprocess
import zlib

ROOT = pathlib.Path(__file__).resolve().parents[1]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def put(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def png(path, rows):
    def chunk(tag, data):
        body = tag + data
        return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body))
    raw = b''.join(b'\0' + bytes(c for p in row for c in p) for row in rows)
    path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack(
        '>IIBBBBB', len(rows[0]), len(rows), 8, 2, 0, 0, 0)) +
        chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b''))


def fixtures(out):
    # Hand-constructed 5x7 glyphs, no external font or source image.
    glyphs = {
        'V': ['10001'] * 4 + ['01010', '01010', '00100'],
        'A': ['01110', '10001', '10001', '11111', '10001', '10001', '10001'],
        'L': ['10000'] * 6 + ['11111'],
        'U': ['10001'] * 6 + ['01110'],
        'E': ['11111', '10000', '10000', '11110', '10000', '10000', '11111'],
        '2': ['01110', '10001', '00001', '00010', '00100', '01000', '11111'],
        '5': ['11111', '10000', '10000', '11110', '00001', '00001', '11110'],
        '.': ['00000'] * 5 + ['00110', '00110'],
        '-': ['00000'] * 3 + ['11111'] + ['00000'] * 3,
        ' ': ['00000'] * 7,
    }
    for name, text in [('reference', 'VALUE -2.5'), ('candidate', 'VALUE 2.5')]:
        rows = [[(255, 255, 255)] * 720 for _ in range(160)]
        for i, char in enumerate(text):
            for y, row in enumerate(glyphs[char]):
                for x, pixel in enumerate(row):
                    if pixel == '1':
                        for yy in range(8):
                            for xx in range(8):
                                rows[48 + y * 8 + yy][32 + i * 56 + x * 8 + xx] = (0, 0, 0)
        png(out / f'{name}.png', rows)
    for name, shifted in [('baseline', False), ('candidate-capture', True)]:
        directory = out / name
        directory.mkdir()
        rows = [[(245, 246, 248)] * 160 for _ in range(96)]
        for y in range(96):
            for x in range(160):
                if y < 16:
                    rows[y][x] = (40, 44, 52)
                elif 40 <= y < 64 and (40 if shifted else 32) <= x < (120 if shifted else 112):
                    rows[y][x] = (30, 100, 220)
        png(directory / 'panel.png', rows)
    put(out / 'PROVENANCE.json', {
        'license': 'CC0-1.0', 'source': 'procedural fixture generator in test-replay-pack.py',
        'domains': ['application screenshot', 'scientific numeric annotation'],
        'external_fonts': False, 'external_images': False,
        'generator_sha256': digest(pathlib.Path(__file__).read_bytes()),
    })


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--binary', required=True)
    ap.add_argument('--models', required=True, help='existing content-addressed OCR cache')
    ap.add_argument('--library', required=True, help='existing pinned ONNX Runtime library')
    ap.add_argument('--container-image', required=True, help='existing image; never pulled')
    ap.add_argument('--out', type=pathlib.Path, required=True, help='new evidence directory')
    args = ap.parse_args()
    binary = pathlib.Path(args.binary).resolve()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    fixtures(out)
    receipts = []

    def command(argv, expected=0, code=None):
        result = subprocess.run([str(v) for v in argv], cwd=out, capture_output=True, text=True)
        if result.returncode != expected:
            raise RuntimeError(f'exit {result.returncode}, expected {expected}: {result.stdout[-1000:]} {result.stderr[-1000:]}')
        value = json.loads(result.stdout)
        if code and value.get('errors', [{}])[0].get('code') != code:
            raise RuntimeError(f'expected {code}: {value}')
        receipts.append({'exit': result.returncode, 'result': value})
        return value

    compare = {'schema': 'saccade-replay-recipe.v1', 'run': {
        'operation': 'compare', 'a': 'baseline', 'b': 'candidate-capture',
        'threshold': 0.01, 'metric': 'mean', 'ppd': 67.0}}
    put(out / 'compare-recipe.json', compare)
    contract = json.loads((ROOT / 'crates/saccade-core/assets/paddle-ocr.json').read_text())
    contract['cache'] = str(pathlib.Path(args.models).resolve())
    put(out / 'registry.json', {'schema': 'saccade-model-registry.v1',
                              'models': [], 'contracts': {'ocr': contract}})
    text = {'schema': 'saccade-replay-recipe.v1', 'run': {
        'operation': 'text', 'a': 'reference.png', 'b': 'candidate.png',
        'a_source': None, 'b_source': None,
        'ocr': {'registry': 'registry.json', 'contract_id': 'ocr',
                'library': str(pathlib.Path(args.library).resolve())},
        'expect_text': [], 'readable_confidence': 80.0, 'moved_px': 3.0}}
    put(out / 'text-recipe.json', text)
    image = subprocess.run(['docker', 'image', 'inspect', args.container_image],
                           capture_output=True, text=True, check=True)
    image_id = json.loads(image.stdout)[0]['Id']
    for name in ['compare', 'text']:
        pack = out / f'{name}-pack'
        recorded = command([binary, 'replay', 'pack', f'{name}-recipe.json', '--out', pack, '--json'])
        report = json.loads((pack / ('run/saccade-report.v1.json' if name == 'compare' else 'run/saccade-text.v1.json')).read_text())
        if name == 'text' and not all(report['observations']):
            raise RuntimeError('real OCR proof requires observations on both images')
        # Only the pack is mounted: no source checkout, original inputs, config or model cache.
        replayed = command(['docker', 'run', '--rm', '--pull=never', '--network=none',
                            '--read-only', '--cap-drop=ALL', '--security-opt=no-new-privileges',
                            '--tmpfs', '/tmp:rw,exec,size=1g', '--mount', f'type=bind,src={pack},dst=/pack,readonly',
                            '--workdir', '/tmp', '--entrypoint', '/pack/tool/saccade', image_id,
                            'replay', 'verify', '/pack', '--json'])
        if replayed['report_id'] != recorded['report_id']:
            raise RuntimeError('container report identity differs')
    compare_pack = out / 'compare-pack'
    for path, code in [('inputs/a/panel.png', 'replay_input_changed'), ('recipe.json', 'replay_config_changed')]:
        file = compare_pack / path
        original = file.read_bytes()
        file.write_bytes(b'changed')
        command([binary, 'replay', 'verify', compare_pack, '--json'], 2, code)
        file.write_bytes(original)
    text_pack = out / 'text-pack'
    model = text_pack / 'ocr/models' / contract['detection']['sha256']
    original = model.read_bytes()
    model.write_bytes(b'changed')
    command([binary, 'replay', 'verify', text_pack, '--json'], 2, 'replay_model_changed')
    model.unlink()
    command([binary, 'replay', 'verify', text_pack, '--json'], 2, 'not_reproducible_here')
    model.write_bytes(original)
    put(out / 'acceptance.json', {
        'status': 'PASS', 'container_image_id': image_id, 'network': 'none',
        'only_pack_mounted': True, 'binary_sha256': digest(binary.read_bytes()),
        'proof_scope': 'reproduction and drift detection; not model accuracy',
        'receipts': receipts,
    })
    print(json.dumps({'status': 'PASS', 'proofs': len(receipts), 'receipt': str(out / 'acceptance.json')}))


if __name__ == '__main__':
    main()
