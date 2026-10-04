#!/usr/bin/env python3
"""Private Moss pilot workbench and offline replay. No provider dispatch exists here."""
import argparse
import collections
import copy
import functools
import fcntl
import hashlib
import http.server
import json
import math
import os
from pathlib import Path
import random
import secrets
import shutil
import subprocess
import time
import tomllib
from urllib.parse import urlsplit

HERE = Path(__file__).resolve().parent
LOCAL = HERE / '.local'
MANIFEST = HERE / 'preparation-pilot.toml'
GEMINI_ADAPTER = Path(__file__).resolve().parents[2] / 'crates/saccade-core/examples/pilot_gemini/mod.rs'
QUESTIONS = ['triage.route.v1', 'vision.route.v1', 'perf.interpret.v1',
             'capture.disposition.v1', 'intent.match.v1']
MODELS = ['gemini-3.8-flash', 'gemini-3.7-flash', 'gemini-3.6-flash', 'gemini-3.5-flash']
SEED = 120032026
SPLITS = ['development', 'calibration', 'held_out']
FAMILIES = ['exact_optimization', 'measured_image_noise', 'intended_visual',
            'local_regression', 'semantic_hud', 'metadata', 'ablation',
            'incomparable_performance', 'hdr_numerical_temporal', 'ambiguous_intent']


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def digest(value):
    return 'sha256:' + hashlib.sha256(encoded(value)).hexdigest()


def file_hash(file):
    h = hashlib.sha256()
    with Path(file).open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return 'sha256:' + h.hexdigest()


def save(file, value):
    file = Path(file)
    if file.is_symlink():
        raise ValueError('refusing local output symlink')
    file.parent.mkdir(parents=True, exist_ok=True)
    tmp = file.with_suffix('.tmp')
    if tmp.is_symlink():
        raise ValueError('refusing temporary symlink')
    with tmp.open('wb') as f:
        os.chmod(tmp, 0o600)
        f.write(encoded(value))
        f.flush()
        os.fsync(f.fileno())
    tmp.replace(file)


def read(file):
    return json.loads(Path(file).read_text())


def run(args, accepted=(0,)):
    p = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    if p.returncode not in accepted:
        raise ValueError(p.stderr[-2000:] or p.stdout[-2000:])
    return p.stdout


def public_case(c):
    return {k: c[k] for k in ('case_id', 'scene_sha256', 'group_sha256', 'change_sha256',
                             'family', 'split', 'intervention_sha256', 'source_sha256', 'input_sha256',
                             'questions', 'request_sha256', 'presentation_sha256',
                             'egress_allowed', 'origin', 'provenance_sha256')}


def plan(cases):
    """Counts the actual one-cell adapters; ROIs are attachments, never cases."""
    eligible = [c for c in cases if c['egress_allowed']]
    rows = []
    for q in QUESTIONS:
        subset = [c for c in eligible if q in c['questions']]
        n = len(subset)
        rows.append(dict(provider='jev', model='jev-latest', encoding='direct',
                         question=q, order='structured', batch_size=1, calls=n))
        for m in MODELS:
            for encoding in ['enriched', 'enriched_without_priors']:
                for order in ['ab', 'ba']:
                    rows.append(dict(provider='jev', model='jev-latest', vision_model=m,
                                     encoding=encoding, question=q, order=order,
                                     batch_size=1, calls=n))
            if q in ['triage.route.v1', 'vision.route.v1', 'intent.match.v1']:
                for order in ['ab', 'ba']:
                    rows.append(dict(provider='gemini', model=m, encoding='gemini_alone',
                                     question=q, order=order, batch_size=1,
                                     calls=n))
    for m in MODELS:
        for order in ['ab', 'ba']:
            rows.append(dict(provider='gemini', model=m, encoding='shared_observations',
                             question='visual-observation/1', order=order, batch_size=1,
                             calls=len(eligible)))
    totals = {p: sum(r['calls'] for r in rows if r['provider'] == p) for p in ['jev', 'gemini']}
    reserve = {p: math.ceil(n / 2) for p, n in totals.items()}
    maximum = {p: totals[p] + reserve[p] for p in totals}
    return dict(rows=rows, initial=totals, retry_reserve=reserve, maximum=maximum,
                global_initial=sum(totals.values()), global_maximum=sum(maximum.values()),
                authorized_calls=0, eligible_proposed=len(eligible))


