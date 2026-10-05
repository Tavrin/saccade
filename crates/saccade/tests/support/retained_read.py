"""Controlled POSIX replacement regression for exact report-byte binding."""
import hashlib
import json
import multiprocessing
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import os

binary, report_file = sys.argv[1:]
a = Path(report_file).read_bytes()
bv = json.loads(a)
bv['entries'][0]['value'] = 0.9
bv['entries'][0]['status'] = 'fail'
bv['totals']['pass'] = 0
bv['totals']['fail'] = 1
for key in ['mean', 'max', 'p50', 'p95', 'p99']:
    bv['entries'][0]['metrics'][key] = 0.9
for key in ['frac_above_0_1', 'frac_above_0_5']:
    bv['entries'][0]['metrics'][key] = 1.0
b = json.dumps(bv).encode()


def replace_on_second_read(pipe):
    with open(pipe, 'wb', buffering=0) as writer:
        writer.write(a)
    # Closing the writer ends the first retained read. Opening another writer
    # blocks until a second reader opens; that reader receives replacement B.
    time.sleep(0.1)
    with open(pipe, 'wb', buffering=0) as writer:
        writer.write(b)


with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    manifest = {'schema':'saccade-inventory.v1', 'expected':[{'case_id':'case','entry':'image.png','required':True}],
                'supplied':[{'case_id':'case','entry':'image.png','state':'captured','capture_sha256':'capture'}]}
    (root/'manifest.json').write_text(json.dumps(manifest))
    failures = []
    for mode in ['explain-grounded', 'inventory']:
        pipe = root / (mode + '.pipe')
        os.mkfifo(pipe)
        worker = multiprocessing.get_context('fork').Process(target=replace_on_second_read, args=(pipe,))
        worker.start()
        out = root/(mode+'.json')
        args = [binary, mode, '--report', str(pipe), '--out', str(out)]
        if mode == 'inventory':args += ['--manifest',str(root/'manifest.json')]
        try:
            result = subprocess.run(args, capture_output=True, timeout=5)
            assert result.returncode == 0, result
            value = json.loads(out.read_bytes())
            if mode == 'inventory':
                if value['report_sha256'] != hashlib.sha256(a).hexdigest():
                    failures.append('inventory bound its facts to replacement B')
            else:
                if value['catalog']['source_sha256'] != hashlib.sha256(a).hexdigest():
                    failures.append('grounded hash differs from retained A')
                for fact in value['catalog']['facts']:
                    if fact['kind'] == 'mean_flip' and fact['value'] != 0.0:
                        failures.append('grounded facts came from replacement B')
        finally:
            worker.terminate()
            worker.join(timeout=2)
    assert not failures, failures
