#!/usr/bin/env python3
"""Offline R12 expansion. Private captures stay outside the repository."""
import argparse
import collections
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import time
import tomllib

import corpus
import pilot

HERE = Path(__file__).resolve().parent
BASE = HERE / 'moss-pilot.toml'
AMENDMENT = HERE / 'freeze-amendment.json'
EXPANDED = HERE / 'moss-pilot-expanded.toml'
ROOT = None
DISK = None
WRAPPER = None
BINARY = None
BINARY_HASH = None
EFFECTS = ('bloom=off', 'cascade_shadows=off', 'sky=off', 'exposure=0.25')
SCENES = (
    ('held_out', 'S4-large-terrain', None, 12),
    ('calibration', 'S5-coastline-ocean', None, 6),
    ('calibration', 'S6-postfx-bench', None, 6),
    ('development', 'sponza', 'development_project', 2),
)
BUILD_CAMERAS = {'S4-large-terrain': 12, 'S5-coastline-ocean': 3,
                 'S6-postfx-bench': 3, 'sponza': 2}


def floor():
    stat = os.statvfs(DISK)
    free = stat.f_bavail * stat.f_frsize
    if free < 25 * 1024**3:
        raise RuntimeError(f'disk floor reached: {free / 1024**3:.1f} GiB free')
    size = sum(p.stat().st_size for p in ROOT.rglob('*') if p.is_file()) if ROOT.exists() else 0
    if size >= 10 * 1024**3:
        raise RuntimeError(f'new derived data limit reached: {size / 1024**3:.1f} GiB')


def rows(scene, count):
    variants = []
    effect_indices = (0, 1, 3) if scene == 'sponza' else range(len(EFFECTS))
    for camera in range(count):
        view = {'camera_fov': round(43 + 2.7 * camera, 2),
                'capture_sample_frames': 8}
        variants.append(dict(view, name=f'c{camera:02d}-base', note='unchanged measured control'))
        variants.append(dict(view, name=f'c{camera:02d}-repeat', note='unchanged measured repeat'))
        variants.append(dict(view, name=f'c{camera:02d}-control', note='third unchanged no-effect arm'))
        for index in effect_indices:
            effect = EFFECTS[index]
            variants.append(dict(view, name=f'c{camera:02d}-e{index}',
                                 note=f'predeclared native intervention {effect}', override=[effect]))
    return variants


def plan():
    floor()
    for split, scene, project, count in SCENES:
        path = ROOT / f'{split}-{scene}-variants.json'
        pilot.save(path, rows(scene, count))
        print(json.dumps({'split': split, 'scene': scene, 'variants': len(rows(scene, count)),
                          'plan_sha256': pilot.file_hash(path)}))


def capture(scene_filter=None):
    for split, scene, project, count in SCENES:
        if scene_filter and scene != scene_filter:
            continue
        floor()
        plan_path = ROOT / f'{split}-{scene}-variants.json'
        if not plan_path.exists():
            raise RuntimeError('run plan first')
        target = ROOT / 'captures' / f'{split}-{scene}'
        if (target / 'variants.json').exists() and all(
            all((target / row['name'] / name).is_file()
                for name in ('lit.png', 'capture.json', 'saccade-perf.json'))
            for row in rows(scene, count)
        ):
            continue
        settings = CAPTURE_CONFIG.get('capture_settings', {}).get(scene, {})
        command = [str(WRAPPER), '--headless', '--capture-png', str(target),
                   '--capture-width', str(settings.get('width', 640)),
                   '--capture-height', str(settings.get('height', 360)),
                   '--capture-warmup-frames', str(settings.get('warmup', 16)),
                   '--capture-sample-frames', '8',
                   '--capture-variants', str(plan_path)]
        if project:
            command += ['--project', CAPTURE_CONFIG[project]]
            if scene == 'sponza':
                command += ['--capture-camera', 'PhysCamera001']
        command.append(scene)
        env = dict(os.environ, MOSS_CAPTURE_BIN=str(BINARY), MOSS_CAPTURE_ALLOW_STALE='1',
                   MOSS_CAPTURE_LABEL=f'saccade-r12x-{split}-{scene}', MOSS_LANE='saccade-r12x',
                   MOSS_CAPTURE_PURPOSE='offline constructed-truth evaluation')
        if settings.get('gpu_warmup'):
            env['MOSS_CAPTURE_GPU_WARMUP'] = '1'
        if settings.get('timeout_s'):
            env['MOSS_CAPTURE_TIMEOUT'] = str(settings['timeout_s'])
        if scene == 'sponza':
            env['MOSS_CAPTURE_LONG'] = 'benchmark'
            env['MOSS_CAPTURE_LONG_REASON'] = 'multi-camera constructed performance corpus needs one loaded scene and measured sample windows'
        logfile = ROOT / f'{split}-{scene}.log'
        with logfile.open('w') as out:
            result = subprocess.run(command, env=env, stdout=out, stderr=subprocess.STDOUT)
        if result.returncode:
            raise RuntimeError(f'capture failed ({result.returncode}); inspect private log {logfile}')
        for row in rows(scene, count):
            child = target / row['name']
            if not all((child / name).is_file() for name in ('lit.png', 'capture.json', 'saccade-perf.json')):
                raise RuntimeError(f'capture lacks required evidence: {child}')
            if pilot.read(child/'capture-meta.json').get('binary',{}).get('sha256') != BINARY_HASH[7:]:
                raise RuntimeError(f'capture renderer identity changed: {child}')
        print(json.dumps({'scene': scene, 'captured': len(rows(scene, count))}))