def freeze(selection, saccade, helper):
    if MANIFEST.exists():
        raise ValueError('pilot is already frozen; use a new dataset directory for a new experiment')
    cases = []
    seen_changes = set()
    seen_pairs = set()
    for item in read(selection)['cases']:
        if item['family'] not in FAMILIES:
            raise ValueError('unknown case family')
        change = digest(item['change'])
        if change in seen_changes:
            raise ValueError('duplicate independent change')
        seen_changes.add(change)
        pairs = item['pairs']
        input_hashes = [[file_hash(p['baseline']), file_hash(p['capture'])] for p in pairs]
        interventions=[]
        for p in pairs:
            a=Path(p['baseline']).parent/'cost-card.json'
            b=Path(p['capture']).parent/'cost-card.json'
            if a.is_file() and b.is_file():
                before,after=read(a),read(b)
                interventions.append({k:[before.get(k),after.get(k)] for k in sorted(set(before)|set(after))
                                      if k.startswith(('env.','override.','cli.','binary.','build.','camera.','lod.'))
                                      and before.get(k)!=after.get(k)})
        intervention=digest(interventions)
        pair_key = digest([sorted(sorted(p) for p in input_hashes),intervention])
        if pair_key in seen_pairs:
            raise ValueError('same unordered images cannot pad independent support')
        seen_pairs.add(pair_key)
        sources = sorted(set(item['records'] + [p[r] for p in pairs for r in ['baseline', 'capture']]))
        source_hashes = [file_hash(p) for p in sources]
        identity = digest(dict(change=change, inputs=input_hashes, source=source_hashes))
        folder = LOCAL / 'cases' / identity[7:]
        for role in ['baseline', 'capture']:
            dest = folder / role
            dest.mkdir(parents=True, exist_ok=True)
            for i, p in enumerate(pairs):
                target = dest / f'view-{i}.png'
                shutil.copyfile(p[role], target)
            # Real source sidecars are retained for each view in the exact measured packet.
            for i, p in enumerate(pairs):
                for name in ['cost-card.json', 'capture-meta.json', 'saccade-perf.json']:
                    source = Path(p[role]).parent / name
                    if source.is_file():
                        if name == 'saccade-perf.json':
                            target = dest / name
                            if not target.exists():
                                shutil.copyfile(source, target)
                        else:
                            shutil.copyfile(source, dest / f'view-{i}.{name}')
                        if str(source) not in sources:
                            sources.append(str(source))
                            source_hashes.append(file_hash(source))
        out = folder / 'measurement'
        args = [saccade, 'compare', folder/'baseline', folder/'capture', '--out', out,
                '--threshold', '1', '--metric', 'mean', '--json',
                '--meta-name', 'cost-card.json', '--entry', '*.png']
        run(args, (0, 1))
        packet_dir = folder / 'packet'
        packet = json.loads(run([helper, 'prepare', out/'saccade-report.v1.json', packet_dir]))
        relocated=json.loads(run([helper,'relocate',out/'saccade-report.v1.json',packet_dir]))
        packet['requests']=relocated['requests']
        save(folder/'packet-index.json', packet)
        proposed = True  # The owner's binding section 16 authorizes every Moss case.
        case = dict(case_id=identity, scene_sha256=digest(item['scene']),
                    group_sha256=digest([item['scene'], item['change_family']]), change_sha256=change,
                    family=item['family'], intervention_sha256=intervention, source_sha256=source_hashes, input_sha256=input_hashes,
                    questions=[r['question'] for r in packet['requests']],
                    request_sha256=[r['request_sha256'] for r in packet['requests']],
                    presentation_sha256=[p['identity'] for p in packet['presentations']],
                    provenance_sha256=digest(item.get('license_evidence') or {'origin':item['origin'],'records':source_hashes}),
                    egress_allowed=proposed, origin=item['origin'], view_count=len(pairs),
                    sources=sources, folder=str(folder), private=item,
                    generated={str(p):file_hash(p) for p in folder.rglob('*') if p.is_file() and not p.is_symlink()})
        cases.append(case)
    if not cases or len(cases) > 60:
        raise ValueError('pilot needs 1 through 60 real cases')
    # One indivisible scene/change group, including every arm/view/repeat/order.
    groups = collections.defaultdict(list)
    for c in cases:
        groups[c['group_sha256']].append(c)
    totals = dict.fromkeys(SPLITS, 0)
    for group in sorted(groups, key=lambda g: (-len(groups[g]), digest([SEED, g]))):
        split = min(SPLITS, key=lambda s: (totals[s], SPLITS.index(s)))
        for c in groups[group]:
            c['split'] = split
        totals[split] += len(groups[group])
    cases.sort(key=lambda c: c['case_id'])
    catalog = packet['catalog']
    save(HERE/'rubrics.json', dict(schema='saccade-evaluation.v1', catalog=catalog,
                                 vision_rubric='vision-route-human/1', labeler_count=1,
                                 important_miss_tolerance='undeclared; no automatic routing'))
    mapping = dict(schema='saccade-evaluation.v1', cases=cases, provider_calls=0)
    save(LOCAL/'mapping.json', mapping)
    b = plan(cases)
    dataset = digest([public_case(c) for c in cases])
    split_hash = digest([[c['case_id'], c['split'], c['group_sha256']] for c in cases])
    frozen = dict(schema='saccade-evaluation.v1', job='score', modes=['pinned_provider','production_policy'],
                  dataset_sha256=dataset, split_sha256=split_hash, rubric_sha256=file_hash(HERE/'rubrics.json'),
                  encoder_version=packet['encoder_version'], feature_projection='pilot-feature-projection/1', adapter_version='r11-single-cell/1',
                  replay_adapter='pilot-recorded-closed-answer/2',
                  gemini_question_adapter='pilot-gemini-question/1', gemini_question_adapter_sha256=file_hash(GEMINI_ADAPTER),
                  vision_rubric='visual-observation/1', vision_transform='r10-display/1', provider_visual_scope='primary-lit-view-plus-recorded-crops/1', secondary_views='local-human-context; zero extra provider requests',
                  fallback_identity=digest(MODELS), fallback_in_pinned=False,
                  production_fallback=MODELS, requested_jev_model='jev-latest', requested_gemini_models=MODELS, resolved_model_revisions='pending; no provider calls', presentation_seed=SEED, retest_seed=SEED+7,
                  batch_size=1, cache_policy='resume_by_frozen_manifest_and_payload',
                  retry_secs=[5,15,45,120,300], elapsed_ms=3600000,
                  evaluation_window_days=14, attempts_caps=b['maximum'], budget_calls=b['global_maximum'],
                  authorized_calls=0, provider_dispatch_enabled=False,
                  price='unknown; repository model names are not a verified live catalog',
                  project_priors_sha256=digest({'policy':'empty-pilot-priors/1'}),
                  labeler_count=1, model_exposure='human_declares_per_case_before_label',
                  implementation_knowledge='human_declares_per_case_before_label',
                  mapping_knowledge='human_declares_per_case_before_label',
                  test_retest=dict(status='pending', minimum_delay_days=7,
                                   case_ids=[c['case_id'] for c in cases if c['split']=='held_out'][:20],
                                   actual_delay_days='pending', agreement='pending'),
                  gates=dict(attempted_completion=0.95, minimum_held_out_per_question=50,
                             deterministic_violations=0, miss_tolerance='undeclared',qualified=False),
                  budget=b, counts=totals, cases=[public_case(c) for c in cases])
    # TOML wraps an immutable JSON projection: no optional TOML-null ambiguities.
    MANIFEST.write_text('schema = "saccade-evaluation.v1"\nformat = "moss-pilot-preparation/1"\n'
                        '# Hash-only public projection; private mapping is .local/mapping.json.\n'
                        "frozen_json = '''\n"+encoded(frozen).decode()+"\n'''\n")
    egress = ['# Egress authorization', '',
              'The binding design section 16 authorizes every Moss case for private Jev and Gemini evaluation, without licence-based exclusions.',
              'Publication remains limited to aggregates and hashes. Images, crops and private paths remain private.']
    (HERE/'egress-review.md').write_text('\n'.join(egress)+'\n')
    validate()
    print(json.dumps(dict(cases=len(cases), splits=totals, budget={k:v for k,v in b.items() if k!='rows'}), indent=2))


