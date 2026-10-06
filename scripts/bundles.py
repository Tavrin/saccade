#!/usr/bin/env python3
"""Release bundle helper: features, inventory and smoke test, driven by scripts/bundles.json."""
import hashlib
import json
import os
import pathlib
import struct
import subprocess
import sys
import tempfile
import zlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
CONFIG = json.loads((ROOT / 'scripts/bundles.json').read_text())


def bundle(name):
    try:
        return CONFIG['bundles'][name]
    except KeyError:
        raise SystemExit(f'unknown bundle {name!r}; known: {", ".join(CONFIG["bundles"])}')


def asset_stem(name, target):
    """Name stem of the bundle's own assets (the default bundle also keeps the legacy name)."""
    spec = bundle(name)
    return spec.get('also_published_as', spec['asset']).format(target=target)


def png(width=24, height=24, colour=(30, 60, 90)):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    rows = b''.join(b'\0' + bytes(colour) * width for _ in range(height))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b''))


def run(binary, args, env, expect=(0,)):
    # Outputs that default to the working directory land in the scratch HOME.
    done = subprocess.run([str(binary), *args], capture_output=True, text=True, env=env, cwd=env['HOME'], timeout=300)
    if done.returncode not in expect:
        raise SystemExit(f'smoke FAILED: saccade {" ".join(args)} -> {done.returncode}\n{done.stderr[-800:]}')
    return done


def smoke(name, binary, target, tag=None):
    """Run the bundle's smoke test against `binary`; returns the result record."""
    spec = bundle(name)
    work = pathlib.Path(tempfile.mkdtemp(prefix='saccade-smoke-'))
    env = {k: v for k, v in os.environ.items() if not k.startswith('SACCADE_')}
    env.update(HOME=str(work), USERPROFILE=str(work), XDG_CONFIG_HOME=str(work / 'config'),
               XDG_CACHE_HOME=str(work / 'cache'))
    checks = []

    def ok(label):
        checks.append(label)
        print(f'PASS {name}/{target}: {label}')

    doctor = json.loads(run(binary, ['doctor', '--json'], env).stdout)
    if tag:
        assert doctor['version'] == tag.removeprefix('v'), (doctor['version'], tag)
    missing = sorted(set(spec['expect_features']) - set(doctor['features']))
    if missing:
        raise SystemExit(f'smoke FAILED: bundle {name} lacks compiled features {missing}')
    ok('doctor reports the full feature inventory')

    for sub in ('base', 'cap'):
        (work / sub).mkdir()
        (work / sub / 'scene.png').write_bytes(png())
    run(binary, ['compare', str(work / 'base'), str(work / 'cap'), '--out', str(work / 'report'), '--json'], env)
    assert (work / 'report').is_dir()
    ok('compare identical captures exits 0 and writes a report')

    config = json.loads(run(binary, ['models', 'config', '--json'], env).stdout)
    assert config['schema'] == 'saccade-model-config.v1', config
    assert pathlib.Path(config['dir']['value']).is_absolute()
    ok('models config resolves the shared model directory')
    listing = json.loads(run(binary, ['models', 'list', '--json'], env).stdout)
    assert listing['schema'] in ('saccade-model-status.v1', 'saccade-model-status.v2'), listing
    ok('models list works offline')

    if name in ('media', 'full'):
        record = json.loads(run(binary, ['analyze-media', str(work / 'base' / 'scene.png')], env).stdout)
        assert record['schema'] in ('saccade-media-record.v1', 'saccade-media-record.v2'), record.get('schema')
        ok('analyze-media produces a record without downloading models')
        run(binary, ['inspect-image', str(work / 'base' / 'scene.png'), '--json'], env)
        ok('inspect-image runs')
    # No smoke step may have provisioned a model.
    cache = work / 'cache' / 'saccade' / 'models'
    assert not cache.exists() or not any(cache.iterdir()), 'smoke test downloaded a model'
    ok('no model was downloaded')
    return {'schema': 'saccade-bundle-smoke.v1', 'bundle': name, 'target': target, 'version': doctor['version'],
            'status': 'passed', 'checks': checks}


def inventory(name, binary, target, archive, doctor_json):
    spec = bundle(name)
    doctor = json.loads(pathlib.Path(doctor_json).read_text())
    return {
        'schema': 'saccade-bundle-inventory.v1',
        'bundle': name,
        'title': spec['title'],
        'summary': spec['summary'],
        'target': target,
        'version': doctor['version'],
        'git_commit': doctor['build'].get('git_commit'),
        'cargo_features_added_to_defaults': spec['features'],
        'compiled_features': doctor['features'],
        'excluded_everywhere': CONFIG['excluded_everywhere'],
        'archive': pathlib.Path(archive).name,
        'archive_sha256': hashlib.sha256(pathlib.Path(archive).read_bytes()).hexdigest(),
        'runtime_notes': 'ONNX Runtime and model files are never bundled; provision them with `saccade models pull`.',
    }


def main(argv):
    cmd = argv[1] if len(argv) > 1 else ''
    if cmd == 'features':      # features BUNDLE -> comma-separated cargo features
        print(','.join(bundle(argv[2])['features']))
    elif cmd == 'stem':        # stem BUNDLE TARGET -> asset name without extension
        print(asset_stem(argv[2], argv[3]))
    elif cmd == 'smoke':       # smoke BUNDLE BINARY TARGET OUT.json [TAG]
        record = smoke(argv[2], argv[3], argv[4], argv[6] if len(argv) > 6 else None)
        pathlib.Path(argv[5]).write_text(json.dumps(record, indent=2) + '\n')
    elif cmd == 'inventory':   # inventory BUNDLE BINARY TARGET ARCHIVE DOCTOR.json OUT.json
        record = inventory(argv[2], argv[3], argv[4], argv[5], argv[6])
        pathlib.Path(argv[7]).write_text(json.dumps(record, indent=2) + '\n')
    elif cmd == 'expected':    # expected -> every file name the assembled bundle set must contain
        for name, spec in CONFIG['bundles'].items():
            for target in spec['platforms']:
                ext = 'zip' if 'windows' in target else 'tar.gz'
                stem = asset_stem(name, target)
                print(f'{stem}.{ext}')
                print(f'{stem}.inventory.json')
                print(f'{stem}.smoke.json')
    else:
        raise SystemExit('usage: bundles.py features|stem|smoke|inventory|expected ...')


if __name__ == '__main__':
    main(sys.argv)
