#!/usr/bin/env python3
"""Generated cross-domain CLI proof. Run fixture example first; no downloads."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--saccade', required=True)
parser.add_argument('--fixtures', required=True, type=Path)
args = parser.parse_args()

def run(argv, code=0):
    p = subprocess.run([args.saccade, *map(str, argv)], capture_output=True, text=True)
    assert p.returncode == code, (argv, p.returncode, p.stdout, p.stderr)
    return json.loads(p.stdout)

assert 'print' in run(['doctor', '--json'])['features']
schema = run(['schema', 'get', 'saccade-print.v1'])
assert schema['$id'] == 'saccade-print.v1'
with tempfile.TemporaryDirectory(prefix='saccade-print-proof-') as tmp:
    for domain in ['packaging-label', 'magazine-page']:
        a = args.fixtures / f'{domain}.tif'
        b = args.fixtures / f'{domain}-shift.tif'
        for name, candidate in [('identity', a), ('shift', b)]:
            out = Path(tmp) / f'{domain}-{name}'
            value = run(['print', a, candidate, '--out', out, '--tac-limit', 300,
                         '--dpi', 300, '--output-profile', args.fixtures / 'synthetic.icc', '--json'])
            assert value == json.loads((out / 'saccade-print.v1.json').read_text())
            assert value['report_id'].startswith('sha256:')
            delta = value['delta_e2000']['max']
            assert delta == 0 if name == 'identity' else 2.4 < delta < 2.5
            assert value['sides']['candidate']['over_limit_pixels'] == 50
            assert value['sides']['candidate']['registration_sensitive_marks']['candidates']
            assert value['sides']['candidate']['out_of_gamut']['state'] == 'measured'
            assert len(list(out.glob('*.png'))) == 9
            print(f'{domain}/{name}: delta={delta:.10f}, TAC=50, linked report and 9 maps')
    p = subprocess.run([args.saccade, 'print', str(args.fixtures / 'rgb.png'),
                        str(args.fixtures / 'rgb.png'), '--out', str(Path(tmp) / 'rgb'),
                        '--dpi', '300', '--tac-limit', '300', '--json'], capture_output=True, text=True)
    assert p.returncode == 2 and 'not_print_input' in p.stdout + p.stderr, (p.stdout, p.stderr)
print('PRINT CLI PASS')
