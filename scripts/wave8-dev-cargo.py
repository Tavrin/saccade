#!/usr/bin/env python3
"""One owned Cargo command, disk/time guard, source witness and retained receipts."""
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import threading
import time

root = Path(__file__).resolve().parents[1]
evidence = Path(os.environ.get('SACCADE_W8_EVIDENCE', '/mnt/linux-extra/moss-scratch/saccade-wave8'))
evidence.mkdir(parents=True, exist_ok=True)
lock = (evidence / 'cargo.lock').open('w')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
argv = sys.argv[1:]
if not argv:
    raise SystemExit('command required')
ident = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
log = evidence / f'{ident}.log'
target = os.environ.get('CARGO_TARGET_DIR', '/mnt/linux-extra/moss-cargo-targets/codex-saccade-w8')
env = dict(os.environ, CARGO_TARGET_DIR=target, CARGO_INCREMENTAL='0',
           CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='4')
diff = subprocess.check_output(['git', 'diff', '--binary', 'HEAD'], cwd=root)
(evidence / f'{ident}.patch').write_bytes(diff)
files = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard'], cwd=root).decode().splitlines()
source = {p: hashlib.sha256((root / p).read_bytes()).hexdigest() for p in files
          if (p.endswith(('.rs', '.toml')) or p == 'Cargo.lock') and (root / p).is_file()}
receipt = {'argv': argv, 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip(),
           'dirty_diff_sha256': hashlib.sha256(diff).hexdigest(), 'source_sha256': source,
           'target': target, 'log': str(log), 'started_utc': ident}
code = 75
if shutil.disk_usage('/mnt/linux-extra').free < 25 * 1024**3:
    log.write_text('DISK REFUSED: below 25 GiB; continue coding\n')
    print(log.read_text(), end='')
    receipt['result'] = 'disk-refused'
else:
    with log.open('w') as output:
        process = subprocess.Popen(['nice', '-n', '19', *argv], env=env, cwd=root,
                                   start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        def forward():
            for line in iter(process.stdout.readline, b''):
                text = line.decode(errors='replace')
                output.write(text)
                output.flush()
                print(text, end='', flush=True)
        reader = threading.Thread(target=forward)
        reader.start()
        started = time.monotonic()
        while process.poll() is None:
            if shutil.disk_usage('/mnt/linux-extra').free < 25 * 1024**3 or time.monotonic() - started > 900:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                receipt['result'] = 'disk-or-time-interrupted'
                break
            time.sleep(2)
        reader.join()
        if 'result' not in receipt:
            code = process.returncode
            receipt['result'] = 'pass' if code == 0 else 'fail'
receipt['exit_code'] = code
receipt['free_gib_after'] = shutil.disk_usage('/mnt/linux-extra').free / 1024**3
(evidence / f'{ident}.json').write_text(json.dumps(receipt, indent=2) + '\n')
raise SystemExit(code)