def single(config):
    """Capture independent Sponza sessions through the leased wrapper."""
    for name, effect in (('repeat', None), ('bloom-off', 'bloom=off'),
                         ('control-a', None), ('control-b', None)):
        floor()
        target = ROOT/'qualified'/name
        if all((target/item).is_file() for item in
               ('lit.png', 'capture.json', 'capture-meta.json', 'saccade-perf.json')):
            continue
        command = [str(WRAPPER), '--headless', '--capture-png', str(target),
                   '--capture-width', '1920', '--capture-height', '1080',
                   '--capture-warmup-frames', '48', '--capture-sample-frames', '8',
                   '--project', config['development_project'], '--capture-camera', 'PhysCamera001']
        if effect:
            command += ['--override', effect]
        command.append('sponza')
        env = dict(os.environ, MOSS_CAPTURE_BIN=str(BINARY), MOSS_CAPTURE_ALLOW_STALE='1',
                   MOSS_CAPTURE_LABEL=f'saccade-r12x-single-{name}', MOSS_LANE='saccade-r12x',
                   MOSS_CAPTURE_PURPOSE='offline independent performance qualification',
                   MOSS_CAPTURE_GPU_WARMUP='1', MOSS_CAPTURE_FORCE='1', MOSS_CAPTURE_TIMEOUT='3600',
                   MOSS_CAPTURE_LONG='benchmark',
                   MOSS_CAPTURE_LONG_REASON='independent measured Sponza repeat and intervention')
        logfile = ROOT/f'qualified-{name}.log'
        with logfile.open('w') as out:
            result = subprocess.run(command, env=env, stdout=out, stderr=subprocess.STDOUT)
        if result.returncode:
            raise RuntimeError(f'single capture failed ({result.returncode}); inspect {logfile}')
        if pilot.read(target/'capture-meta.json').get('binary',{}).get('sha256') != BINARY_HASH[7:]:
            raise RuntimeError(f'single capture renderer identity changed: {target}')
        status = pilot.read(target/'saccade-perf.json')['context']['qualification']['status']
        print(json.dumps({'single':name, 'qualification':status}),flush=True)