def manifest():
    m = tomllib.loads(MANIFEST.read_text())
    return json.loads(m['frozen_json'])


def validate():
    m = manifest()
    local = read(LOCAL/'mapping.json')
    cases = local['cases']
    if m['rubric_sha256'] != file_hash(HERE/'rubrics.json'):
        raise ValueError('frozen rubric changed')
    if m['gemini_question_adapter'] != 'pilot-gemini-question/1' or m['gemini_question_adapter_sha256'] != file_hash(GEMINI_ADAPTER):
        raise ValueError('frozen Gemini question adapter changed')
    if [public_case(c) for c in cases] != m['cases'] or digest(m['cases']) != m['dataset_sha256']:
        raise ValueError('dataset or mapping changed')
    if digest([[c['case_id'], c['split'], c['group_sha256']] for c in cases]) != m['split_sha256']:
        raise ValueError('split identity changed')
    groups, changes, pairs = {}, set(), set()
    for c in cases:
        g = c['group_sha256']
        if g in groups and groups[g] != c['split']:
            raise ValueError('split leakage')
        groups[g] = c['split']
        pair = digest([sorted(sorted(x) for x in c['input_sha256']),c['intervention_sha256']])
        if c['change_sha256'] in changes or pair in pairs:
            raise ValueError('duplicate change or views padded as support')
        changes.add(c['change_sha256'])
        pairs.add(pair)
        if [file_hash(p) for p in c['sources']] != c['source_sha256']:
            raise ValueError('source content changed')
        for p, h in c['generated'].items():
            if file_hash(p) != h:
                raise ValueError('packet or display content changed')
    if plan(cases) != m['budget']:
        raise ValueError('budget plan changed')
    if m['budget_calls'] != m['budget']['global_maximum'] or m['attempts_caps'] != m['budget']['maximum']:
        raise ValueError('global or provider cap differs')
    return m, local


def label_file(phase):
    return LOCAL/f'{phase}-labels.json'


def labels(phase):
    file = label_file(phase)
    return read(file) if file.exists() else []


def retest_ids(m, initial, now):
    ids = m['test_retest']['case_ids']
    if not ids or any(not any(r['case_id']==c and r['stage']=='final' for r in initial) for c in ids):
        raise ValueError('test-retest pending: initial held-out labels are incomplete')
    latest = max(r['recorded_unix_ms'] for r in initial if r['case_id'] in ids and r['stage']=='final')
    if now - latest < 7*86400000:
        raise ValueError('test-retest pending: seven days have not elapsed since the last initial label')
    return ids


