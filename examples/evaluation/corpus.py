#!/usr/bin/env python3
"""Construct, freeze, execute and score the private R12 corpus. Public output is hash-only."""
import argparse
import collections
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

import pilot

HERE = Path(__file__).resolve().parent
MANIFEST = HERE / 'moss-pilot.toml'
SCENES = ['S4-large-terrain', 'S5-coastline-ocean', 'S6-postfx-bench']
FAMILIES = ['exact_optimization', 'repeat_noise', 'no_effect', 'local_regression',
            'intended_visual', 'metadata', 'incomparable_performance', 'hdr', 'temporal', 'semantic_hud']
VISUAL_QUESTIONS = ['triage.route.v1', 'vision.route.v1', 'intent.match.v1']
CAPS = {'jev': 400, 'gemini': 250}
SEED = 120032026
VERSION = 'constructed-truth-evaluation/1'


def load(path):
    return json.loads(Path(path).read_text())


def save(path, value):
    pilot.save(path, value)


def call(args, accepted=(0,)):
    return pilot.run(args, accepted)


def feature(value, source='measured'):
    return {'source': source, 'value': {'availability': 'available', 'value': value}}


def text(value):
    return {'type': 'text', 'value': value if isinstance(value, str) else json.dumps(value, sort_keys=True)}


def capture(config, scene, variant, role):
    """The wrapper owns every GPU operation and archive write, including its lease wait."""
    index = SCENES.index(scene)
    label = f'saccade-r12-S{index+4}-{role}-{variant}-20261003'
    folder = Path(config['capture_root']) / label
    receipt = Path(config['root']) / 'captures' / f'{label}.json'
    if receipt.exists():
        result = load(receipt)
        if any(pilot.file_hash(p) != h for p, h in result['files'].items()):
            raise ValueError('completed capture changed')
        return result
    args = [str(Path(config['moss'])/'scripts/moss-capture.sh'), '--headless',
            '--capture-png', str(folder), '--capture-json', str(folder/'capture.json'),
            '--capture-width', '640', '--capture-height', '360', '--capture-warmup-frames', '8',
            '--capture-sample-frames', '1']
    if scene != 'S6-postfx-bench':
        args += ['--capture-camera-fit', '--capture-camera-fit-scale', str(1.0 - 0.18*variant)]
    else:
        args += ['--camera-fov', str(60-8*variant)]
    if role == 'toggle':
        args += ['--debug-mode', 'default']
    args.append(scene)
    env = dict(os.environ, MOSS_CAPTURE_BIN=config['binary'], MOSS_CAPTURE_LABEL=label)
    if config.get('pinned_capture_binary'):
        env['MOSS_CAPTURE_ALLOW_STALE'] = '1'
    logfile = Path(config['root'])/'logs'/f'{label}.log'
    logfile.parent.mkdir(parents=True, exist_ok=True)
    print(f'capture {scene} {variant} {role}', flush=True)
    # A separately completed first wrapper invocation may be adopted only with
    # its successful post-flight health line, never on the presence of a PNG.
    original_log = Path(config['root'])/'logs'/'S4-base-0.log'
    adopted = (scene == SCENES[0] and variant == 0 and role == 'base'
               and original_log.exists() and 'released after' in original_log.read_text()
               and '(exit 0)' in original_log.read_text())
    if not adopted:
        with logfile.open('w') as output:
            status = subprocess.run(args, env=env, stdout=output, stderr=subprocess.STDOUT).returncode
        if status == 75:
            return capture(config, scene, variant, role)
        if status:
            raise ValueError(f'capture wrapper failed with exit {status}; private log retained')
    pngs = sorted(folder.rglob('*.png'))
    lit = next((p for p in pngs if p.stem == 'lit'), pngs[0] if pngs else None)
    if lit is None:
        raise ValueError('successful capture has no image')
    result = {'scene': scene, 'variant': variant, 'role': role, 'image': str(lit),
              'command': args, 'binary_sha256': pilot.file_hash(config['binary']),
              'files': {str(p): pilot.file_hash(p) for p in [lit, folder/'capture.json'] if p.is_file()}}
    save(receipt, result)
    return result


def intention(objective, changes):
    return {'availability': 'available', 'value': {
        'id': 'construction-intent', 'objective': objective, 'assurance': 'structured',
        'expected_changes': [{'key': 'construction.operation', 'reason': objective,
                              'expected_before': None, 'expected_after': objective}],
        'invariants': ['all pixels outside declared scope remain unchanged'],
        'criteria': [], 'source': None,
        'provenance': {'timestamp_unix_ms': None, 'paths': [], 'source_roots': [], 'source': 'predeclared-construction/1'},
    }}