def make_case(config, split, scene, camera, effect_index):
    floor()
    target = ROOT / 'captures' / f'{split}-{scene}'
    base = target / f'c{camera:02d}-base'
    repeat = target / f'c{camera:02d}-repeat'
    candidate = target / (f'c{camera:02d}-control' if effect_index < 0 else f'c{camera:02d}-e{effect_index}')
    if scene == 'sponza' and camera == 0 and effect_index == -1 and config.get('qualified_triplet'):
        base, repeat, candidate = map(Path, config['qualified_triplet'])
    sources = [base, repeat, candidate]
    for source in sources:
        if not all((source / name).is_file() for name in ('lit.png', 'capture.json', 'saccade-perf.json')):
            raise RuntimeError(f'missing real capture evidence: {source}')
        meta = source/'capture-meta.json'
        if not meta.is_file() or pilot.read(meta).get('binary',{}).get('sha256') != BINARY_HASH[7:]:
            raise RuntimeError(f'capture used a different renderer binary: {source}')
    source_files = [source / name for source in sources for name in
                    ('lit.png', 'capture.json', 'saccade-perf.json', 'capture-meta.json')]
    source_files = [p for p in source_files if p.is_file()]
    source_hashes = [pilot.file_hash(p) for p in source_files]
    effect = 'unchanged third capture' if effect_index < 0 else EFFECTS[effect_index]
    family = 'measured_no_effect' if effect_index < 0 else 'native_performance'
    expected_performance = 'within_measured_noise' if effect_index in (-1, 3) else 'faster_or_within_measured_noise'
    identifier = pilot.digest(['r12x-perf/1', split, scene, camera, effect, source_hashes])
    folder = ROOT / 'cases' / identifier[7:]
    receipt = folder / 'case-index.json'
    if receipt.exists():
        return pilot.read(receipt)
    intent = corpus.intention(f'native renderer intervention {effect}; assess measured frame cost', [])
    independent = scene == 'sponza' and camera == 0 and effect_index == -1 and config.get('qualified_triplet')
    projection_hash = None
    if independent:
        original = [pilot.read(source/'saccade-perf.json') for source in sources]
        if len({p['context']['capture_hash'] for p in original}) != 3 or \
           any(p['context']['qualification']['status'] != 'qualified' for p in original):
            raise RuntimeError('independent triplet lacks three distinct qualified captures')
        stable = []
        for source in sources:
            manifest = pilot.read(source/'capture.json')
            inputs = {key:manifest[key] for key in
                      ('scene','camera','environment','render_feature_overrides','scene_overrides')
                      if key in manifest}
            stable.append(pilot.digest({'config':manifest['cost_card']['config'],'inputs':inputs}))
        if len(set(stable)) != 1:
            raise RuntimeError('independent triplet has different semantic configurations')
        projection_hash = stable[0]
    construction = {'schema': 'r12x-native-perf/1', 'scene': scene,
                    'camera_fov': 58.512 if independent else round(43 + 2.7 * camera, 2),
                    'family': family, 'intervention': effect,
                    'expected_performance': expected_performance,
                    'baseline': str(base), 'repeat': str(repeat), 'candidate': str(candidate),
                    'source_hashes': dict(zip(map(str, source_files), source_hashes)),
                    'truth_source': 'constructed intervention plus measured perf.v2',
                    'declared_intent': intent}
    if independent:
        construction['performance_scope'] = 'qualified whole-frame only; pass/gap attribution unavailable'
        construction['configuration_projection'] = {
            'rule':'r12x-stable-config/1', 'sha256':projection_hash,
            'included':'cost_card.config plus scene, camera, environment, render_feature_overrides, scene_overrides',
            'excluded':'dynamic render_passes; original producer sidecars retained and hashed'}
    folder.mkdir(parents=True)
    pilot.save(folder / 'construction.json', construction)
    for role, source in (('baseline', base), ('repeat', repeat), ('capture', candidate)):
        dest = folder / role
        dest.mkdir()
        shutil.copyfile(source / 'lit.png', dest / 'frame-00.png')
        if independent:
            evidence = pilot.read(source/'saccade-perf.json')
            evidence['terms'] = []
            evidence['context']['configuration_hash'] = projection_hash
            pilot.save(dest/'saccade-perf.json', evidence)
        else:
            shutil.copyfile(source / 'saccade-perf.json', dest / 'saccade-perf.json')
    context = {'validity': {'status': 'valid', 'reasons': []}, 'intent': intent,
               'features': {'declared_intervention': corpus.feature(corpus.text({'objective': effect}), 'declared'),
                            'expected_content': corpus.feature(corpus.text({'same_scene': True, 'same_camera': True}))}}
    pilot.save(folder / 'context.json', context)
    noise = folder / 'perf-noise.json'
    corpus.call([config['saccade'], 'noise', folder/'baseline', folder/'repeat',
                 '--kind', 'performance', '--out', noise, '--json'])
    report = folder / 'measurement'
    corpus.call([config['saccade'], 'compare', folder / 'baseline', folder / 'capture',
                 '--out', report, '--threshold', '1', '--metric', 'mean', '--json', '--entry', '*.png',
                 '--perf-noise', noise], (0, 1))
    measured = pilot.read(report / 'saccade-report.v1.json')
    perf = measured.get('perf_diff')
    if perf is None:
        raise RuntimeError('production comparator omitted real perf evidence')
    qualified = perf.get('comparability') == 'qualified'
    change = perf.get('frame_change')
    if not qualified or change == 'unknown' or \
       (effect_index < 0 and perf.get('materiality',{}).get('blocks_no_effect')):
        answer = 'collect_more_evidence'
    elif change == 'within_measured_noise' or (effect_index not in (-1, 3) and change == 'faster'):
        answer = 'consistent_with_intent'
    else:
        answer = 'unexplained_change'
    visual_same = measured['entries'][0].get('bit_identical') is True
    truth = {'triage.route.v1': 'likely_noise' if effect_index < 0 else 'likely_intended',
             'vision.route.v1': 'text_sufficient' if visual_same else 'inspect_full_frame',
             'perf.interpret.v1': answer, 'capture.disposition.v1': 'continue_review',
             'intent.match.v1': 'consistent'}
    pilot.save(folder / 'truth.json', {'truth': truth, 'measured_comparability': perf.get('comparability'),
                                      'measured_frame_change': change, 'noise_comparability': perf.get('noise_comparability')})
    packet = json.loads(corpus.call([config['helper'], 'prepare', report / 'saccade-report.v1.json',
                                     folder / 'packet', folder / 'context.json']))
    repaired = json.loads(corpus.call([config['helper'], 'relocate', report / 'saccade-report.v1.json', folder / 'packet']))
    packet['requests'] = repaired['requests']
    pilot.save(folder / 'packet-index.json', packet)
    questions = [r['question'] for r in packet['requests']]
    if not set(truth).issubset(questions):
        raise RuntimeError(f'production packet lacks required question(s): {set(truth)-set(questions)}')
    case = {'case_id': identifier, 'family': family, 'scene_sha256': pilot.digest(scene),
            'group_sha256': pilot.digest([scene, family]),
            'split_group_sha256': pilot.digest(scene), 'split': split,
            'construction_sha256': pilot.file_hash(folder / 'construction.json'),
            'truth_sha256': pilot.digest(truth), 'truth': truth, 'truth_source': 'construction',
            'questions': questions, 'egress_allowed': True, 'folder': str(folder),
            'origin': 'constructed', 'view_count': 1,
            'input_sha256': [[pilot.file_hash(folder/'baseline/frame-00.png'),
                              pilot.file_hash(folder/'capture/frame-00.png')]],
            'source_sha256': source_hashes}
    case['generated'] = {str(p): pilot.file_hash(p) for p in folder.rglob('*') if p.is_file()}
    case['generated_sha256'] = corpus.generated_hash(case)
    pilot.save(receipt, case)
    return case