def validate_record(c, record, phase, prior):
    if set(record) != {'case_id','stage','answers','vision','exposure'} or record['case_id'] != c['case_id']:
        raise ValueError('label content does not match this case')
    if record['stage'] not in ['structured','final']:
        raise ValueError('unknown labeling stage')
    if any(record['exposure'].get(k) not in ['yes','no','unknown'] for k in ['model','mapping','implementation']):
        raise ValueError('record all exposure conditions')
    if record['exposure']['model'] != 'no':
        raise ValueError('blind pilot requires no previous model proposals')
    v = record['vision']
    if record['stage']=='structured':
        if record['answers'] or set(v) != {'supports_disposition','unresolved_visual_fact','provisional_route','uncertainty'}:
            raise ValueError('structured assessment cannot contain final labels')
        if v['supports_disposition'] not in ['yes','no','undetermined']:
            raise ValueError('invalid structured assessment')
        if v['provisional_route'] not in read(HERE/'rubrics.json')['catalog'][1]['answers']:
            raise ValueError('invalid provisional route')
        return
    if not any(r['case_id']==c['case_id'] and r['stage']=='structured' for r in prior):
        raise ValueError('record structured-first assessment before pixels')
    if set(record['answers']) != set(c['questions']):
        raise ValueError('answer every applicable question')
    for q, answer in record['answers'].items():
        request = read(Path(c['folder'])/'packet'/f'{q}.json')
        if answer not in request['question']['answers']:
            raise ValueError('unknown closed answer')
    if set(v) != {'requires_pixels','minimum_scope','requires_human','necessary_visual_fact','route'}:
        raise ValueError('record the complete vision rubric')
    if v['requires_pixels'] not in ['yes','no','undetermined'] or v['minimum_scope'] not in ['none','regions','full_frame'] or not isinstance(v['requires_human'],bool):
        raise ValueError('invalid vision rubric')
    if v['requires_pixels']=='yes' and (not v['necessary_visual_fact'].strip() or v['minimum_scope']=='none'):
        raise ValueError('pixel need requires a necessary visual fact and useful scope')
    if v['requires_pixels']=='no' and v['minimum_scope']!='none':
        raise ValueError('no pixel need means no minimum scope')
    if v['route'] != record['answers']['vision.route.v1']:
        raise ValueError('final route must match the vision answer')
    if v['route']=='human_directly' and not v['requires_human']:
        raise ValueError('human_directly requires human judgment')


def locked_labels(function):
    @functools.wraps(function)
    def locked(*args, **kwargs):
        lock=LOCAL/'labels.lock'
        if lock.is_symlink():
            raise ValueError('refusing label lock symlink')
        with lock.open('a') as handle:
            fcntl.flock(handle,fcntl.LOCK_EX)
            return function(*args,**kwargs)
    return locked


@locked_labels
def record_label(c, record, phase, session, region_refs=None, full_frame_seen=False):
    m, _ = validate()
    prior = labels(phase)
    if phase=='retest':
        retest_ids(m, labels('initial'), int(time.time()*1000))
    validate_record(c, record, phase, prior)
    if any(r['case_id']==c['case_id'] and r['stage']==record['stage'] for r in prior):
        raise ValueError('keep initial labels; adjudication requires a separate record')
    stamped = dict(record, recorded_unix_ms=int(time.time()*1000), phase=phase,
                   manifest_sha256=file_hash(MANIFEST), session_sha256=digest(session),
                   packet_sha256=c['request_sha256'], presentation_sha256=c['presentation_sha256'])
    stamped['label_id'] = digest(stamped)
    if record['stage']=='final':
        initial=next(r for r in prior if r['case_id']==c['case_id'] and r['stage']=='structured')
        retest_of={}
        old=None
        if phase=='retest':
            old=read(LOCAL/'canonical-labels'/'initial'/f"{c['case_id'][7:]}.json")
            retest_of={r['label']['question_id']:r['label_id'] for r in old['records']['items']}
        audit=LOCAL/'label-inputs'/phase/f"{c['case_id'][7:]}.json"
        save(audit,dict(initial=initial,final=stamped,region_refs=region_refs or [],
                        full_frame_seen=full_frame_seen,test_retest_of=retest_of,
                        prior_records=old['records']['items'] if old else [],prior_human=old['human'] if old else None))
        result=json.loads(run([LOCAL/'bin'/'pilot_offline','label',Path(c['folder'])/'packet',audit]))
        save(LOCAL/'canonical-labels'/phase/f"{c['case_id'][7:]}.json",result)
    save(label_file(phase), prior+[stamped])


def retest_metrics(initial, second):
    first = {r['case_id']:r for r in initial if r['stage']=='final'}
    later = {r['case_id']:r for r in second if r['stage']=='final'}
    changes = collections.defaultdict(collections.Counter)
    n = same = 0
    delays = []
    for cid, b in later.items():
        a = first.get(cid)
        if not a:
            continue
        delays.append((b['recorded_unix_ms']-a['recorded_unix_ms'])/86400000)
        for q, truth in a['answers'].items():
            n += 1
            same += b['answers'][q]==truth
            changes[q][truth+' -> '+b['answers'][q]] += 1
    return dict(status='pending' if not later else 'partial_or_complete', cases=len(delays),
                minimum_actual_delay_days=min(delays) if delays else None,
                agreement=same/n if n else None, per_class_changes={q:dict(v) for q,v in changes.items()})


def cells(cases):
    for c in cases:
        for q in c['questions']:
            yield c,q,'direct','jev-latest',None,'structured'
            yield c,q,'deterministic','rules/1',None,'structured'
            for m in MODELS:
                for order in ['ab','ba']:
                    for e in ['enriched','enriched_without_priors']:
                        yield c,q,e,'jev-latest',m,order
                    if q in ['triage.route.v1','vision.route.v1','intent.match.v1']:
                        # All views remain in the same independent-case group.
                        for v in range(1):
                            yield c,q,'gemini_alone',m,None,f'{order}/view-{v}'


