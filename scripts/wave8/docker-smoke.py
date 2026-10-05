#!/usr/bin/env python3
"""Heavy container wire smoke, explicit local image only, always remove the container."""
import json
from pathlib import Path
import subprocess
import tempfile
import time
import urllib.request

with tempfile.TemporaryDirectory() as directory:
    cid = subprocess.check_output(['docker', 'run', '-d', '--network=host', '--read-only', '--tmpfs', '/tmp',
        '-v', f'{directory}:/data:ro', 'saccade-wave8-gate:local', 'serve', '/data', '--api', '--port', '17878',
        '--api-model-dir', '/models']).decode().strip()
    try:
        for _ in range(60):
            try:
                with urllib.request.urlopen('http://127.0.0.1:17878/v1/health', timeout=2) as response:
                    value = json.load(response)
                assert value['schema'] == 'saccade-api-health.v1' and value['status'] == 'ok'
                assert subprocess.check_output(['docker', 'exec', cid, 'id', '-u']).decode().strip() == '10001'
                break
            except (OSError, subprocess.CalledProcessError):
                time.sleep(1)
        else:
            raise AssertionError('container did not become healthy in 60 seconds')
    finally:
        subprocess.run(['docker', 'rm', '-f', cid], check=True)
print('Unprivileged container loopback health: PASS')
