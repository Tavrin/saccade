#!/usr/bin/env python3
"""Generate CC0 declared-case proofs; uses only the standard library and a local CLI."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import zlib


def png(path, family, a, b, anchor=False):
    # Entirely procedural pixels: responsive panels, an asset with hand-drawn
    # language marks, or scene-like geometry. No font or external texture.
    width = 96 if b in {'wide', 'fr', 'high'} else 48
    height = 48 if family != 'assets' or a == 'small' else 96
    pixels = [[(245, 245, 245) for _ in range(width)] for _ in range(height)]

    def rect(x0, y0, x1, y1, color):
        for y in range(max(y0, 0), min(y1, height)):
            for x in range(max(x0, 0), min(x1, width)):
                pixels[y][x] = color

    if family == 'pages':
        rect(2, 2, width - 2, 10, (40, 60, 100))
        rect(4, 14, width // 2, height - 4, (120, 160, 210))
        rect(width // 2 + 2, 14, width - 4, height - 4, (180, 200, 225))
    elif family == 'assets':
        rect(4, 4, width - 4, height - 4, (60, 140, 170))
        glyphs = {'E': ['111', '100', '110', '100', '111'],
                  'N': ['101', '111', '111', '111', '101'],
                  'F': ['111', '100', '110', '100', '100'],
                  'R': ['110', '101', '110', '101', '101']}
        for index, letter in enumerate(b.upper()):
            for y, row in enumerate(glyphs[letter]):
                for x, bit in enumerate(row):
                    if bit == '1':
                        rect(8 + index * 12 + x * 3, 12 + y * 3,
                             11 + index * 12 + x * 3, 15 + y * 3, (255, 255, 255))
    else:
        rect(0, height // 2, width, height, (80, 120, 80))
        rect(width // 4, 8, 3 * width // 4, 3 * height // 4, (130, 100, 65))
        if b == 'high':
            for x in range(width // 4, 3 * width // 4, 6):
                rect(x, 10, x + 2, 3 * height // 4, (160, 140, 110))
    if anchor:
        rect(0, 0, width, height, (0, 0, 0))

    def chunk(kind, payload):
        return struct.pack('!I', len(payload)) + kind + payload + struct.pack('!I', zlib.crc32(kind + payload))
    rows = b''.join(b'\0' + bytes(c for pixel in row for c in pixel) for row in pixels)
    path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', width, height, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))


def generate(out, binary):
    proofs = []
    for family, axes in [
        ('pages', {'page': ['start', 'details'], 'viewport': ['narrow', 'wide']}),
        ('assets', {'size': ['small', 'large'], 'language': ['en', 'fr']}),
        ('scenes', {'scene': ['interior', 'outdoor'], 'tier': ['low', 'high']}),
    ]:
        root = out / family
        root.mkdir(parents=True, exist_ok=False)

        def run(argv, code):
            result = subprocess.run([str(binary), *argv, '--json'], cwd=root, capture_output=True, text=True)
            if result.returncode != code:
                raise RuntimeError(f'{family} {argv}: expected {code}, got {result.returncode}: {result.stdout} {result.stderr}')
            return json.loads(result.stdout)

        def file(name):
            return {'path': name, 'sha256': hashlib.sha256((root / name).read_bytes()).hexdigest()}

        def reference(report):
            return {'report_id': report['report_id'], 'entry': report['entries'][0]['name']}

        keys = list(axes)
        cases = []
        observed_at = 0
        for i, a in enumerate(axes[keys[0]]):
            for j, b in enumerate(axes[keys[1]]):
                missing = (i, j) == (1, 0)
                refused = (i, j) == (1, 1)
                current_name = f'current-{a}-{b}.png'
                anchor_name = f'anchor-{a}-{b}.png'
                if not missing:
                    png(root / current_name, family, a, b)
                if not missing and not refused:
                    png(root / anchor_name, family, a, b, anchor=True)
                    latest_dir = f'latest-{a}-{b}'
                    drift_dir = f'drift-{a}-{b}'
                    run(['compare', current_name, current_name, '--out', latest_dir], 0)
                    run(['compare', anchor_name, current_name, '--out', drift_dir], 1)
                    latest = json.loads((root / latest_dir / 'saccade-report.v1.json').read_text())
                    drift = json.loads((root / drift_dir / 'saccade-report.v1.json').read_text())
                    observed_at = max(observed_at, latest['generated_at_unix'])
                cases.append({
                    'case_id': f'{family}-{a}-{b}', 'variants': {keys[0]: a, keys[1]: b}, 'required': True,
                    'baseline': None if missing else file(current_name),
                    'capture': None if missing or refused else file(current_name),
                    'approved_anchor': None if missing or refused else file(anchor_name),
                    'approved_at_unix': None if missing or refused else 1_000_000_000,
                    'last_good': None if missing or refused else file(current_name),
                    'refusal': 'producer refused acquisition' if refused else None,
                    'latest': None if missing or refused else reference(latest),
                    'anchor_comparison': None if missing or refused else reference(drift),
                    'last_good_comparison': None if missing or refused else reference(latest),
                })
        (root / 'cases.json').write_text(json.dumps({'schema': 'saccade-cases.v1', 'axes': axes, 'cases': cases}, indent=2) + '\n')
        run(['manifest', 'build', '.', '--cases', 'cases.json'], 0)
        result = run(['manifest', 'views', '.', '--out', 'views', '--group-by', keys[0], '--now-unix', str(observed_at + 1), '--max-age-seconds', '3600'], 1)
        report = json.loads((root / 'views/coverage.json').read_text())
        assert report['counts']['expected'] == 4 and report['counts']['measured'] == 2
        assert report['counts']['outcomes'] == {'measured': 2, 'missing': 1, 'refused': 1}
        assert len(report['groups']) == 2 and all(g['counts']['expected'] == 2 for g in report['groups'])
        missing_rows = [r for r in report['rows'] if r['baseline_state'] == r['capture_state'] == 'missing']
        assert len(missing_rows) == 1 and 'never_approved' in missing_rows[0]['health'] and 'no_recent_run' in missing_rows[0]['health']
        measured = [r for r in report['rows'] if r['coverage'] == 'measured']
        assert all(r['approved_anchor']['verdict'] == 'fail' and r['last_good']['verdict'] == 'pass' for r in measured)
        assert all('approved_anchor_old' in r['health'] and 'no_recent_run' not in r['health'] for r in measured)
        html = (root / 'views/index.html').read_text()
        assert all(c['case_id'] in html for c in cases)
        proofs.append({'family': family, 'counts': report['counts'], 'missing_case': missing_rows[0]['case']['case_id'], 'coverage_exit': 1, 'result': result})
    (out / 'PROVENANCE.json').write_text(json.dumps({
        'licence': 'CC0-1.0', 'source': 'Generated locally by scripts/gen-coverage-fixtures.py; no external assets, fonts, providers or downloads.',
        'generator_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'images': 'Procedural responsive panels, asset language marks from authored bitmap glyphs, and scene geometry by quality tier. RGB PNGs at declared dimensions; standard-library encoding.',
        'proofs': proofs,
    }, indent=2) + '\n')
    print('PASS: pages/viewports, assets/sizes/languages, scenes/tiers; explicit absence and refusal; anchor fail / last-good pass')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('out', type=Path, help='new output directory; existing data is never replaced')
    parser.add_argument('--bin', type=Path, required=True)
    args = parser.parse_args()
    if args.out.exists():
        parser.error('output directory must not exist')
    args.out.mkdir(parents=True)
    generate(args.out.resolve(), args.bin.resolve())


if __name__ == '__main__':
    main()