def check_attempt_caps(records, m):
    for mode in m['modes']:
        attempts=collections.Counter()
        for r in records:
            if r.get('mode','pinned_provider')!=mode or r['encoding']=='deterministic':
                continue
            count=r.get('attempts',0)
            if not isinstance(count,int) or count<0:
                raise ValueError('invalid recorded attempt count')
            provider='gemini' if r['encoding'] in ['gemini_alone','shared_observations'] else 'jev'
            attempts[provider]+=count
        if sum(attempts.values())>m['budget_calls'] or any(n>m['attempts_caps'][p] for p,n in attempts.items()):
            raise ValueError('recorded attempts exceed the R11 evaluation caps')


def replay_extractions(cases, answers, m, scheduled):
    """Shared extraction work has operational counts and no catalog-quality score."""
    groups=collections.defaultdict(list)
    for c in cases:
        presentations=read(Path(c['folder'])/'packet-index.json')['presentations']
        for model in MODELS:
            for order in ['ab','ba']:
                for mode in m['modes']:
                    k=(c['case_id'],'visual-observation/1','shared_observations',model,None,order,mode)
                    scheduled.add(k)
                    r=answers.get(k)
                    outcome='deferred' if c['egress_allowed'] else 'denied'
                    if r:
                        if r.get('answer') is not None or r.get('probability') is not None:
                            raise ValueError('shared extraction is not a closed question answer')
                        if not c['egress_allowed']:
                            if r['outcome']!='denied' or r.get('attempts',0)!=0:
                                raise ValueError('recorded provider answer for denied content')
                        else:
                            p=next(p for p in presentations if p['view']==0 and p['order']==order)
                            if r['manifest_sha256']!=file_hash(MANIFEST) or r['presentation_identity']!=p['identity']:
                                raise ValueError('shared extraction does not bind the frozen presentation')
                            if mode=='pinned_provider' and r.get('actual_model',model)!=model:
                                raise ValueError('pinned replay forbids fallback substitution')
                            outcome=r['outcome']
                    actual=r.get('actual_model',model) if r else model
                    groups[(mode,actual)].append(dict(outcome=outcome,attempts=r.get('attempts',0) if r else 0,
                                                      latency_ms=r.get('latency_ms') if r else None,
                                                      usage=r.get('usage') if r else None,cost=r.get('cost') if r else None))
    output=[]
    for (mode,model),rows in sorted(groups.items()):
        counts=collections.Counter(r['outcome'] for r in rows)
        eligible=len(rows)-counts['denied']
        valid=counts['answered']+counts['abstained']
        latencies=sorted(r['latency_ms'] for r in rows if r['latency_ms'] is not None)
        output.append(dict(mode=mode,model=model,scheduled=len(rows),eligible=eligible,
                           attempted=sum(r['attempts']>0 for r in rows),attempts=sum(r['attempts'] for r in rows),
                           **{s:counts[s] for s in ['answered','abstained','invalid','unavailable','deferred','denied','budget_blocked']},
                           availability=valid/eligible if eligible else None,
                           latency_p50_ms=latencies[round((len(latencies)-1)*.5)] if latencies else None,
                           latency_p95_ms=latencies[round((len(latencies)-1)*.95)] if latencies else None,
                           usage=[r['usage'] for r in rows],cost=None if any(r['cost'] is None for r in rows) else sum(r['cost'] for r in rows),
                           conditional_accuracy=None,quality_support=0))
    return output