def visual_source(scene, index):
    if scene == 'sponza':
        if CAPTURE_CONFIG.get('development_visual_source'):
            return Path(CAPTURE_CONFIG['development_visual_source'])
        path = ROOT/'captures/development-sponza'/f'c{index % 12:02d}-base'/'lit.png'
        return path
    old = pilot.read(ROOT.parent/'corpus-mapping.json')['cases']
    for case in old:
        if case['origin'] != 'constructed' or case['family'] != 'exact_optimization':
            continue
        construction = pilot.read(Path(case['folder'])/'construction.json')
        if construction['scene'] == scene and construction['camera_variant'] == index % 2:
            return Path(construction['captures'][0]['image'])
    raise RuntimeError(f'original source camera unavailable: {scene} {index % 2}')


def make_visual(config, split, scene, index):
    """A distinct deterministic intervention, declared before the image is written."""
    import numpy as np
    from PIL import Image, ImageDraw, ImageFont
    floor()
    source = visual_source(scene, index)
    if not source.is_file():
        raise RuntimeError(f'original Moss image unavailable: {source}')
    family = ('local_regression', 'intended_visual', 'metadata', 'semantic_hud')[index % 4]
    identifier = pilot.digest(['r12x-visual/1', split, scene, index, family, pilot.file_hash(source)])
    folder = ROOT/'cases'/identifier[7:]
    receipt = folder/'case-index.json'
    if receipt.exists():
        return pilot.read(receipt)
    a = np.array(Image.open(source).convert('RGB'))
    b = a.copy()
    h, w = a.shape[:2]
    x = int(w * (.15 + .05 * (index % 8)))
    y = int(h * (.15 + .05 * ((index // 2) % 8)))
    width = 45 + index % 17
    height = 32 + index % 13
    intent = None
    truth = {'triage.route.v1':'suspected_regression', 'vision.route.v1':'inspect_regions',
             'capture.disposition.v1':'continue_review', 'intent.match.v1':'insufficient_intent'}
    validity = 'valid'
    if family == 'local_regression':
        colour = [255, (37 * index) % 255, 255 - (19 * index) % 255]
        b[y:y+height, x:x+width] = colour
        operation = f'image-space local corruption at deterministic region {x},{y},{width},{height}'
    elif family == 'intended_visual':
        factor = round(1.03 + .014 * index, 4)
        b = np.clip(a.astype(float) * factor, 0, 255).astype('uint8')
        operation = f'image-space declared global display gain {factor}'
        intent = corpus.intention(operation, [])
        truth.update({'triage.route.v1':'likely_intended', 'vision.route.v1':'inspect_full_frame',
                      'intent.match.v1':'consistent'})
    elif family == 'metadata':
        factor = round(.82 - .004 * index, 4)
        b = np.clip(a.astype(float) * factor, 0, 255).astype('uint8')
        operation = f'image-space exposure factor {factor} omitted from change declaration'
        validity = 'invalid'
        truth.update({'triage.route.v1':'needs_eyes', 'vision.route.v1':'text_sufficient',
                      'capture.disposition.v1':'inspect_configuration'})
    else:
        old_text = f'HEALTH {100-index:02d}'
        new_text = f'HEALTH {50-index:02d}'
        font = ImageFont.load_default(size=20)
        for image, label in ((a, old_text), (b, new_text)):
            canvas = Image.fromarray(image)
            draw = ImageDraw.Draw(canvas)
            draw.rectangle((20, 22, 245, 58), fill=(15,15,15))
            draw.text((25,25), label, font=font, fill='white')
            image[:] = np.array(canvas)
        operation = f'image-space HUD text changed from {old_text} to {new_text}'
    construction = {'schema':'r12x-visual/1', 'scene':scene, 'family':family,
                    'source_capture':str(source), 'source_sha256':pilot.file_hash(source),
                    'intervention':operation, 'image_space':True,
                    'declared_intent':intent, 'truth_source':'construction', 'truth':truth,
                    'region':[x,y,width,height]}
    folder.mkdir(parents=True)
    pilot.save(folder/'construction.json', construction)
    for role, pixels in (('baseline',a),('capture',b)):
        dest = folder/role
        dest.mkdir()
        Image.fromarray(pixels).save(dest/'frame-00.png', compress_level=6)
    context = {'validity':{'status':validity,
                           'reasons':['undeclared exposure setting change'] if validity=='invalid' else []},
               'features':{'expected_content':corpus.feature(corpus.text({'same_scene':True,'same_camera':True}))}}
    if intent:
        context['intent'] = intent
        context['features']['declared_intervention'] = corpus.feature(corpus.text({'objective':operation}), 'declared')
    if family == 'metadata':
        context['features']['capture_checks'] = corpus.feature(corpus.text({'matching_settings':False,
                                                   'undeclared_keys':['exposure']}))
    if family == 'semantic_hud':
        context['features']['semantic_uncertainty'] = corpus.feature(corpus.text('HUD text meaning requires pixel inspection'))
    pilot.save(folder/'context.json', context)
    report = folder/'measurement'
    corpus.call([config['saccade'],'compare',folder/'baseline',folder/'capture',
                 '--out',report,'--threshold','1','--metric','mean','--json','--entry','*.png'],(0,1))
    packet = json.loads(corpus.call([config['helper'],'prepare',report/'saccade-report.v1.json',
                                     folder/'packet',folder/'context.json']))
    repaired = json.loads(corpus.call([config['helper'],'relocate',report/'saccade-report.v1.json',folder/'packet']))
    packet['requests'] = repaired['requests']
    pilot.save(folder/'packet-index.json',packet)
    questions = [r['question'] for r in packet['requests']]
    if not set(truth).issubset(questions):
        raise RuntimeError(f'visual packet lacks required question: {set(truth)-set(questions)}')
    case = {'case_id':identifier,'family':family,'scene_sha256':pilot.digest(scene),
            'group_sha256':pilot.digest([scene,family]),'split_group_sha256':pilot.digest(scene),
            'split':split,'construction_sha256':pilot.file_hash(folder/'construction.json'),
            'truth_sha256':pilot.digest(truth),'truth':truth,'truth_source':'construction',
            'questions':questions,'egress_allowed':True,'folder':str(folder),'origin':'constructed',
            'view_count':1,'input_sha256':[[pilot.file_hash(folder/'baseline/frame-00.png'),
                                          pilot.file_hash(folder/'capture/frame-00.png')]],
            'source_sha256':[pilot.file_hash(source)]}
    case['generated'] = {str(p):pilot.file_hash(p) for p in folder.rglob('*') if p.is_file()}
    case['generated_sha256'] = corpus.generated_hash(case)
    pilot.save(receipt,case)
    return case


def visual(config):
    cases = []
    for split, scene, start, count in (('held_out','S4-large-terrain',0,30),
                                       ('calibration','S5-coastline-ocean',0,3),
                                       ('calibration','S6-postfx-bench',15,3),
                                       ('development','sponza',0,46)):
        floor()
        if scene == 'sponza' and not visual_source(scene,0).exists():
            continue
        for index in range(start,start+count):
            cases.append(make_visual(config,split,scene,index))
        print(json.dumps({'scene':scene,'visual_cases':count}),flush=True)
    pilot.save(ROOT/'visual-mapping.json',{'schema':'r12x-visual/1','cases':cases})


def build(config, scene_filter=None):
    mapping = ROOT/'mapping.json'
    cases_by_id = {c['case_id']:c for c in pilot.read(ROOT/'visual-mapping.json')['cases']}
    if mapping.exists():
        for case in pilot.read(mapping)['cases']:
            construction = pilot.read(Path(case['folder'])/'construction.json')
            if construction['schema'] != 'r12x-native-perf/1':
                continue
            camera = round((construction['camera_fov'] - 43) / 2.7)
            if camera < BUILD_CAMERAS[construction['scene']]:
                cases_by_id[case['case_id']] = case
    for split, scene, _, count in SCENES:
        if scene_filter and scene != scene_filter:
            continue
        floor()
        for camera in range(BUILD_CAMERAS[scene]):
            effect_indices = (0, 1, 3) if scene == 'sponza' else range(len(EFFECTS))
            for effect_index in (-1, *effect_indices):
                if scene == 'sponza' and camera == 0 and effect_index == -1:
                    for identifier, previous in list(cases_by_id.items()):
                        prior = pilot.read(Path(previous['folder'])/'construction.json')
                        if prior.get('schema') == 'r12x-native-perf/1' and prior['scene'] == scene \
                           and prior['camera_fov'] == 43 and prior['intervention'] == 'unchanged third capture':
                            del cases_by_id[identifier]
                case = make_case(config, split, scene, camera, effect_index)
                cases_by_id[case['case_id']] = case
        print(json.dumps({'scene': scene, 'cases': BUILD_CAMERAS[scene] * (len(effect_indices)+1)}), flush=True)
        pilot.save(mapping, {'schema': 'r12x-expansion/1',
                             'cases': sorted(cases_by_id.values(), key=lambda c:c['case_id'])})


def expanded_jobs(cases):
    selected = [c for c in cases if c['split'] != 'development']
    jobs = []
    for c in sorted(selected, key=lambda c: pilot.digest([corpus.SEED, c['case_id']])):
        qs = c['questions']
        jobs += [dict(case_id=c['case_id'], encoding='deterministic', provider='rules', order='structured',
                      questions=qs, mode='pinned_provider'),
                 dict(case_id=c['case_id'], encoding='direct', provider='jev', order='structured',
                      questions=qs, mode='pinned_provider')]
        for order in ('ab', 'ba'):
            jobs.append(dict(case_id=c['case_id'], encoding='gemini_alone', provider='gemini', order=order,
                             questions=[q for q in qs if q in corpus.VISUAL_QUESTIONS], mode='production_policy'))
            for encoding in ('enriched', 'enriched_without_priors'):
                jobs.append(dict(case_id=c['case_id'], encoding=encoding, provider='jev', order=order,
                                 questions=qs, mode='production_policy'))
    probes = [c for c in sorted(selected, key=lambda c:c['case_id'])
              if c['split']=='calibration' and c['origin']=='constructed'][:2]
    for model in pilot.MODELS:
        for c in probes:
            for order in ('ab','ba'):
                jobs.append(dict(case_id=c['case_id'], encoding='gemini_alone', provider='gemini',
                                 model=model, order=order,
                                 questions=[q for q in c['questions'] if q in corpus.VISUAL_QUESTIONS],
                                 mode='pinned_provider'))
    for i, job in enumerate(jobs):
        job['job_id'] = pilot.digest(['r12x-expanded-plan/1', i, job])
    return jobs


def freeze(config):
    floor()
    if AMENDMENT.exists() or EXPANDED.exists():
        raise RuntimeError('expansion already frozen')
    old = json.loads(tomllib.loads(BASE.read_text())['frozen_json'])
    seal = pilot.read(HERE/'corpus-freeze.json')
    if pilot.file_hash(BASE) != seal['manifest_sha256']:
        raise RuntimeError('base freeze changed')
    snapshot = ROOT/'source'
    snapshot.mkdir(exist_ok=True)
    for name, source in (('moss-capture.sh', WRAPPER),
                         ('moss-saccade-perf.py', Path(config['capture_moss'])/'scripts/moss-saccade-perf.py')):
        target = snapshot/name
        if not target.exists():
            shutil.copyfile(source,target)
        if pilot.file_hash(target) != pilot.file_hash(source):
            raise RuntimeError(f'capture script identity changed before freeze: {name}')
    mapping = ROOT/'mapping.json'
    cases = pilot.read(mapping if mapping.exists() else ROOT/'visual-mapping.json')['cases']
    if len({c['case_id'] for c in cases}) != len(cases):
        raise RuntimeError('duplicate new case')
    expected_splits = {'held_out': 90, 'calibration': 36, 'development': 54}
    actual_splits = dict(collections.Counter(c['split'] for c in cases))
    if actual_splits != expected_splits:
        raise RuntimeError(f'incomplete expansion selection: {actual_splits}, expected {expected_splits}')
    all_cases = old['cases'] + [corpus.public_case(c) for c in cases]
    if len({c['case_id'] for c in all_cases}) != len(all_cases):
        raise RuntimeError('duplicate with base freeze')
    groups = {}
    for c in all_cases:
        group = c['split_group_sha256']
        if groups.setdefault(group, c['split']) != c['split']:
            raise RuntimeError('split leakage')
    known = collections.Counter(q for c in cases if c['split']=='held_out' for q in c['truth'])
    base_known = collections.Counter(q for c in pilot.read(Path(config['root'])/'corpus-mapping.json')['cases']
                                     if c['split']=='held_out' for q in c['truth'])
    shortfalls = {q:50-base_known[q]-known[q] for q in pilot.QUESTIONS if base_known[q]+known[q] < 50}
    shortfall_file = ROOT/'shortfall.json'
    if shortfalls and not shortfall_file.is_file():
        raise RuntimeError(f'held-out truth gate still short without recorded reason: {shortfalls}')
    shortfall_reason = pilot.read(shortfall_file) if shortfalls else None
    perf_cases = [c for c in cases if 'perf.interpret.v1' in c['truth']]
    measurement_status = collections.Counter()
    pair_comparability = collections.Counter()
    noise_comparability = collections.Counter()
    frame_changes = collections.Counter()
    qualification_reasons = collections.Counter()
    qualified_evidence_pairs = 0
    for c in perf_cases:
        folder = Path(c['folder'])
        status = pilot.read(folder/'baseline/saccade-perf.json')['context']['qualification']['status']
        finding = pilot.read(folder/'measurement/saccade-report.v1.json')['perf_diff']
        measurement_status[status] += 1
        pair_comparability[finding['comparability']] += 1
        noise_comparability[finding['noise_comparability']] += 1
        frame_changes[finding['frame_change']] += 1
        qualification_reasons.update(set(finding['qualification_reasons']))
        if finding['comparability']=='qualified' and finding['noise_comparability']=='qualified':
            qualified_evidence_pairs += 1
    qualification_shortfall = qualified_evidence_pairs == 0
    new_jobs = expanded_jobs(cases)
    new_requests = collections.Counter(j['provider'] for j in new_jobs if j['provider']!='rules')
    manifest = dict(old)
    manifest.update(cases=all_cases, dataset_sha256=pilot.digest(all_cases),
                    base_frozen_unix_ms=old['frozen_unix_ms'],
                    frozen_unix_ms=int(time.time()*1000),
                    split_sha256=pilot.digest([[c['case_id'],c['split'],c['group_sha256']] for c in all_cases]),
                    jobs=old['jobs']+new_jobs, counts=dict(collections.Counter(c['split'] for c in all_cases)),
                    initial_requests=dict(collections.Counter(j['provider'] for j in old['jobs']+new_jobs
                                                              if j['provider']!='rules')),
                    expansion_adapter_sha256=pilot.file_hash(__file__),
                    expansion_capture_wrapper_sha256=pilot.file_hash(snapshot/'moss-capture.sh'),
                    expansion_perf_producer_sha256=pilot.file_hash(snapshot/'moss-saccade-perf.py'),
                    expansion_capture_binary_sha256=pilot.file_hash(BINARY),
                    base_manifest_sha256=seal['manifest_sha256'],
                    base_results_sha256=pilot.file_hash(HERE/'RESULTS.json'),
                    expansion_jobs_sha256=pilot.digest(new_jobs),
                    evaluation_status='frozen with documented shortfall; new provider jobs unattempted'
                    if shortfalls or qualification_shortfall else 'frozen; new provider jobs unattempted')
    manifest['caps'] = {p:old['caps'][p]+math.ceil(1.25*new_requests[p]) for p in old['caps']}
    manifest['retry_reserve'] = {p:manifest['caps'][p]-manifest['initial_requests'][p]
                                 for p in old['caps']}
    EXPANDED.write_text('schema = "saccade-evaluation.v1"\nformat = "constructed-truth-evaluation/1"\nfrozen_json = \'\'\'\n'+pilot.encoded(manifest).decode()+'\n\'\'\'\n')
    amendment = {'schema':'saccade-evaluation-freeze-amendment.v1',
                 'frozen_unix_ms':manifest['frozen_unix_ms'],
                 'base_manifest_sha256':seal['manifest_sha256'], 'expanded_manifest_sha256':pilot.file_hash(EXPANDED),
                 'new_cases_sha256':pilot.digest([corpus.public_case(c) for c in cases]),
                 'new_cases':len(cases), 'split_counts':dict(collections.Counter(c['split'] for c in cases)),
                 'family_counts':dict(collections.Counter(c['family'] for c in cases)),
                 'family_by_split':{s:dict(collections.Counter(c['family'] for c in cases if c['split']==s))
                                    for s in pilot.SPLITS},
                 'question_by_split':{s:dict(collections.Counter(q for c in cases if c['split']==s
                                                                for q in c['truth'])) for s in pilot.SPLITS},
                 'held_out_known_truth':dict(base_known+known), 'rubric_sha256':old['rubric_sha256'],
                 'held_out_shortfall':shortfalls, 'shortfall_reason':shortfall_reason,
                 'performance_measurement_status':dict(measurement_status),
                 'performance_pair_comparability':dict(pair_comparability),
                 'performance_noise_comparability':dict(noise_comparability),
                 'performance_frame_change':dict(frame_changes),
                 'performance_qualification_reasons':dict(qualification_reasons),
                 'qualified_evidence_pairs':qualified_evidence_pairs,
                 'qualified_pair_shortfall':qualification_shortfall,
                 'old_split_is_subset':True, 'old_rubrics_unchanged':True,
                 'new_initial_requests':dict(new_requests),
                 'new_attempt_caps':{p:manifest['caps'][p]-old['caps'][p] for p in old['caps']},
                 'provider_calls_for_expansion':0,
                 'source_revalidation_limit':'older recorded private inputs absent; frozen public hashes retained'}
    pilot.save(AMENDMENT, amendment)
    validate(config)
    print(json.dumps(amendment, indent=2))


def validate(config):
    old = json.loads(tomllib.loads(BASE.read_text())['frozen_json'])
    expanded = json.loads(tomllib.loads(EXPANDED.read_text())['frozen_json'])
    amendment = pilot.read(AMENDMENT)
    seal = pilot.read(HERE/'corpus-freeze.json')
    if pilot.file_hash(BASE) != seal['manifest_sha256'] or pilot.digest(old['cases']) != seal['dataset_sha256']:
        raise RuntimeError('base freeze seal changed')
    old_private = pilot.read(Path(config['root'])/'corpus-mapping.json')['cases']
    if [corpus.public_case(c) for c in old_private] != old['cases']:
        raise RuntimeError('base public mapping changed')
    if expanded['cases'][:len(old['cases'])] != old['cases']:
        raise RuntimeError('old frozen cases are not an exact prefix')
    if old['rubric_sha256'] != expanded['rubric_sha256'] or old['rubric_sha256'] != pilot.file_hash(HERE/'rubrics.json'):
        raise RuntimeError('rubric changed')
    if pilot.file_hash(BASE) != amendment['base_manifest_sha256'] or pilot.file_hash(EXPANDED) != amendment['expanded_manifest_sha256']:
        raise RuntimeError('manifest hash mismatch')
    if expanded['expansion_adapter_sha256'] != pilot.file_hash(__file__) or \
       expanded['expansion_capture_wrapper_sha256'] != pilot.file_hash(ROOT/'source/moss-capture.sh') or \
       expanded['expansion_perf_producer_sha256'] != pilot.file_hash(ROOT/'source/moss-saccade-perf.py') or \
       expanded['expansion_capture_binary_sha256'] != pilot.file_hash(BINARY):
        raise RuntimeError('expansion adapter or capture producer identity changed')
    mapping = ROOT/'mapping.json'
    cases = pilot.read(mapping if mapping.exists() else ROOT/'visual-mapping.json')['cases']
    if [corpus.public_case(c) for c in cases] != expanded['cases'][len(old['cases']):]:
        raise RuntimeError('new case mapping changed')
    groups = {c['split_group_sha256']:c['split'] for c in old['cases']}
    inputs = {}
    for c in old['cases']:
        for pair in c['input_sha256']:
            for h in pair:
                inputs.setdefault(h,c['split'])
    for c in cases:
        if groups.setdefault(c['split_group_sha256'],c['split']) != c['split']:
            raise RuntimeError('scene split leakage')
        for pair in c['input_sha256']:
            for h in pair:
                if inputs.setdefault(h,c['split']) != c['split']:
                    raise RuntimeError('source image split leakage')
        if c['truth_sha256'] != pilot.digest(c['truth']) or c['generated_sha256'] != corpus.generated_hash(c):
            raise RuntimeError('case seal changed')
        for p, h in c['generated'].items():
            if pilot.file_hash(p) != h:
                raise RuntimeError(f'generated case file changed: {p}')
        construction = pilot.read(Path(c['folder'])/'construction.json')
        if 'source_hashes' in construction:
            for p,h in construction['source_hashes'].items():
                if pilot.file_hash(p) != h:
                    raise RuntimeError(f'native capture source changed: {p}')
        elif pilot.file_hash(construction['source_capture']) != construction['source_sha256']:
            raise RuntimeError('visual capture source changed')
    print(json.dumps({'validated':True, 'base_cases':len(old['cases']), 'new_cases':len(cases),
                      'expanded_cases':len(expanded['cases'])}))


def main():
    global ROOT, DISK, WRAPPER, BINARY, BINARY_HASH, CAPTURE_CONFIG
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['plan','capture','single','visual','build','freeze','validate'])
    parser.add_argument('--config', required=True, help='private configuration JSON')
    parser.add_argument('--scene', help='capture just one planned scene')
    args = parser.parse_args()
    config = pilot.read(args.config)
    ROOT = Path(config['root'])/'r12x'
    DISK = Path(config['root']).parents[1]
    WRAPPER = Path(config['capture_moss'])/'scripts/moss-capture.sh'
    BINARY = Path(config['capture_binary'])
    BINARY_HASH = pilot.file_hash(BINARY)
    CAPTURE_CONFIG = config
    if args.command == 'capture':
        capture(args.scene)
    elif args.command == 'single':
        single(config)
    elif args.command == 'build':
        build(config, args.scene)
    elif args.command == 'plan':
        plan()
    else:
        globals()[args.command](config)


if __name__ == '__main__':
    main()