def capture_native_intent(config):
    """Four camera/control variants share one renderer, wrapper and lease."""
    root=Path(config['root']); receipt=root/'captures/native-intent.json'
    if receipt.exists():
        result=load(receipt)
        if any(pilot.file_hash(p)!=h for p,h in result['files'].items()):
            raise ValueError('native capture changed')
        return result['pairs']
    variants=[]
    for v in range(2):
        for role in ['baseline','candidate']:
            overrides=['exposure=0','volumetric_fog=off']
            if role=='candidate':
                overrides=['exposure=0','volumetric_fog=on'] if v==0 else ['exposure=0.25','volumetric_fog=off']
            variants.append({'name':f'{v}-{role}','camera_fov':60-8*v,'override':overrides})
    plan=root/'native-intent-variants.json'; save(plan,variants)
    label='saccade-r12-S6-native-intent-20261004'
    folder=Path(config['capture_root'])/label
    args=[str(Path(config['moss'])/'scripts/moss-capture.sh'),'--headless',
          '--capture-png',str(folder),'--capture-width','640','--capture-height','360',
          '--capture-warmup-frames','8','--capture-sample-frames','1','--quality-profile','fidelity',
          '--enable-volumetrics','--capture-variants',str(plan),SCENES[2]]
    env=dict(os.environ,MOSS_CAPTURE_BIN=config['binary'],MOSS_CAPTURE_LABEL=label)
    if config.get('pinned_capture_binary'):env['MOSS_CAPTURE_ALLOW_STALE']='1'
    print('capture native intended exposure/fog variants',flush=True)
    with (root/'logs/native-intent.log').open('w') as output:
        status=subprocess.run(args,env=env,stdout=output,stderr=subprocess.STDOUT).returncode
    if status==75:return capture_native_intent(config)
    if status:raise ValueError(f'native wrapper failed with exit {status}; private log retained')
    pairs=[]; all_files={str(plan):pilot.file_hash(plan),str(folder/'variants.json'):pilot.file_hash(folder/'variants.json')}
    for v in range(2):
        pair=[]
        for role in ['baseline','candidate']:
            image=folder/f'{v}-{role}'/'lit.png'; metadata=image.parent/'capture.json'
            files={str(p):pilot.file_hash(p) for p in [image,metadata,plan,folder/'variants.json']}
            all_files.update(files)
            pair.append({'scene':SCENES[2],'variant':v,'role':role,'image':str(image),'command':args,
                         'binary_sha256':pilot.file_hash(config['binary']),'files':files,
                         'native_settings':variants[2*v+(role=='candidate')]['override']})
        pairs.append(pair)
    save(receipt,{'pairs':pairs,'files':all_files})
    return pairs