def replay(answer_file, helper):
    m, local = validate()
    records = read(answer_file) if answer_file else []
    key = lambda r: (r['case_id'],r['question'],r['encoding'],r['model'],r.get('vision_model'),r['order'],r.get('mode','pinned_provider'))
    answers = {}
    for r in records:
        k = key(r)
        if k in answers:
            raise ValueError('duplicate recorded cell')
        if r.get('mode','pinned_provider') not in m['modes']:
            raise ValueError('invalid replay mode')
        permitted_outcomes={'answered','abstained','invalid','unavailable','deferred','denied','budget_blocked'}
        if r['outcome'] not in permitted_outcomes:
            raise ValueError('unknown resolution')
        usage=r.get('usage')
        if usage is not None:
            if not isinstance(usage,dict) or any(k not in {'input_tokens','output_tokens','total_tokens','cached_tokens'} or not isinstance(v,int) or v<0 for k,v in usage.items()):
                raise ValueError('usage export accepts numeric token counters only')
        for field in ['latency_ms','cost','probability']:
            value=r.get(field)
            if value is not None and (not isinstance(value,(int,float)) or not math.isfinite(value) or value<0):
                raise ValueError('operational metrics must be nonnegative finite numbers')
        if r.get('probability') is not None and r['probability']>1:
            raise ValueError('probability exceeds one')
        actual_models=MODELS if r['encoding'] in ['gemini_alone','shared_observations'] else ['jev-latest','rules/1']
        if r.get('actual_model',r['model']) not in actual_models:
            raise ValueError('unplanned actual model')
        answers[k] = r
    check_attempt_caps(answers.values(),m)
    pairs=collections.defaultdict(list)
    for r in answers.values():
        if r['order'].split('/')[0] in ['ab','ba']:
            pairs[(r['case_id'],r['question'],r['encoding'],r['model'],r.get('vision_model'),r['order'].partition('/')[2],r.get('mode','pinned_provider'))].append(r)
    for group in pairs.values():
        if len(group)==2 and len({r.get('actual_model',r['model']) for r in group})>1:
            for r in group:
                r.update(outcome='deferred',answer=None,probability=None)
    truth = {r['case_id']:r for r in labels('initial') if r['stage']=='final'}
    eligibility={}
    for cid in truth:
        canonical=read(LOCAL/'canonical-labels'/'initial'/f'{cid[7:]}.json')
        eligibility[cid]={r['question']:r['eligible'] for r in canonical['eligibility']}
    rows = collections.defaultdict(list)
    scheduled = set()
    extraction_scores=replay_extractions(local['cases'],answers,m,scheduled)
    for c,q,e,model,vision,order in cells(local['cases']):
        k = (c['case_id'],q,e,model,vision,order)
        scheduled.update(k+(mode,) for mode in m['modes'])
        request = read(Path(c['folder'])/'packet'/f'{q}.json')
        base_request=request
        for mode in m['modes']:
            request=base_request
            actual_vision=vision
            r = answers.get(k+(mode,))
            outcome = 'deferred'
            if e!='deterministic' and not c['egress_allowed']:
                outcome = 'denied'
                if r and (r.get('outcome') != 'denied' or r.get('attempts',0)!=0 or r.get('answer') is not None):
                    raise ValueError('recorded provider answer for denied content')
            elif r:
                if r['manifest_sha256'] != file_hash(MANIFEST):
                    raise ValueError('recorded answer does not bind the frozen request')
                if e.startswith('enriched') and (r.get('request_file') or r.get('attempts',0)>0 or r['outcome'] in ['answered','abstained']):
                    reference=(LOCAL/r.get('request_file','')).resolve()
                    if not reference.is_relative_to(LOCAL.resolve()) or not reference.is_file():
                        raise ValueError('enriched request must be a private local hash-checked reference')
                    enriched=read(reference)
                    context=enriched['evidence'].get('observation_context')
                    if enriched['case_id']!=request['case_id'] or enriched['question']!=request['question'] or not enriched['evidence']['observations'] or not context or context['extractor']['model'] not in MODELS:
                        raise ValueError('enrichment case, catalog or actual extractor differs')
                    extraction=answers.get((c['case_id'],'visual-observation/1','shared_observations',vision,None,order,mode))
                    if not extraction or extraction['outcome'] not in ['answered','abstained'] or context['extractor']['model']!=extraction.get('actual_model',vision):
                        raise ValueError('enriched answer requires its counted shared observation extraction')
                    actual_vision=context['extractor']['model']
                    if mode=='pinned_provider' and actual_vision!=vision:
                        raise ValueError('pinned replay forbids fallback substitution')
                    request=enriched
                else:
                    reference=Path(c['folder'])/'packet'/f'{q}.json'
                if r['request_sha256'] != file_hash(reference):
                    raise ValueError('recorded answer does not bind the frozen request')
                if mode=='pinned_provider' and r.get('actual_model',model)!=model:
                    raise ValueError('pinned replay forbids fallback substitution')
                outcome = r['outcome']
            if e=='deterministic':
                rule={'triage.route.v1':'needs_eyes','vision.route.v1':'human_directly',
                      'perf.interpret.v1':'collect_more_evidence','capture.disposition.v1':'needs_human',
                      'intent.match.v1':'insufficient_intent'}[q]
                r=dict(answer=rule,outcome='answered',attempts=0)
                outcome='answered'
            t = truth.get(c['case_id'],{}).get('answers',{}).get(q) if eligibility.get(c['case_id'],{}).get(q) else None
            obs = dict(case_id=c['case_id'],question=q,provider='rules' if e=='deterministic' else ('gemini' if e=='gemini_alone' else 'jev'),
                       model=r.get('actual_model',model) if r else model, vision_model=actual_vision,
                       depends_on_model_observation=e.startswith('enriched'),truth=t,
                       answer=r.get('answer') if r else None,probability=r.get('probability') if r else None,
                       outcome=outcome,attempts=r.get('attempts',0) if r else 0,
                       latency_ms=r.get('latency_ms') if r else None,usage=r.get('usage') if r else None,
                       cost=r.get('cost') if r else None,order=order,critical_error=False)
            rows[(mode,c['split'],e)].append(dict(observation=obs,request=request,synthetic=False))
    if set(answers)-scheduled:
        raise ValueError('unscheduled case/question/model/order')
    output = []
    for (mode,split,e), group in sorted(rows.items()):
        file = LOCAL/'score-input.json'
        save(file,group)
        scores = json.loads(run([helper,'score',file]))
        output.append(dict(mode=mode,split=split,encoding=e,**scores))
    pixel=collections.defaultdict(lambda:collections.Counter())
    for r in records:
        if r['question']!='vision.route.v1' or r['outcome']!='answered':
            continue
        human=truth.get(r['case_id'])
        if not human or not eligibility.get(r['case_id'],{}).get('vision.route.v1'):
            continue
        predicted=r.get('requires_pixels')
        observed=human['vision']['requires_pixels']
        if predicted not in ['yes','no','undetermined']:
            continue
        group=pixel[(r.get('mode','pinned_provider'),r['encoding'],r.get('actual_model',r['model']),r.get('vision_model'))]
        group[observed+' -> '+predicted]+=1
    pixel_metrics=[]
    for (mode,e,model,vision),counts in pixel.items():
        true_yes=sum(n for key,n in counts.items() if key.startswith('yes -> '))
        predicted_yes=sum(n for key,n in counts.items() if key.endswith(' -> yes'))
        hits=counts['yes -> yes']
        pixel_metrics.append(dict(mode=mode,encoding=e,model=model,vision_model=vision,confusion=dict(counts),
                                  pixel_requirement_recall=hits/true_yes if true_yes else None,
                                  pixel_requirement_precision=hits/predicted_yes if predicted_yes else None,
                                  unnecessary_visual_requests=counts['no -> yes']))
    result = dict(schema='saccade-evaluation.v1',manifest_sha256=file_hash(MANIFEST),
                  dataset_sha256=m['dataset_sha256'],split_sha256=m['split_sha256'],
                  real_cases=len(local['cases']),human_labeled_cases=len(truth),
                  missing_human_labels=len(local['cases'])-len(truth),provider_calls=0,
                  applicable={q:sum(q in c['questions'] for c in local['cases']) for q in QUESTIONS},
                  missing_retest_cases=len(m['test_retest']['case_ids'])-sum(r['stage']=='final' for r in labels('retest')),
                  denied_cases=sum(not c['egress_allowed'] for c in local['cases']),
                  proposed_egress_cases=sum(c['egress_allowed'] for c in local['cases']),
                  labeler_count=1,limitation='single-labeler pilot; no inter-reviewer reliability estimate',
                  test_retest=retest_metrics(labels('initial'),labels('retest')),
                  incomplete=len(truth)<len(local['cases']) or any(r['observation']['outcome'] in ['deferred','budget_blocked'] for group in rows.values() for r in group)
                             or any(s['deferred'] or s['budget_blocked'] for s in extraction_scores),
                  qualified=False,quality_ranking=None,pixel_need_metrics=pixel_metrics,
                  shared_observation_metrics=extraction_scores,scores=output)
    save(HERE/'replay-summary.json',result)
    return result


