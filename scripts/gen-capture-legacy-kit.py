#!/usr/bin/env python3
"""Generate two fictional historical formats from the procedural capture kit."""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'examples/capture-legacy'


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + '\n').encode()


def generate():
    native = json.loads((ROOT / 'examples/capture-kit/renderer-valid.json').read_text())
    plan = native['expected'][0]
    attempt = native['acquisitions'][0]
    source = lambda path, **kw: dict(path=path, **kw)
    top = {name: source(name) for name in ('producer', 'run_id', 'completion')}
    expected = {name: source(name) for name in plan}
    acquired = {name: source(name) for name in attempt if name not in ('details', 'error')}
    first = dict(schema='saccade-capture-legacy-map.v1', record='ledger.json',
                 hash_policy='recorded', absent='unavailable', fields=top,
                 expected=dict(source=source('planned'), fields=expected),
                 acquisitions=dict(source=source('attempted'), fields=acquired),
                 retry_records=source('retries'))
    ledger = {key: native[key] for key in top}
    ledger.update(planned=native['expected'], attempted=native['acquisitions'],
                  retries=[dict(slot=attempt['id'], attempt=0, status='failed', error='synthetic retry')])
    second = dict(schema='saccade-capture-legacy-map.v1', record='journal.json',
                  hash_policy='recorded', absent='unavailable',
                  fields=dict(producer=source('family', file='session.json'),
                              run_id=source('session.key', file='session.json'),
                              completion=source('session.end', file='session.json')),
                  expected=dict(source=source('requests'), fields={
                      'id': source('slot'), 'settings': source('configuration'),
                      'fingerprint': source('identity'), 'clock_domain': source('timer')}),
                  acquisitions=dict(source=source('results'), fields={
                      'id': source('slot'), 'status': source('outcome'),
                      'settings': source('observed.configuration'),
                      'fingerprint': source('observed.identity'),
                      'clock': source('observed.window'),
                      'image.path': source('artifact.name'),
                      'image.sha256': source('artifact.digest')}))
    translated_attempt = 'saved'
    second['acquisitions']['fields']['status']['values'] = {'saved': 'captured', 'error': 'failed', 'omitted': 'skipped'}
    journal = dict(requests=[dict(slot=plan['id'], configuration=plan['settings'],
                                 identity={k: v for k, v in plan['fingerprint'].items() if k != 'schema'}, timer=plan['clock_domain'])],
                   results=[dict(slot=attempt['id'], outcome=translated_attempt,
                                 observed=dict(configuration=attempt['settings'],
                                               identity={k: v for k, v in attempt['fingerprint'].items() if k != 'schema'}, window=attempt['clock']),
                                 artifact=dict(name='image.png', digest=attempt['image']['sha256']))])
    files = {
        'atlas/map.json': encoded(first), 'atlas/ledger.json': encoded(ledger),
        'beacon/map.json': encoded(second), 'beacon/journal.json': encoded(journal),
        'beacon/session.json': encoded(dict(family='renderer', session=dict(key=native['run_id'], end='complete'))),
        'provenance.json': encoded(dict(generator='scripts/gen-capture-legacy-kit.py',
                                       license='CC0-1.0', scope='Fictional, procedural offline fixtures; synthetic declarations, no producer execution.')),
    }
    image = (ROOT / 'examples/capture-kit/image.png').read_bytes()
    files.update({f'{name}/image.png': image for name in ('atlas', 'beacon')})
    # Ship TOML as well as JSON, exercising the exact same mapping semantics.
    toml = ['schema = "saccade-capture-legacy-map.v1"', 'record = "ledger.json"',
            'hash_policy = "recorded"', 'absent = "unavailable"', '']
    for key, src in top.items():
        toml += [f'[fields.{key}]', f'path = "{src["path"]}"', '']
    for key, rows in [('expected', first['expected']), ('acquisitions', first['acquisitions'])]:
        toml += [f'[{key}.source]', f'path = "{rows["source"]["path"]}"', '']
        for field, src in rows['fields'].items():
            toml += [f'[{key}.fields."{field}"]', f'path = "{src["path"]}"', '']
    toml += ['[retry_records]', 'path = "retries"', '']
    files['atlas/map.toml'] = '\n'.join(toml).encode()
    return files


def main():
    args = argparse.ArgumentParser()
    args.add_argument('--check', action='store_true')
    check = args.parse_args().check
    for name, data in generate().items():
        path = OUT / name
        if check:
            if not path.is_file() or path.read_bytes() != data:
                raise SystemExit(f'stale fixture: {name}')
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
    print('capture legacy fixtures: PASS')


if __name__ == '__main__':
    main()
