#!/usr/bin/env python3
"""Generate bundled demo assets from repository-owned, licensed renderers."""
import hashlib
import importlib.util
import json
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'crates/saccade/assets/demo'


def module(name):
    source = ROOT / 'scripts' / (name + '.py')
    spec = importlib.util.spec_from_file_location(name, source)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def save(image, relative, **options):
    path = DEST / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, **options)


def main():
    renderer = module('gen-examples')
    drawn = module('gen-showcases')
    sphere = Image.fromarray(renderer.to_srgb8(renderer.render()))
    save(sphere, 'identity/baseline/sphere.png', compress_level=0)
    save(sphere, 'identity/capture/sphere.png', compress_level=9)
    save(drawn.dashboard(), 'baseline/ui_label.png')
    save(drawn.dashboard(label='Delete data'), 'capture/ui_label.png')
    save(sphere, 'review/baseline/sphere.png')
    save(Image.fromarray(renderer.to_srgb8(renderer.render(albedo=(0.82, 0.25, 0.20)))),
         'review/capture/sphere.png')
    intent = {
        'id': 'demo-warmer-material', 'objective': 'Increase sphere red albedo; preserve geometry and background.',
        'assurance': 'structured', 'expected_changes': [{'key': 'sphere.albedo.red',
            'reason': 'Declared material adjustment', 'expected_before': 0.80, 'expected_after': 0.82}],
        'invariants': ['Sphere geometry, light, camera, resolution and background remain fixed.'],
        'criteria': [{'id': 'bounded-mean-error', 'fact_id': 'demo-mean-flip',
            'comparator': 'less_or_equal', 'expected': {'type': 'number', 'value': 0.02}, 'units': 'FLIP'}],
        'source': None, 'provenance': {'timestamp_unix_ms': None, 'paths': [],
                                     'source_roots': [], 'source': 'repository-owned demo declaration'},
    }
    (DEST / 'review/intent.json').write_text(json.dumps(intent, indent=2) + '\n')
    sources = ['scripts/gen-examples.py', 'scripts/gen-showcases.py', 'scripts/gen-demo.py']
    record = {
        'schema': 'saccade-demo-provenance.v1',
        'license': 'MIT OR Apache-2.0',
        'publication_permission': 'Repository license grants redistribution of repository-owned code and generated assets.',
        'clearance_basis': 'Existing repository LICENSE-MIT and LICENSE-APACHE; no imported source imagery or private captures.',
        'rendering_example': 'Local CPU analytic sphere render: Lambert diffuse, Blinn-Phong highlight and analytic soft shadow.',
        'limits': ['Procedural scene; not a Moss GPU capture or performance benchmark.',
                   'Review provider responses and human resolution are illustrative offline fixtures, not live observations or attestation.'],
        'sources': {name: 'sha256:' + hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in sources},
        'assets': {p.relative_to(DEST).as_posix(): 'sha256:' + hashlib.sha256(p.read_bytes()).hexdigest()
                   for p in sorted(DEST.rglob('*')) if p.is_file() and p.name != 'provenance.json'},
    }
    (DEST / 'provenance.json').write_text(json.dumps(record, indent=2) + '\n')
    print('Bundled demo assets and provenance written.')


if __name__ == '__main__':
    main()