def construct(config, scene, variant, family, captures):
    """One independent intervention; frames/crops/orders never increase its count."""
    import numpy as np
    from PIL import Image, ImageDraw, ImageFont
    native_intent=family=='intended_visual' and scene==SCENES[2]
    if native_intent:
        native=capture_native_intent(config)[variant]
        captures=[native[0],native[1],native[0]]
    base, repeat, toggle = captures
    identifier = pilot.digest([VERSION, scene, variant, family, base['files'], repeat['files'], toggle['files']])
    folder = Path(config['root'])/'constructed'/identifier[7:]
    receipt = folder/'case-index.json'
    if receipt.exists():
        case = load(receipt)
        if 'generated_sha256' in case:
            return case
        # Upgrade preparation made before the freeze to the final native adapter.
        report = folder/'measurement/saccade-report.v1.json'
        packet = json.loads(call([config['helper'],'prepare',report,folder/'packet',folder/'context.json']))
        repaired = json.loads(call([config['helper'],'relocate',report,folder/'packet']))
        packet['requests'] = repaired['requests']; save(folder/'packet-index.json',packet)
        case['generated'] = {str(p):pilot.file_hash(p) for p in folder.rglob('*') if p.is_file() and p != receipt}
        case['generated_sha256'] = generated_hash(case)
        save(receipt,case); return case
    a = np.array(Image.open(base['image']).convert('RGB'))
    b = a.copy()
    h, w = a.shape[:2]
    x, y = (w//3 + variant*37), (h//3 + variant*23)
    rw, rh = 76+variant*9, 61+variant*7
    rect = [x, y, rw, rh]
    intent = None
    operation = ''
    truth = {'triage.route.v1': 'suspected_regression', 'vision.route.v1': 'inspect_regions',
             'capture.disposition.v1': 'continue_review', 'intent.match.v1': 'insufficient_intent'}
    extras = {}
    before, after = [a], [b]
    extension = '.png'
    if family == 'exact_optimization':
        operation = 'lossless PNG compression level changed; decoded samples preserved'
        intent = intention(operation, [])
        truth.update({'triage.route.v1': 'likely_intended', 'vision.route.v1': 'text_sufficient', 'intent.match.v1': 'consistent'})
    elif family == 'repeat_noise':
        operation = 'independent unchanged recapture; envelope measured against third unchanged control'
        b = np.array(Image.open(repeat['image']).convert('RGB'))
        control = np.array(Image.open(toggle['image']).convert('RGB'))
        envelope = int(max(np.abs(a.astype(int)-b.astype(int)).max(), np.abs(a.astype(int)-control.astype(int)).max()))
        extras['noise'] = feature(text({'kind': 'image', 'repeat_count': 3, 'native_max_channel_delta': envelope}))
        truth.update({'triage.route.v1': 'likely_noise', 'vision.route.v1': 'text_sufficient'})
        after = [b]
    elif family == 'no_effect':
        operation = 'documented renderer debug-mode toggle: implicit lit versus default alias'
        b = np.array(Image.open(toggle['image']).convert('RGB'))
        if not np.array_equal(a, b):
            raise ValueError('no-effect construction was not exact; do not invent truth')
        intent = intention(operation, [])
        truth.update({'triage.route.v1': 'likely_intended', 'vision.route.v1': 'text_sufficient', 'intent.match.v1': 'consistent'})
        after = [b]
    elif family == 'local_regression':
        if variant == 0:
            yy, xx = np.indices((rh, rw)); checker = ((xx//8+yy//8)%2)*255
            b[y:y+rh, x:x+rw] = np.stack([checker, np.zeros_like(checker), 255-checker], axis=-1)
            operation = 'image-space missing-texture checker patch'
        else:
            b[y:y+rh, x:x+rw] = np.clip(np.sqrt(b[y:y+rh, x:x+rw]/255)*255, 0, 255).astype('uint8')
            operation = 'image-space broken regional tonemap'
    elif family == 'intended_visual':
        subtype = (SCENES.index(scene)+variant) % 3
        if native_intent:
            b=np.array(Image.open(repeat['image']).convert('RGB'))
            operation=('native volumetric fog off to on; Fidelity and exposure=0 preserved' if variant==0 else
                       'native exposure EV 0 to 0.25; Fidelity and volumetric fog=off preserved')
            extras['capture_checks']=feature(text({'declared_settings_change':True,'before':base['native_settings'],'after':repeat['native_settings']}))
        elif subtype == 0:
            b = np.clip(a.astype(float)*1.18, 0, 255).astype('uint8'); operation = 'increase display exposure by factor 1.18'
        elif subtype == 1:
            b = np.clip(a.astype(float)*[1.10, 1.0, .90], 0, 255).astype('uint8'); operation = 'warm colour grade; red times 1.10 and blue times 0.90'
        else:
            b = (a.astype(float)*.78 + np.array([180, 190, 205])*.22).astype('uint8'); operation = 'uniform fog veil at opacity 0.22'
        intent = intention(operation, [])
        truth.update({'triage.route.v1': 'likely_intended', 'vision.route.v1': 'inspect_full_frame', 'intent.match.v1': 'consistent'})
        if native_intent and np.array_equal(a,b):truth['vision.route.v1']='text_sufficient'
        after = [b]
    elif family == 'metadata':
        factor = 1.12 + variant*.06
        b = np.clip(a.astype(float)*factor, 0, 255).astype('uint8')
        operation = 'image-space exposure setting changed without declaration'
        extras['capture_checks'] = feature(text({'matching_settings': False, 'undeclared_keys': ['exposure'], 'before': 1.0, 'after': factor}))
        truth.update({'triage.route.v1': 'needs_eyes', 'vision.route.v1': 'text_sufficient', 'capture.disposition.v1': 'inspect_configuration'})
    elif family == 'incomparable_performance':
        operation = 'constructed timing metadata: clock qualification absent or explicitly failed'
        after = [np.array(Image.open(repeat['image']).convert('RGB'))]
        truth = {'vision.route.v1': 'text_sufficient', 'perf.interpret.v1': 'collect_more_evidence',
                 'capture.disposition.v1': 'continue_review', 'intent.match.v1': 'insufficient_intent', 'triage.route.v1': 'needs_eyes'}
    elif family == 'hdr':
        linear = a.astype(np.float32)/255
        linear = np.where(linear <= .04045, linear/12.92, ((linear+.055)/1.055)**2.4)*8
        changed = linear.copy()
        if variant == 0:
            changed *= 1.25; operation = 'constructed float HDR: display-linear reconstruction times 8, declared exposure times 1.25'
            intent = intention('increase reconstructed linear exposure by factor 1.25', [])
            truth.update({'triage.route.v1': 'likely_intended', 'vision.route.v1': 'inspect_full_frame', 'intent.match.v1': 'consistent'})
        else:
            changed[y:y+rh, x:x+rw] = np.minimum(changed[y:y+rh, x:x+rw], .15)
            operation = 'constructed float HDR: local highlight clipping in display-linear reconstruction'
        before, after, extension = [linear], [changed], '.exr'
    elif family == 'temporal':
        before = [np.array(Image.open(c['image']).convert('RGB')) for c in captures]
        after = [frame.copy() for frame in before]
        if variant == 0:
            after[1][y:y+rh, x:x+rw] = [255, 0, 255]; operation = 'three-frame sequence with one-frame local flicker injection'
        else:
            after = [np.clip(frame.astype(float)*(1+.12*i), 0, 255).astype('uint8') for i, frame in enumerate(before)]
            operation = 'three-frame sequence with declared exposure ramp 1.0, 1.12, 1.24'
            intent = intention(operation, [])
            truth.update({'triage.route.v1': 'likely_intended', 'intent.match.v1': 'consistent'})
        truth['vision.route.v1'] = 'inspect_full_frame'
        extras['temporal'] = feature(text({'frame_count': 3, 'frame_order': [0, 1, 2], 'cadence': 'constructed sequence of independently rendered static captures', 'continuous_renderer_timeline': False}))
    elif family == 'semantic_hud':
        im = Image.fromarray(a); d = ImageDraw.Draw(im)
        font = ImageFont.load_default(size=20)
        d.rectangle((20, 22, 215, 55), fill=(15, 15, 15)); d.text((25, 25), 'HEALTH 100', font=font, fill='white')
        a = np.array(im); b = a.copy()
        if variant == 0:
            im = Image.fromarray(b); d = ImageDraw.Draw(im); d.rectangle((20, 22, 215, 55), fill=(15, 15, 15)); d.text((25,25), 'HEALTH 10', font=font, fill='white'); b = np.array(im)
            operation = 'constructed image-space HUD text changed from HEALTH 100 to HEALTH 10'
        else:
            a[75:89, 25:39] = [255, 255, 0]; b = a.copy(); b[75:89,25:39] = [15,15,15]; b[75:89,45:59] = [255,255,0]
            operation = 'constructed image-space HUD icon moved 20 pixels right'
        rect = [20, 20, 200, 80]; before, after = [a], [b]
        extras['semantic_uncertainty'] = feature(text('HUD meaning or position requires pixel inspection'))
    else:
        raise ValueError('unknown family')
    if family in ['local_regression', 'metadata']:
        after = [b]
    construction = {'schema': VERSION, 'scene': scene, 'camera_variant': variant, 'family': family,
                    'intervention': operation, 'image_space': family not in ['repeat_noise', 'no_effect'] and not native_intent,
                    'declared_intent': intent, 'captures': captures, 'region': rect,
                    'sequence_frames': len(before), 'truth': truth, 'truth_source': 'construction',
                    'hdr_scope': 'constructed float samples; no recovered scene radiance' if family == 'hdr' else None}
    folder.mkdir(parents=True, exist_ok=True)
    # The declaration and reference precede writing the intervention, not model inspection.
    save(folder/'construction.json', construction)
    for role, frames in [('baseline', before), ('capture', after)]:
        dest = folder/role; dest.mkdir(exist_ok=True)
        for i, frame in enumerate(frames):
            target = dest/f'frame-{i:02d}{extension}'
            if extension == '.exr':
                raw = dest/f'frame-{i:02d}.f32'
                frame.astype('<f4').tofile(raw)
                call([config['helper'],'exr',str(w),str(h),raw,target])
                raw.unlink()
            else:
                Image.fromarray(frame).save(target, compress_level=1 if role == 'baseline' else 9)
    if family in ['exact_optimization', 'no_effect'] and any(not np.array_equal(x, z) for x, z in zip(before, after)):
        raise ValueError('identity construction failed')
    if family == 'incomparable_performance':
        for role, source in [('baseline',base),('capture',repeat)]:
            timing_file = next(p for p in source['files'] if Path(p).name == 'capture.json')
            timing = load(timing_file)
            frame = timing['cost_card']['timings']['whole_frame_ms_p50']
            count = timing['benchmark']['whole_frame_available_sample_count']
            context = {'measurement': {'source':'real capture JSON','hash':pilot.file_hash(timing_file)},
                       'timer':timing['benchmark'].get('whole_frame_metric'), 'quantum_ms':None,
                       'sample_window':None, 'raw_samples':[], 'hardware':{}, 'aggregation':'independent_statistics',
                       'attribution_coverage':None, 'unresolved_remainder_ms':None,
                       'capture_hash':pilot.file_hash(folder/role/'frame-00.png'),
                       'configuration_hash':pilot.digest(timing['cost_card']['config']),
                       'producer':'constructed qualification over measured Moss timing','producer_version':'1',
                       'qualification':{'status':'unknown' if variant == 0 else 'rejected',
                                        'rule':'constructed-clock/1','version':'1',
                                        'checks':{'clock_qualified':None if variant == 0 else False},
                                        'reasons':['clock window absent' if variant == 0 else 'constructed clock qualification failed']},
                       'unavailable_terms':[]}
            save(folder/role/'saccade-perf.json', {'schema':'saccade-perf.v2','kind':'measurement','unit':'ms',
                 'frame':{'value':frame,'samples':count,'stat':'p50'},'terms':[],'counters':{},'context':context})
    features = dict(extras)
    features['expected_content'] = feature(text({'same_scene': True, 'same_camera': True, 'frame_count': len(before)}))
    if intent:
        features['declared_intervention'] = feature(text({'objective': intent['value']['objective']}), 'declared')
    context = {'validity': {'status': 'invalid' if family == 'metadata' else 'valid',
                            'reasons': ['undeclared exposure setting change'] if family == 'metadata' else []},
               'features': features}
    if intent:
        context['intent'] = intent
    save(folder/'context.json', context)
    report = folder/'measurement'
    call([config['saccade'], 'compare', folder/'baseline', folder/'capture', '--out', report,
          '--threshold', '1', '--metric', 'mean', '--json', '--entry', '*'+extension], (0, 1))
    packet = json.loads(call([config['helper'], 'prepare', report/'saccade-report.v1.json', folder/'packet', folder/'context.json']))
    repaired = json.loads(call([config['helper'], 'relocate', report/'saccade-report.v1.json', folder/'packet']))
    packet['requests'] = repaired['requests']; save(folder/'packet-index.json', packet)
    if family == 'incomparable_performance':
        request = load(folder/'packet/perf.interpret.v1.json')
        qualified = next(f for f in request['evidence']['facts'] if f['name']=='qualification')
        if qualified['value'] != {'availability':'available','value':{'type':'boolean','value':False}}:
            raise ValueError('production reader did not reject constructed clock qualification')
    case = {'case_id': identifier, 'family': family, 'scene_sha256': pilot.digest(scene),
            'group_sha256': pilot.digest([scene, family]), 'split_group_sha256': pilot.digest(scene),
            'construction_sha256': pilot.file_hash(folder/'construction.json'),
            'truth_sha256': pilot.digest(truth), 'truth': truth, 'truth_source': 'construction',
            'questions': [r['question'] for r in packet['requests']], 'egress_allowed': True,
            'folder': str(folder), 'origin': 'constructed', 'view_count': len(before),
            'input_sha256': [[pilot.file_hash(folder/'baseline'/f'frame-{i:02d}{extension}'), pilot.file_hash(folder/'capture'/f'frame-{i:02d}{extension}')] for i in range(len(before))],
            'source_sha256': [h for c in captures for h in c['files'].values()],
            'generated': {str(p): pilot.file_hash(p) for p in folder.rglob('*') if p.is_file()}}
    case['generated_sha256'] = generated_hash(case)
    save(receipt, case)
    return case


def public_case(case):
    return {k: case[k] for k in ['case_id', 'family', 'scene_sha256', 'group_sha256', 'split_group_sha256', 'split',
                                'construction_sha256', 'truth_sha256', 'truth_source', 'questions',
                                'egress_allowed', 'origin', 'view_count', 'input_sha256', 'source_sha256', 'generated_sha256']}


def generated_hash(case):
    return pilot.digest(sorted([str(Path(p).relative_to(Path(case['folder']))),h] for p,h in case['generated'].items()))


def recorded(config):
    original = load(HERE/'.local/mapping.json')
    cases = []
    for prior in original['cases']:
        case = copy.deepcopy(prior)
        case['truth'] = {}; citations = []
        if 'perf.interpret.v1' in case['questions']:
            request_file = Path(case['folder'])/'packet/perf.interpret.v1.json'
            request = load(request_file)
            qualification = next((f for f in request['evidence']['facts'] if f['name'] == 'qualification'), None)
            if qualification and qualification['value'] == {'availability':'available','value':{'type':'boolean','value':False}}:
                case['truth']['perf.interpret.v1'] = 'collect_more_evidence'
                citations.append(pilot.file_hash(request_file))
        for filename in case['private']['records']:
            file = Path(filename)
            if file.name == 'saccade-report.v1.json':
                report = load(file)
                entries = report.get('entries', [])
                if entries and all(e.get('bit_identical') is True and not e.get('error') for e in entries):
                    case['truth']['vision.route.v1'] = 'text_sufficient'; citations.append(pilot.file_hash(file))
            if file.name == 'ablate-summary.json':
                record = load(file)
                names = {'alpha-draw-interleaved': 'interleaved', 'alpha-draw-eq-off': 'eq-off', 'alpha-draw-gbuf': 'gbuf'}
                change = case['private']['change']; arm = names.get(change, change)
                row = next((r for r in record.get('runs', []) if r['name'] == arm), None)
                if row and row.get('effect') == 'NO-EFFECT':
                    case['truth']['vision.route.v1'] = 'text_sufficient'; citations.append(pilot.file_hash(file))
            if file.name == 'cost-card.json':
                card = load(file)
                if card.get('qualification.status') in ['unknown', 'unqualified'] or card.get('qualification.gpu_clock_qualified') is False:
                    if 'perf.interpret.v1' in case['questions']:
                        case['truth']['perf.interpret.v1'] = 'collect_more_evidence'; citations.append(pilot.file_hash(file))
        case['truth_source'] = 'recorded_outcome' if case['truth'] else 'unknown'
        case['truth_sha256'] = pilot.digest(case['truth'])
        case['construction_sha256'] = pilot.digest({'recorded_change': case['change_sha256'], 'verdict_citations': citations})
        case['generated_sha256'] = generated_hash(case)
        case['origin'] = 'recorded'; case['egress_allowed'] = True
        scene_group = case['private']['scene'].lower()
        if 'bistro' in scene_group:
            scene_group = 'bistro'
        case['split_group_sha256'] = pilot.digest(scene_group)
        # Broad imported-scene families stay indivisible even across older change aliases.
        case['group_sha256'] = pilot.digest([case['scene_sha256'], case['family']])
        cases.append(case)
    return cases


def split_cases(cases):
    groups = collections.defaultdict(list)
    for c in cases:
        groups[c.get('split_group_sha256', c['group_sha256'])].append(c)
    counts = dict.fromkeys(pilot.SPLITS, 0)
    for group in sorted(groups, key=lambda g: (-len(groups[g]), pilot.digest([SEED, g]))):
        split = min(counts, key=lambda s: (counts[s], pilot.SPLITS.index(s)))
        for c in groups[group]:
            c['split'] = split
        counts[split] += len(groups[group])
    return counts


def schedule(cases):
    selected = [c for c in cases if c['split'] != 'development']
    rows = []
    for c in sorted(selected, key=lambda c: pilot.digest([SEED, c['case_id']])):
        qs = c['questions']
        if not qs:
            continue
        rows.append({'case_id': c['case_id'], 'encoding': 'deterministic', 'provider': 'rules', 'order': 'structured', 'questions': qs, 'mode': 'pinned_provider'})
        rows.append({'case_id': c['case_id'], 'encoding': 'direct', 'provider': 'jev', 'order': 'structured', 'questions': qs, 'mode': 'pinned_provider'})
        for order in ['ab', 'ba']:
            rows.append({'case_id': c['case_id'], 'encoding': 'gemini_alone', 'provider': 'gemini', 'order': order,
                         'questions': [q for q in qs if q in VISUAL_QUESTIONS], 'mode': 'production_policy'})
            for encoding in ['enriched', 'enriched_without_priors']:
                rows.append({'case_id': c['case_id'], 'encoding': encoding, 'provider': 'jev', 'order': order, 'questions': qs, 'mode': 'production_policy'})
    # Balanced pinned availability probes on calibration cases are separate from policy scores.
    probes = [c for c in selected if c['split'] == 'calibration' and c['origin'] == 'constructed'][:2]
    for model in pilot.MODELS:
        for c in probes:
            for order in ['ab', 'ba']:
                rows.append({'case_id': c['case_id'], 'encoding': 'gemini_alone', 'provider': 'gemini', 'model': model,
                             'order': order, 'questions': [q for q in c['questions'] if q in VISUAL_QUESTIONS], 'mode': 'pinned_provider'})
    for i, row in enumerate(rows):
        row['job_id'] = pilot.digest([VERSION, i, row])
    initial = dict(collections.Counter(r['provider'] for r in rows if r['provider'] != 'rules'))
    if initial.get('jev', 0) > CAPS['jev'] or initial.get('gemini', 0) > CAPS['gemini']:
        raise ValueError('selected evaluation exceeds hard caps')
    return rows, initial


def freeze(config):
    if (HERE/'corpus-freeze.json').exists():
        raise ValueError('corpus already frozen; validate or resume it')
    ledger = Path(config['root'])/'ledger/evaluation.json'
    if ledger.exists() and load(ledger).get('attempts'):
        raise ValueError('provider attempts exist before this manifest freeze')
    cases = recorded(config)
    for scene in SCENES:
        for variant in range(2):
            captures = [capture(config, scene, variant, role) for role in ['base', 'repeat', 'toggle']]
            for family in FAMILIES:
                print(f'construct {scene} {variant} {family}', flush=True)
                cases.append(construct(config, scene, variant, family, captures))
    counts = split_cases(cases)
    rows, initial = schedule(cases)
    save(Path(config['root'])/'corpus-mapping.json', {'schema': VERSION, 'cases': cases, 'config': config})
    frozen = {'schema': VERSION, 'result_kind': 'constructed-truth evaluation', 'frozen_unix_ms': int(time.time()*1000),
              'cases': [public_case(c) for c in cases], 'dataset_sha256': pilot.digest([public_case(c) for c in cases]),
              'split_sha256': pilot.digest([[c['case_id'], c['split'], c['group_sha256']] for c in cases]),
              'labeler_count': 0, 'test_retest': 'not applicable under section 16', 'egress': 'all cases authorized under section 16',
              'exposure':{'builder_knows_construction_and_mapping':True,'human_labeling':False,
                          'provider_calls_before_freeze':0,'pinned_probes':'calibration cases only',
                          'held_out':'whole scenes isolated; never used to fit calibration'},
              'publication': 'aggregates and hashes only', 'rubric_sha256': pilot.file_hash(HERE/'rubrics.json'),
              'adapter_version': 'corpus-batched-closed/1', 'encoder_version': 'structured-evidence/1',
              'adapters': {str(p.relative_to(HERE.parents[1])): pilot.file_hash(p) for p in [HERE/'corpus.py', HERE/'corpus_live.py', HERE/'pilot.py',
                          *[HERE.parents[1]/f'crates/saccade-core/{f}' for f in ['examples/corpus_transport.rs','examples/pilot_offline.rs',
                          'src/questions.rs','src/budget_ledger.rs','src/judge_provider/transport.rs','src/judge_provider/observations.rs',
                          'src/judge_evidence.rs','src/judge_evidence/vision.rs','src/evidence/request.rs','src/judge_stats/evaluation.rs']]]},
              'executables': {name:pilot.file_hash(config[name]) for name in ['saccade','helper','transport','binary']},
              'requested_jev_model': 'jev-latest', 'gemini_chain': pilot.MODELS, 'fallback_policy': 'same actual model for both orders; mixed pairs unresolved',
              'batch_size': 'all applicable questions for one case per HTTP request', 'jobs': rows, 'initial_requests': initial,
              'caps': CAPS, 'retry_reserve': {p: CAPS[p]-initial.get(p,0) for p in CAPS}, 'retry_secs': [5,15,45,120,300],
              'elapsed_window_ms': 3600000, 'seed': SEED, 'counts': counts, 'cache_policy': 'manifest-and-payload hash; no truth in provider body',
              'price': 'unknown', 'gates': {'minimum_held_out': 50, 'attempted_completion': .95, 'deterministic_violations': 0,
                                         'important_miss_tolerance': 'undeclared; no routing away from review'}}
    MANIFEST.write_text('schema = "saccade-evaluation.v1"\nformat = "constructed-truth-evaluation/1"\nfrozen_json = \'\'\'\n'+pilot.encoded(frozen).decode()+'\n\'\'\'\n')
    save(HERE/'corpus-freeze.json', {'manifest_sha256': pilot.file_hash(MANIFEST), 'dataset_sha256': frozen['dataset_sha256'],
                                   'split_sha256': frozen['split_sha256'], 'frozen_before_provider_calls': True})
    validate(config)
    print(json.dumps({'cases': len(cases), 'splits': counts, 'requests': initial}, indent=2))


AMENDMENT = HERE / 'resume-amendment.json'
AMENDED_ADAPTERS = ['examples/evaluation/corpus.py', 'examples/evaluation/corpus_live.py',
                    'crates/saccade-core/examples/corpus_transport.rs', 'crates/saccade-core/src/budget_ledger.rs',
                    'crates/saccade-core/src/judge_provider/transport.rs']


def amendment():
    """Transport-fix resume record. It may replace adapter and transport identities and
    open a new elapsed window; the corpus, splits, rubrics, truth and job plan stay frozen."""
    if not AMENDMENT.exists():
        return None
    value = load(AMENDMENT)
    if value.get('manifest_sha256') != pilot.file_hash(MANIFEST):
        raise ValueError('resume amendment belongs to another frozen manifest')
    if set(value['adapters']) - set(AMENDED_ADAPTERS) or set(value['executables']) - {'transport'}:
        raise ValueError('resume amendment may only replace transport adapters')
    return value


def amend(config):
    """Record the post-fix transport identities. Never touches the frozen manifest."""
    manifest = json.loads(__import__('tomllib').loads(MANIFEST.read_text())['frozen_json'])
    root = HERE.parents[1]
    value = {'schema': 'saccade-evaluation-resume.v1', 'epoch': 'r12-transport-fix/1',
             'manifest_sha256': pilot.file_hash(MANIFEST),
             'reason': 'Jev max_tokens_exceeded batch split; ledger request rejection and pacing; jittered same-model backoff',
             'adapters': {f: pilot.file_hash(root/f) for f in AMENDED_ADAPTERS if pilot.file_hash(root/f) != manifest['adapters'][f]},
             'executables': {'transport': pilot.file_hash(config['transport'])},
             'resume_window_ms': 3*3600000, 'jev_split_bytes': 60000,
             # Measured from the partial run's ledger; the diagnosis probe counts inside these.
             'used_before_fix': {'jev': 112, 'gemini': 117},
             'remaining_caps': {'jev': CAPS['jev']-112, 'gemini': CAPS['gemini']-117}}
    save(AMENDMENT, value)
    print(json.dumps({k: value[k] for k in ['epoch', 'remaining_caps']}))


def validate(config):
    import tomllib
    manifest = json.loads(tomllib.loads(MANIFEST.read_text())['frozen_json'])
    seal = load(HERE/'corpus-freeze.json')
    if pilot.file_hash(MANIFEST) != seal['manifest_sha256']:
        raise ValueError('frozen manifest changed')
    if manifest['rubric_sha256'] != pilot.file_hash(HERE/'rubrics.json'):
        raise ValueError('frozen rubric changed')
    amended = amendment() or {'adapters': {}, 'executables': {}}
    for filename, expected in manifest['adapters'].items():
        if pilot.file_hash(HERE.parents[1]/filename) != amended['adapters'].get(filename, expected):
            raise ValueError('frozen adapter changed')
    for name,expected in manifest['executables'].items():
        if pilot.file_hash(config[name]) != amended['executables'].get(name, expected):
            raise ValueError('frozen executable changed')
    mapping = load(Path(config['root'])/'corpus-mapping.json')
    cases = mapping['cases']
    if [public_case(c) for c in cases] != manifest['cases'] or pilot.digest(manifest['cases']) != seal['dataset_sha256']:
        raise ValueError('corpus mapping changed')
    groups = {}; seen = set(); input_splits = {}
    for c in cases:
        if c['case_id'] in seen:
            raise ValueError('duplicate independent case')
        seen.add(c['case_id'])
        if groups.setdefault(c['split_group_sha256'], c['split']) != c['split']:
            raise ValueError('split leakage')
        for pair in c['input_sha256']:
            for content in pair:
                if input_splits.setdefault(content,c['split']) != c['split']:
                    raise ValueError('source image leakage across splits')
        if c['truth_sha256'] != pilot.digest(c['truth']):
            raise ValueError('frozen truth changed')
        if c['generated_sha256'] != generated_hash(c):
            raise ValueError('frozen packet inventory changed')
        for filename, expected in c['generated'].items():
            if pilot.file_hash(filename) != expected:
                raise ValueError('frozen input or packet changed')
        if c['origin'] == 'recorded':
            for source, expected in zip(c['sources'], c['source_sha256']):
                if not Path(source).exists() and config.get('allow_missing_recorded_sources'):
                    # The frozen generated packet is still checked above. A
                    # disappeared upstream archive cannot be rehashed here.
                    continue
                if pilot.file_hash(source) != expected:
                    raise ValueError('recorded source changed')
        if c['origin'] == 'constructed':
            for capture_record in load(Path(c['folder'])/'construction.json')['captures']:
                if any(pilot.file_hash(p) != h for p,h in capture_record['files'].items()):
                    raise ValueError('construction capture source changed')
    jobs, initial = schedule(cases)
    if jobs != manifest['jobs'] or initial != manifest['initial_requests']:
        raise ValueError('frozen plan changed')
    return manifest, mapping


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['freeze', 'validate', 'amend', 'run', 'score'])
    parser.add_argument('--config', required=True, help='private configuration JSON, outside public outputs')
    args = parser.parse_args(); config = load(args.config)
    if args.command == 'freeze':
        freeze(config)
    elif args.command == 'amend':
        amend(config)
    elif args.command == 'validate':
        m, _ = validate(config); print(json.dumps({'validated': True, 'cases': len(m['cases'])}))
    else:
        import corpus_live
        getattr(corpus_live, args.command)(config)


if __name__ == '__main__':
    main()
