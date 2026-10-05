#!/usr/bin/env python3
"""Run the Waves 4–6/host CI gates sequentially in one admitted batch."""
import argparse
import json
import os
import shlex
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--evidence', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
os.chdir(root)
args.evidence.mkdir(parents=True, exist_ok=False)
env = os.environ.copy()
env.update(CARGO_TARGET_DIR='/mnt/linux-extra/moss-cargo-targets/codex-saccade-integ',
           CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='4',
           RUSTC_WRAPPER='', CARGO_BUILD_RUSTC_WRAPPER='', SYSTEM_DEPS_DAV1D_BUILD_INTERNAL='never')
binary = env['CARGO_TARGET_DIR'] + '/debug/saccade'
commands = [
    ('msrv-default', ['cargo', '+1.88', 'check', '--offline', '--locked', '-p', 'saccade']),
    ('msrv-compression-reference', ['cargo', '+1.88', 'test', '--offline', '--locked', '-p', 'saccade', '--test', 'quality_reference']),
    ('wave4', ['bash', 'scripts/gates-wave4.sh']),
    ('wave5', ['bash', 'scripts/gates-wave5.sh']),
    ('wave6', ['bash', 'scripts/gates-wave6.sh']),
    ('ci-fmt', ['cargo', 'fmt', '--all', '--check']),
    ('ci-clippy-default', ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings']),
    ('ci-tests', ['cargo', 'test', '--workspace', '--locked', '--no-fail-fast']),
]
for name, package, flags in [
        ('core-minimal', 'saccade-core', ['--no-default-features']),
        ('cli-default', 'saccade', []), ('cli-all', 'saccade', ['--all-features'])]:
    commands.extend([
        ('ci-build-' + name, ['cargo', 'build', '--locked', '-p', package, *flags]),
        ('ci-clippy-' + name, ['cargo', 'clippy', '--locked', '-p', package, '--all-targets', *flags, '--', '-D', 'warnings']),
        ('ci-test-' + name, ['cargo', 'test', '--locked', '-p', package, *flags]),
    ])
commands.extend([
    ('integration-regressions', ['python3', 'scripts/test-integration.py', binary]),
    ('genericity', ['bash', 'scripts/check-genericity.sh']),
    ('ci-packages', ['python3', 'scripts/check-packages.py']),
    ('ci-package-verify', ['cargo', 'package', '--workspace', '--all-features', '--locked']),
    ('release-check', ['bash', 'scripts/release-check.sh']),
])
receipts = []
for name, command in commands:
    log = args.evidence / (name + '.log')
    print('START', name, shlex.join(command), flush=True)
    start = time.time()
    with log.open('w') as output:
        free = os.statvfs('/mnt/linux-extra')
        if free.f_bavail * free.f_frsize < 25 * 1024**3:
            output.write('REFUSED: less than 25 GiB free on /mnt/linux-extra\n')
            code = 75
        else:
            try:
                code = subprocess.run(command, env=env, stdout=output, stderr=subprocess.STDOUT,
                                      timeout=3600).returncode
            except subprocess.TimeoutExpired:
                code = 124
                output.write('\nTIMEOUT after 3600 seconds\n')
    receipts.append({'name': name, 'command': shlex.join(command), 'exit_code': code,
                     'elapsed_seconds': round(time.time() - start, 3), 'log': str(log)})
    (args.evidence / 'receipts.json').write_text(json.dumps(receipts, indent=2) + '\n')
    print('GATE', name, 'PASS' if code == 0 else 'FAIL', 'exit=' + str(code), flush=True)
raise SystemExit(int(any(r['exit_code'] for r in receipts)))
