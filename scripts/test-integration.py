#!/usr/bin/env python3
"""Offline cross-wave discovery, registration and capture-failure regressions."""
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from PIL import Image

binary = str(Path(sys.argv[1]).resolve())

def cli(args, expected=0):
    result = subprocess.run([binary, *map(str, args)], capture_output=True, text=True)
    assert result.returncode == expected, (args[0], result.returncode, result.stderr)
    return json.loads(result.stdout)

with tempfile.TemporaryDirectory(prefix='saccade-integration-') as tmp:
    root = Path(tmp)
    image = root / 'input.png'
    pixels = Image.new('RGB', (64, 64))
    pixels.putdata([((x * 17 + y * 29) % 256, (x * 3) % 256, (y * 5) % 256)
                    for y in range(64) for x in range(64)])
    pixels.save(image)
    catalogue = cli(['capabilities', '--json'])
    commands = ';'.join(f['command'] for f in catalogue['families'])
    for command in ['review explain', 'review audit-mask', 'review check-ui', 'sweep compare',
                    'imgtune search', 'design compare', 'notify']:
        assert command in commands, command
    assert {'assist', 'products', 'imgtune-avif'} <= set(catalogue['compiled_features'])
    for command, schema in [('assess', 'saccade-assess.v1'), ('inspect-image', 'saccade-inspect-image.v1')]:
        out = root / command
        cli([command, image, '--out', out, '--json'])
        report = json.loads((out / (schema + '.json')).read_text())
        assert 'review explain' in report['related_commands']
        assert 'sweep plan|compare' in report['related_commands']
        assert report['verdict'] == 'unknown'
    urls = root / 'urls.txt'
    urls.write_text('https://example.org/page/1\n')
    manifest = root / 'manifest.json'
    cli(['sweep', 'plan', '--urls', urls, '--before-origin', 'https://example.org',
         '--after-origin', 'https://example.com', '--out', manifest, '--json'])
    pages = json.loads(manifest.read_text())['pages']
    receipts = []
    for page in pages:
        for side in ['before', 'after']:
            receipts.append({'id': page['id'], 'side': side, 'status': 'captured',
                             'path': image.name, 'sha256': hashlib.sha256(image.read_bytes()).hexdigest(),
                             'final_url': page[side], 'timing_ms': 1, 'error': None})
    captures = root / 'captures.json'
    document = {'schema': 'saccade-sweep-captures.v1',
                'manifest_sha256': hashlib.sha256(manifest.read_bytes()).hexdigest(), 'receipts': receipts}
    captures.write_text(json.dumps(document))
    out = root / 'sweep'
    value = cli(['sweep', 'compare', manifest, '--captures', captures, '--align', 'none',
                 '--resample', 'reference', '--out', out, '--json'])
    assert value['verdict'] == 'pass'
    evidence = json.loads((out / 'comparison/saccade-registration.v1.json').read_text())
    assert evidence['pipeline']['align'] == 'none'
    assert evidence['entries'][0]['artifacts']['geometry_inclusion']
    document['receipts'][1]['status'] = 'failed'
    document['receipts'][1]['error'] = 'generated capture failure'
    captures.write_text(json.dumps(document))
    value = cli(['sweep', 'compare', manifest, '--captures', captures, '--align', 'none',
                 '--out', root / 'failed-sweep', '--json'], expected=1)
    assert value['capture_failures'] and value['verdict'] == 'regression'
    script = r'''
const {compareFiles,resolveOptions} = require('./integrations/playwright/matcher.cjs');
const assert = require('node:assert/strict');
(async () => {
 const [binary,image,out] = process.argv.slice(1);
 const result = await compareFiles(image,image,out,{binary,align:'none',resample:'reference'});
 assert.equal(result.pass,true); assert.equal(result.result.schema,'saccade-general-result.v1');
 assert.throws(()=>resolveOptions({project:{}},{align:'similarity',masks:[{reason:'clock',rect:[0,0,.1,.1]}]}));
 assert.throws(()=>resolveOptions({project:{}},{resample:'common'}));
})().catch(e=>{console.error(e);process.exitCode=1;});
'''
    subprocess.run(['node', '-e', script, binary, str(image), str(root / 'matcher')], check=True)
print('PASS integrated discovery, matcher/sweep registration and retained capture failure')