def dry_run(helper):
    m, local = validate()
    c = local['cases'][0]
    adapter_replay=json.loads(run([helper,'conformance',Path(c['folder'])/'packet']))
    assert adapter_replay['adapter_round_trip'] and adapter_replay['real_support']==0
    q = 'vision.route.v1'
    request = read(Path(c['folder'])/'packet'/f'{q}.json')
    outcomes = ['answered','answered','abstained','invalid','unavailable','deferred','denied','budget_blocked']
    controls = []
    for i,o in enumerate(outcomes):
        obs = dict(case_id=digest(['synthetic-control',i//2]),question=q,provider='mock',model='mock/1',
                   vision_model=None,depends_on_model_observation=False,truth='text_sufficient',
                   answer=('text_sufficient' if i==0 else 'inspect_full_frame') if o=='answered' else ('abstain' if o=='abstained' else None),
                   probability=0.8 if o=='answered' else None,outcome=o,attempts=1 if i<5 else 0,
                   latency_ms=10 if i<5 else None,usage=None,cost=None,
                   order='ab' if i%2==0 else 'ba',critical_error=False)
        controls.append(dict(observation=obs,request=request,synthetic=True))
    f = LOCAL/'control-input.json'
    save(f,controls)
    score = json.loads(run([helper,'score',f]))
    stats = score['controls'][0]['metrics']
    assert not score['strata'] and stats['conditional_accuracy']==0.5
    assert stats['answer_coverage']==2/7 and stats['availability']==3/7
    assert stats['unavailable']==1 and stats['budget_blocked']==1 and stats['both_order_disagreement']==1
    summary = replay(None,helper)
    assert summary['quality_ranking'] is None and not summary['qualified']
    assert all(s['metrics']['conditional_accuracy'] is None for group in summary['scores'] for s in group['strata'])
    save(HERE/'dry-run.json',dict(schema='saccade-evaluation.v1',provider_calls=0,
                                manifest_sha256=file_hash(MANIFEST),dataset_sha256=m['dataset_sha256'],
                                split_sha256=m['split_sha256'],rubric_sha256=m['rubric_sha256'],
                                synthetic_controls_excluded_from_real_support=True,
                                controls=stats,adapter_replay=adapter_replay,real_cases=summary['real_cases'],
                                coverage_accuracy_separated=True,no_label_run_unqualified=True,
                                incomplete_result_preserved=True,qualified=False))
    print('PASS: offline replay, grouped scores, failures, denied cells and no-label handling')


class Workbench(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def respond(self, status, data, mime='application/json'):
        self.send_response(status)
        self.send_header('Content-Type',mime)
        self.send_header('Cache-Control','no-store')
        self.send_header('Content-Security-Policy',"default-src 'self'; img-src 'self'; script-src 'self'; style-src 'self'")
        self.end_headers()
        self.wfile.write(data if isinstance(data,bytes) else encoded(data))

    def route(self):
        if self.headers.get('Host') != f'127.0.0.1:{self.server.server_port}':
            raise ValueError('invalid local host')
        parts = urlsplit(self.path).path.strip('/').split('/')
        if not parts or parts[0]!=self.server.token:
            raise ValueError('session token required')
        return parts[1:]

    def current(self):
        previous = labels(self.server.phase)
        completed = {r['case_id'] for r in previous if r['stage']=='final'}
        return next((c for c in self.server.cases if c['case_id'] not in completed),None)

    def do_GET(self):
        try:
            route = self.route()
            if route in [[],['workbench.js'],['workbench.css']]:
                name = 'workbench.html' if not route else route[0]
                mime = {'workbench.html':'text/html','workbench.js':'text/javascript','workbench.css':'text/css'}[name]
                return self.respond(200,(HERE/name).read_bytes(),mime)
            if route==['egress']:
                # Exact private scenes and citations are local to this session.
                return self.respond(200,[dict(case_id=c['case_id'],scene=c['private']['scene'],origin=c['origin'],proposed_egress=c['egress_allowed'],evidence=c['private'].get('license_evidence'),records=c['private']['records'],reason=c['private']['egress_reason']) for c in self.server.cases])
            c = self.current()
            if not c:
                return self.respond(200,dict(complete=True))
            prior = labels(self.server.phase)
            structured = any(r['case_id']==c['case_id'] and r['stage']=='structured' for r in prior)
            if route==['case']:
                packet = Path(c['folder'])/'packet'
                requests = {q:read(packet/f'{q}.json') for q in c['questions']}
                # Initial and retest workbench never load model answers or previous-phase labels.
                return self.respond(200,dict(case_id=c['case_id'],position=self.server.cases.index(c)+1,total=len(self.server.cases),
                                            stage='final' if structured else 'structured',requests=requests,
                                            views=c['view_count'],regions=[p['views'] for p in read(Path(c['folder'])/'packet-index.json')['presentations'] if p['order']=='ab'],
                                            region_seen=c['case_id'] in self.server.region_seen,full_seen=c['case_id'] in self.server.full_seen))
            if len(route)==3 and route[0]=='image':
                if not structured:
                    raise ValueError('structured assessment required before pixels')
                view=int(route[1])
                if not 0<=view<c['view_count']:
                    raise ValueError('unknown view')
                swap = int(digest([self.server.seed,c['case_id']])[-1],16)%2
                order = 'ba' if swap else 'ab'
                directory = Path(c['folder'])/'packet'/f'view-{view}-{order}'
                index = read(directory/'presentation.json')
                name=route[2]
                allowed = {v['id']+'.png':v for v in index['views']}
                if name not in allowed:
                    raise ValueError('unknown anonymous image')
                if allowed[name]['kind']=='full_frame':
                    self.server.full_seen.add(c['case_id'])
                else:
                    self.server.region_seen.add(c['case_id'])
                return self.respond(200,(directory/name).read_bytes(),'image/png')
            raise ValueError('unknown route')
        except (ValueError,KeyError,FileNotFoundError) as e:
            self.respond(400,dict(error=str(e)))

    def do_POST(self):
        try:
            route = self.route()
            if self.headers.get('Origin') not in [None,f'http://127.0.0.1:{self.server.server_port}']:
                raise ValueError('local origin required')
            length=int(self.headers.get('Content-Length',0))
            if length<=0 or length>65536:
                raise ValueError('bounded label required')
            r=json.loads(self.rfile.read(length))
            c=self.current()
            if route!=['label'] or c is None:
                raise ValueError('unknown active case')
            if r['stage']=='final':
                v=r['vision']
                if v['minimum_scope']=='regions' and c['case_id'] not in self.server.region_seen:
                    raise ValueError('no available region was inspected')
                if v['minimum_scope']=='full_frame' and c['case_id'] not in self.server.full_seen:
                    raise ValueError('full frame was not inspected')
            record_label(c,r,self.server.phase,self.server.token,
                         ['anonymous-region-inspected'] if c['case_id'] in self.server.region_seen else [],
                         c['case_id'] in self.server.full_seen)
            self.respond(200,dict(saved=True))
        except (ValueError,KeyError,TypeError) as e:
            self.respond(400,dict(error=str(e)))


def serve(phase, port):
    m, local=validate()
    cases=copy.deepcopy(local['cases'])
    seed=m['presentation_seed'] if phase=='initial' else m['retest_seed']
    if phase=='retest':
        ids=retest_ids(m,labels('initial'),int(time.time()*1000))
        cases=[c for c in cases if c['case_id'] in ids]
    random.Random(seed).shuffle(cases)
    server=http.server.HTTPServer(('127.0.0.1',port),Workbench)
    server.cases,server.phase,server.seed=cases,phase,seed
    server.token=secrets.token_urlsafe(32)
    server.region_seen,server.full_seen=set(),set()
    print(f'Local labeling: http://127.0.0.1:{server.server_port}/{server.token}/',flush=True)
    print('No provider calls. Leave this terminal open; Ctrl-C stops safely.',flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


def main():
    a=argparse.ArgumentParser(description=__doc__)
    s=a.add_subparsers(dest='command',required=True)
    f=s.add_parser('freeze');f.add_argument('--selection',required=True);f.add_argument('--saccade',required=True);f.add_argument('--helper',required=True)
    s.add_parser('validate')
    w=s.add_parser('label');w.add_argument('--phase',choices=['initial','retest'],default='initial');w.add_argument('--port',type=int,default=8797)
    d=s.add_parser('dry-run');d.add_argument('--helper',required=True)
    r=s.add_parser('replay');r.add_argument('--answers');r.add_argument('--helper',required=True)
    args=a.parse_args()
    if args.command=='freeze':freeze(args.selection,args.saccade,args.helper)
    elif args.command=='validate':validate();print('PASS: selection, sources, splits, packets, rubrics and caps frozen')
    elif args.command=='label':serve(args.phase,args.port)
    elif args.command=='dry-run':dry_run(args.helper)
    elif args.command=='replay':replay(args.answers,args.helper);print('Offline replay summary written')


if __name__=='__main__':
    main()
