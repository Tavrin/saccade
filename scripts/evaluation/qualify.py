#!/usr/bin/env python3
"""R12q frozen expansion runner. Private packets/results never enter the repo.

Usage: qualify.py prepare|count|submit|poll|jev|report --config PRIVATE_CONFIG
The commands are resumable. `submit` enforces the $8 reservation before egress.
"""
import argparse
import collections
import hashlib
import json
import math
from pathlib import Path
import subprocess
import sys
import time
import tomllib

HERE = Path(__file__).resolve().parents[2] / 'examples/evaluation'
sys.path.insert(0, str(HERE))
import corpus  # noqa: E402
import pilot  # noqa: E402
import corpus_live  # noqa: E402

EXPANDED = HERE / 'moss-pilot-expanded.toml'
SEAL = corpus.load(HERE / 'freeze-amendment.json')
CAP_USD = 8.0
PRICES = {'gemini-3.8-flash': (.375, 1.875), 'gemini-3.7-flash': (.75, 4.50),
          'gemini-3.6-flash': (.75, 4.50), 'gemini-3.5-flash': (.75, 4.50)}
# The current price page does not list the older two model IDs. For their eight
# frozen calibration probes, the budget reserves at the higher 3.5 batch rate.


def encoded(value):
    return json.dumps(value, separators=(',', ':'), sort_keys=True).encode()


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + '.tmp')
    tmp.write_bytes(encoded(value) + b'\n')
    tmp.replace(path)


def read(path):
    return json.loads(path.read_bytes())


def run(args):
    done = subprocess.run([str(x) for x in args], capture_output=True, check=False)
    if done.returncode:
        # Diagnostics from credential-bearing transports are deliberately withheld.
        raise RuntimeError(f'private helper failed with exit {done.returncode}')
    return json.loads(done.stdout)


def manifest():
    if pilot.file_hash(EXPANDED) != SEAL['expanded_manifest_sha256']:
        raise RuntimeError('expanded manifest seal mismatch')
    return json.loads(tomllib.loads(EXPANDED.read_text())['frozen_json'])


def load_cases(config):
    m = manifest()
    if m['egress'] != 'all cases authorized under section 16':
        raise RuntimeError('frozen egress authorization changed')
    root = Path(config['root'])
    old = read(root/'corpus-mapping.json')['cases']
    new = read(root/'r12x/mapping.json')['cases']
    if [corpus.public_case(c) for c in old + new] != m['cases']:
        raise RuntimeError('private mapping differs from frozen expansion')
    # The original private root was relocated after its public freeze. Use its
    # byte-identical retained packets for the baseline, leaving the seal intact.
    for c in old:
        if not Path(c['folder']).exists():
            relocated = root/'preparation/cases'/Path(c['folder']).name
            if not relocated.is_dir(): raise RuntimeError('old frozen packet unavailable')
            c['folder'] = str(relocated)
    if len(new) != 180 or collections.Counter(c['split'] for c in new) != {'held_out': 90, 'calibration': 36, 'development': 54}:
        raise RuntimeError('expanded split changed')
    for c in new:
        if c['split'] == 'development':
            continue
        for file, digest in c['generated'].items():
            if pilot.file_hash(file) != digest:
                raise RuntimeError('frozen generated packet changed')
    return m, old, new


def selected_jobs(m, new):
    ids = {c['case_id'] for c in new if c['split'] in ('held_out', 'calibration')}
    jobs = [j for j in m['jobs'] if j['case_id'] in ids]
    assert len(jobs) == 1024 and all(j['provider'] != 'rules' or j['encoding'] == 'deterministic' for j in jobs)
    return jobs


def fact(request, name):
    for cell in request['evidence']['facts']:
        if cell['name'] == name and cell['value']['availability'] == 'available':
            value = cell['value']['value']
            if value['type'] == 'text':
                raw = value['value']
                try:
                    return json.loads(raw)
                except (ValueError, TypeError):
                    return raw
    return None


def rule(request):
    q = request['question']['id']
    validity = fact(request, 'validity')
    if q == 'triage.route.v1':
        if validity != 'valid': return 'needs_eyes'
        noise = fact(request, 'noise')
        findings = fact(request, 'deterministic_findings')
        if noise is not None and isinstance(findings, dict) and findings.get('within_noise') is True:
            return 'likely_noise'
        if fact(request, 'structured_intent') is not None and isinstance(findings, dict) and findings.get('consistent') is True:
            return 'likely_intended'
        deltas, regions = fact(request, 'deltas'), fact(request, 'region_facts')
        if isinstance(deltas, list) and isinstance(regions, list) and any(
                isinstance(d, dict) and isinstance(d.get('value'), (int, float)) and d['value'] > .1 for d in deltas
        ) and any(isinstance(r, dict) and r.get('hotspots') for r in regions):
            return 'suspected_regression'
        return 'needs_eyes'
    if q == 'vision.route.v1':
        if fact(request, 'semantic_uncertainty') is not None: return 'human_directly'
        regions = fact(request, 'affected_regions')
        if isinstance(regions, list):
            if any(isinstance(r, dict) and (r.get('regions') or r.get('hotspots')) for r in regions):
                return 'inspect_regions'
            return 'text_sufficient'
        return 'inspect_full_frame'
    if q == 'perf.interpret.v1':
        qualification, repeats, attribution = (fact(request, x) for x in ('qualification','repeats','attribution'))
        if qualification is None or repeats is None or attribution is None: return 'collect_more_evidence'
        findings = fact(request, 'deterministic_findings')
        if isinstance(findings, dict):
            if findings.get('consistent') is True: return 'consistent_with_intent'
            if findings.get('unexplained') is True: return 'unexplained_change'
        return 'needs_human'
    if q == 'capture.disposition.v1':
        if validity == 'invalid':
            checks = fact(request, 'capture_checks')
            return 'inspect_configuration' if isinstance(checks, dict) and checks.get('matching_settings') is False else 'recapture'
        return 'continue_review' if validity == 'valid' else 'needs_human'
    if q == 'intent.match.v1':
        if fact(request, 'structured_intent') is None: return 'insufficient_intent'
        findings = fact(request, 'deterministic_findings')
        if isinstance(findings, dict):
            if findings.get('contradicts') is True: return 'contradicts'
            if findings.get('consistent') is True: return 'consistent'
        return 'abstain'
    raise RuntimeError('unknown frozen question')


def prepare(config):
    m, old, new = load_cases(config)
    dest = Path(config['root'])/'r12q'
    baseline = dest/'baseline-predictions.json'
    if not baseline.exists():
        predictions = {}
        for c in old + new:
            if c['split'] != 'held_out': continue
            predictions[c['case_id']] = {q: rule(corpus.load(Path(c['folder'])/'packet'/f'{q}.json')) for q in c['questions']}
        save(baseline, {'policy': 'DECISIONS.md/R12q-2026-10-04T17:56:39Z',
                        'manifest_sha256': SEAL['expanded_manifest_sha256'], 'predictions': predictions})
    jobs = selected_jobs(m, new)
    summary = {'baseline_sha256': pilot.file_hash(baseline), 'new_cases': len(new),
               'run_splits': dict(collections.Counter(c['split'] for c in new if c['split'] != 'development')),
               'jobs': dict(collections.Counter(j['provider'] for j in jobs))}
    save(dest/'plan.json', summary)
    print(json.dumps(summary, sort_keys=True))


def payloads(config):
    m, _, new = load_cases(config)
    cases = {c['case_id']: c for c in new}
    return m, cases, [j for j in selected_jobs(m,new) if j['provider'] == 'gemini']


def count(config):
    m, cases, jobs = payloads(config)
    dest = Path(config['root'])/'r12q'
    if not (dest/'baseline-predictions.json').exists(): raise RuntimeError('freeze baseline first')
    counts = read(dest/'token-counts.json') if (dest/'token-counts.json').exists() else {}
    for i, job in enumerate(jobs):
        model = job.get('model') or m['gemini_chain'][0]
        if model not in PRICES: raise RuntimeError('unpriced frozen model')
        case = cases[job['case_id']]
        packet, identity = corpus_live.gemini_payload(case, job, corpus_live.requests(case, job['questions']))
        record = {'job': job, 'identity': identity, 'model': model, 'request_sha256': pilot.digest(packet)}
        folder = dest/'gemini'/job['job_id'][7:]
        save(folder/'request.json', packet)
        if job['job_id'] in counts:
            if counts[job['job_id']]['request_sha256'] != record['request_sha256']:
                raise RuntimeError('token count request hash changed')
            continue
        value = run([config['batch'], '--count', model, folder/'request.json'])
        record['input_tokens'] = value['totalTokens']
        record['max_output_tokens'] = packet['generationConfig']['maxOutputTokens']
        counts[job['job_id']] = record
        save(dest/'token-counts.json', counts)
        if (i + 1) % 20 == 0: print(f'token counts {i+1}/{len(jobs)}', flush=True)
    total = sum((v['input_tokens']*PRICES[v['model']][0]+v['max_output_tokens']*PRICES[v['model']][1])/1e6 for v in counts.values())
    print(json.dumps({'counted':len(counts),'planned':len(jobs),'upper_estimate_usd':round(total,6)}))


def submit(config):
    m, cases, jobs = payloads(config)
    dest = Path(config['root'])/'r12q'
    counts = read(dest/'token-counts.json')
    planned = [j for j in jobs if j['job_id'] in counts]
    if len(planned) < len(jobs):
        raise RuntimeError('count every priced job before submission')
    total = sum((v['input_tokens']*PRICES[v['model']][0]+v['max_output_tokens']*PRICES[v['model']][1])/1e6 for v in counts.values())
    if total > CAP_USD: raise RuntimeError('Gemini $8 hard cap: token estimate exceeds cap')
    budget = dest/'gemini-budget.json'
    if budget.exists():
        prior = read(budget)
        if prior['reserved_usd'] != round(total, 6): raise RuntimeError('budget reservation changed')
    else:
        save(budget, {'cap_usd':CAP_USD,'reserved_usd':round(total,6),
                      'source':'https://ai.google.dev/gemini-api/docs/pricing',
                      'mode':'prompt tokens plus maximum output tokens; no further paid Gemini submission allowed'})
    # Each chunk is kept below the published 20 MB inline limit. Persist a
    # dispatch receipt before the request, so a crash never blindly resubmits.
    grouped = collections.defaultdict(list)
    for j in planned: grouped[counts[j['job_id']]['model']].append(j)
    for model, model_jobs in grouped.items():
        chunks, current = [], []
        for job in model_jobs:
            item = {'request':read(dest/'gemini'/job['job_id'][7:]/'request.json'),
                    'metadata':{'job_id':job['job_id'],'request_sha256':counts[job['job_id']]['request_sha256']}}
            if current and len(encoded({'batch':{'displayName':'r12q','inputConfig':{'requests':{'requests':current+[item]}}}})) >= 18_000_000:
                chunks.append(current); current = []
            current.append(item)
        if current: chunks.append(current)
        for n, chunk in enumerate(chunks):
            folder = dest/'batches'/model/f'{n:03d}'
            folder.mkdir(parents=True,exist_ok=True)
            save(folder/'requests.json',chunk)
            body = {'batch':{'displayName':f'r12q-{model}-{n}', 'inputConfig':{'requests':{'requests':chunk}}}}
            save(folder/'submission.json',body)
            if (folder/'operation.json').exists(): continue
            if (folder/'submit-started.json').exists():
                raise RuntimeError('uncertain batch submission; inspect provider before retry')
            save(folder/'submit-started.json',{'submission_sha256':pilot.file_hash(folder/'submission.json'),
                                                'started_ms':int(time.time()*1000)})
            operation = run([config['batch'],'--submit',model,folder/'submission.json'])
            save(folder/'operation.json',operation)
            print(f'batch submitted {model} {n+1}/{len(chunks)} {operation["name"]}',flush=True)


def poll(config):
    dest = Path(config['root'])/'r12q'
    for file in sorted((dest/'batches').glob('*/*/operation.json')):
        folder = file.parent
        op = read(file)
        if (folder/'collected.json').exists(): continue
        if read(folder/'submit-started.json')['submission_sha256'] != pilot.file_hash(folder/'submission.json'):
            raise RuntimeError('submitted batch payload changed')
        if read(folder/'submission.json')['batch']['inputConfig']['requests']['requests'] != read(folder/'requests.json'):
            raise RuntimeError('batch request records changed')
        updated = run([config['batch'],'--get',op['name']])
        save(folder/'latest.json',updated)
        state = updated.get('state') or updated.get('metadata',{}).get('state')
        if state not in ('BATCH_STATE_SUCCEEDED','JOB_STATE_SUCCEEDED'):
            print(f'{op["name"]} {state}',flush=True)
            continue
        save(folder/'complete.json',updated)
        results = run([config['batch'],'--collect',folder/'complete.json',folder/'requests.json'])
        for result in results:
            case_folder = dest/'gemini'/result['job_id'][7:]
            save(case_folder/'response.json',result['response'])
        save(folder/'collected.json',{'jobs':len(results),'response_hashes':{r['job_id']:pilot.file_hash(dest/'gemini'/r['job_id'][7:]/'response.json') for r in results}})
        print(f'{op["name"]} collected {len(results)}',flush=True)


def gemini_results(config, m, cases, jobs):
    """Decode only hash-bound collected answers through the existing decoder."""
    dest = Path(config['root'])/'r12q'
    output = {}
    counts = read(dest/'token-counts.json') if (dest/'token-counts.json').exists() else {}
    collected = {job:digest for file in (dest/'batches').glob('*/*/collected.json')
                 for job,digest in read(file)['response_hashes'].items()}
    for job in jobs:
        job_id = job['job_id']
        if job_id not in collected: continue
        case = cases[job['case_id']]
        folder = dest/'gemini'/job_id[7:]
        response = folder/'response.json'
        if pilot.file_hash(response) != collected[job_id] or pilot.digest(read(folder/'request.json')) != counts[job_id]['request_sha256']:
            raise RuntimeError('batch request/response record changed')
        envelope = read(response)
        model = counts[job_id]['model']
        if 'batch_error_status' in envelope:
            output[job_id] = {'job':job,'outcome':'unavailable',
                              'reason':f"batch request {envelope['batch_error_status']}",
                              'answers':{},'attempts':1,'case_id':job['case_id']}
            continue
        identity = corpus_live.gemini_payload(case, job, corpus_live.requests(case,job['questions']))[1]
        exchanges = [{'batch':True,'request_sha256':counts[job_id]['request_sha256'],
                      'response_sha256':pilot.file_hash(response)}]
        try:
            vision = corpus_live.decode_gemini(case, job, envelope, identity, model, response, exchanges)
        except (ValueError, KeyError, TypeError, AttributeError) as error:
            output[job_id] = {'job':job,'outcome':'invalid','reason':str(error),'answers':{},'attempts':1}
            continue
        request_files = {}
        for request in corpus_live.enriched(config, m, case, dict(job, encoding='enriched'), vision, folder/'validation'):
            path = folder/'requests-used'/f"{request['question']['id']}.json"
            save(path, request)
            request_files[request['question']['id']] = str(path)
        output[job_id] = {'job':job,'outcome':'answered','answers':{q:{'answer':a,'probability':None} for q,a in vision['answers'].items()},
                          'case_id':job['case_id'],'vision':vision,'actual_model':model,
                          'revision':vision['revision'],'usage':vision['usage'],'attempts':1,
                          'exchanges':exchanges,'request_files':request_files}
    return output


def jev(config):
    m, cases, jobs = payloads(config)
    root = Path(config['root'])/'r12q'
    local = dict(config, root=str(root))
    state = root/'run-state.json'
    if not state.exists(): save(state,{'started_ms':int(time.time()*1000),
                                       'manifest_sha256':SEAL['expanded_manifest_sha256']})
    corpus.MANIFEST = EXPANDED
    vision_jobs = gemini_results(config,m,cases,jobs)
    by_case = {(r['case_id'],r['job']['order']):r for r in vision_jobs.values() if r.get('vision') and r['job']['mode']=='production_policy'}
    all_jobs = [j for j in selected_jobs(m,list(cases.values())) if j['provider']=='jev']
    done = 0
    for j in sorted(all_jobs,key=lambda x: (x['encoding']!='direct', cases[x['case_id']]['split']!='held_out')):
        folder = root/'jev'/j['job_id'][7:]
        result_file = folder/'result.json'
        if result_file.exists(): done += 1; continue
        case = cases[j['case_id']]
        reqs = corpus_live.requests(case,j['questions'])
        if j['encoding'].startswith('enriched'):
            vision = by_case.get((j['case_id'],j['order']))
            partner = by_case.get((j['case_id'],'ba' if j['order']=='ab' else 'ab'))
            if not vision or not partner or (vision['actual_model'],vision['revision']) != (partner['actual_model'],partner['revision']):
                save(result_file, {'job':j,'case_id':j['case_id'],'split':case['split'],
                                   'outcome':'unavailable','reason':'required same-model visual pair unavailable',
                                   'answers':{},'attempts':0,'request_files':{},'exchanges':[]})
                done += 1
                continue
            reqs = corpus_live.enriched(local,m,case,j,vision['vision'],folder)
        folder.mkdir(parents=True,exist_ok=True)
        result = {'job':j,'case_id':j['case_id'],'split':case['split'],'outcome':'unavailable',
                  'answers':{},'attempts':0,'exchanges':[], 'request_files':{},
                  'manifest_sha256':SEAL['expanded_manifest_sha256'],'actual_model':None,'revision':None}
        for req in reqs:
            path = folder/'requests-used'/f"{req['question']['id']}.json"
            save(path,req)
            result['request_files'][req['question']['id']] = str(path)
        corpus_live.jev_dispatch(local,m,case,j,reqs,folder,result)
        save(result_file,result)
        done += 1
        if done % 10 == 0: print(f'Jev {done}/{len(all_jobs)}',flush=True)
    print(json.dumps({'jev_jobs_completed':done,'jev_jobs_scheduled':len(all_jobs)}))


def wilson(correct, total):
    if not total: return None
    z = 1.959963984540054
    p = correct/total
    mid = (p+z*z/(2*total))/(1+z*z/total)
    half = z*math.sqrt(p*(1-p)/total+z*z/(4*total*total))/(1+z*z/total)
    return [round(max(0,mid-half),4),round(min(1,mid+half),4)]


def score_cells(config, m, cases, results):
    """Apply the production evaluator to each exact observation stratum."""
    selected = dict(m, jobs=[r['job'] for r in results])
    rows = corpus_live.rows_for_results(selected, {'cases':cases},results)
    groups = collections.defaultdict(list)
    for row in rows:
        o=row['observation']
        key=(row['mode'],row['encoding'],row['split'],o['provider'],o['model'],o['vision_model'],o['question'],o['depends_on_model_observation'])
        groups[key].append(row)
    output=[];calibrators=[]
    for key, group in groups.items():
        path=Path(config['root'])/'r12q/scores'/f'{pilot.digest(key)[7:]}.json'
        save(path,[{k:v for k,v in r.items() if k in ('observation','request','synthetic')} for r in group])
        scored=run([config['helper'],'score',path])
        if key[2]=='calibration':
            calibrators.append({'question':key[6],'provider':key[3],'encoding':key[1],
                                'mode':key[0],'model':key[4],'vision_model':key[5],
                                'depends_on_model_observation':key[7],
                                'identity_sha256':pilot.digest([m['dataset_sha256'],m['split_sha256'],
                                                               m['rubric_sha256'],m['adapters'],key]),
                                'fit':scored.get('fit'),
                                'fit_cases':len({o['case_id'] for o in scored['observations']
                                                 if o['truth'] is not None and o['outcome']=='answered' and o.get('probability') is not None})})
        for before,o in zip(group,scored['observations'],strict=True):
            output.append({'case_id':o['case_id'],'question':o['question'],'provider':key[3],
                           'encoding':key[1],'split':key[2],'mode':key[0],
                           'outcome':o['outcome'],'truth':o['truth'],'answer':o['answer'],
                           'model':key[4],'vision_model':key[5],
                           'depends_on_model_observation':key[7],
                           'critical_error':o['critical_error'],
                           'constraint_invalidated':before['observation']['outcome'] in ('answered','abstained') and o['outcome']=='invalid'})
    return output,calibrators


def report(config, publish=False):
    m, old, new = load_cases(config)
    cases = {c['case_id']:c for c in new}
    root=Path(config['root'])/'r12q'
    all_jobs=selected_jobs(m,new)
    gemini=gemini_results(config,m,cases,[j for j in all_jobs if j['provider']=='gemini'])
    jev_results={r['job']['job_id']:r for file in (root/'jev').glob('*/result.json')
                 for r in [read(file)]}
    baseline=read(root/'baseline-predictions.json')
    if baseline['manifest_sha256'] != SEAL['expanded_manifest_sha256']:
        raise RuntimeError('baseline seal changed')
    rule_results=[]
    for j in m['jobs']:
        if j['provider']!='rules' or j['case_id'] not in baseline['predictions']: continue
        c=next((c for c in old+new if c['case_id']==j['case_id']),None)
        if c is None: continue
        rule_results.append({'job':j,'case_id':j['case_id'],'split':c['split'],'outcome':'answered',
                             'answers':{q:{'answer':baseline['predictions'][j['case_id']][q],
                                            'probability':None} for q in j['questions']},
                             'revision':'rules/r12q','actual_model':'rules/r12q','attempts':0})
    new_results=list(gemini.values())+list(jev_results.values())
    scored,calibrators=score_cells(config,m,old+new,rule_results+new_results)
    original=read(HERE/'RESULTS.json')
    old_counts=collections.defaultdict(lambda:[0,0])
    old_misses={}
    old_violations={}
    for r in original['per_question']:
        if r['provider']=='rules': continue
        key=(r['question'],r['provider'],r['encoding'])
        old_misses[key]=r.get('important_miss_bounds_by_case') or {}
        old_violations[key]=r.get('qualification_gates',{}).get('deterministic_constraint_violations',0)
    for s in original['strata']:
        if s['split']!='held_out' or s['provider']=='rules': continue
        key=(s['question'],s['provider'],s['encoding'])
        n=s['metrics'].get('labelled_committed',0)
        acc=s['metrics'].get('conditional_accuracy')
        old_counts[key][0]+=round(n*acc) if acc is not None else 0
        old_counts[key][1]+=n
    totals=collections.defaultdict(lambda:[0,0])
    for o in scored:
        if o['split']!='held_out' or o['truth'] is None or o['outcome']!='answered': continue
        key=(o['question'],o['provider'],o['encoding'])
        totals[key][0]+=int(o['answer']==o['truth'])
        totals[key][1]+=1
    # Old model rows are immutable; the frozen no-model rule is evaluated anew
    # on all original and expanded held-out cases.
    rows=[]
    for q in pilot.QUESTIONS:
        base_correct,base_n=totals[(q,'rules','deterministic')]
        baseline_acc=base_correct/base_n if base_n else None
        for provider,encodings in [('rules',['deterministic']),('jev',['direct','enriched','enriched_without_priors']),('gemini',['gemini_alone'])]:
            for encoding in encodings:
                key=(q,provider,encoding)
                a,b=totals[key]
                if provider!='rules': a+=old_counts[key][0];b+=old_counts[key][1]
                acc=a/b if b else None
                relevant=[j for j in all_jobs if j['provider']==provider and j['encoding']==encoding and q in j['questions']]
                attempted=sum((jev_results.get(j['job_id']) or gemini.get(j['job_id']) or {}).get('attempts',0)>0 for j in relevant)
                support=SEAL['held_out_known_truth'][q]
                completed=1.0 if provider=='rules' else (attempted/len(relevant) if relevant else None)
                applicable = bool(relevant) or provider != 'gemini'
                order_factor = 2 if encoding in ('gemini_alone','enriched','enriched_without_priors') else 1
                coverage = b/(support*order_factor) if applicable and support else None
                by_case=collections.defaultdict(list)
                for o in scored:
                    if o['split']=='held_out' and (o['question'],o['provider'],o['encoding'])==key and o['truth'] is not None and o['outcome'] in ('answered','abstained','invalid'):
                        by_case[o['case_id']].append(o)
                miss_cases=len(by_case)+(old_misses.get(key,{}).get('cases') or 0)
                misses=sum(any(o['critical_error'] for o in values) for values in by_case.values())+(old_misses.get(key,{}).get('misses') or 0)
                violations=sum(o['constraint_invalidated'] for o in scored if o['split']=='held_out' and (o['question'],o['provider'],o['encoding'])==key)+old_violations.get(key,0)
                held_identities={(o['mode'],o['model'],o['vision_model'],o['depends_on_model_observation'])
                                 for o in scored if o['split']=='held_out' and (o['question'],o['provider'],o['encoding'])==key and o['outcome']=='answered'}
                fitted_identities={(c['mode'],c['model'],c['vision_model'],c['depends_on_model_observation'])
                                   for c in calibrators if (c['question'],c['provider'],c['encoding'])==key and c['fit_cases'] and isinstance(c['fit'],dict) and c['fit'].get('fitted')}
                matching_calibration=None if provider=='rules' else bool(held_identities) and held_identities <= fitted_identities
                rows.append({'question':q,'provider':provider,'encoding':encoding,'held_out_cases':support,
                             'n':b,'correct':a,'accuracy':acc,'wilson_95':wilson(a,b),'coverage':coverage,
                             'ci_unit':'committed answers; presentation orders are correlated',
                             'baseline_accuracy':baseline_acc,'margin':acc-baseline_acc if acc is not None and baseline_acc is not None else None,
                             'important_miss_cases':miss_cases,'important_misses':misses,
                             'important_miss_wilson_95':wilson(misses,miss_cases),
                             'deterministic_constraint_violations':violations,
                             'matching_calibration_identity':matching_calibration,
                             'attempted_completion_new':completed,
                             'qualified':False,'failed_gates':['important-miss tolerance undeclared']+
                               (['matching qualified calibration absent'] if provider!='rules' and not matching_calibration else [])+
                               (['attempted completion below 95%'] if completed is not None and completed < .95 else [])+
                               (['deterministic constraint violation observed'] if violations else [])})
    usage=collections.Counter()
    usage_by_model=collections.defaultdict(lambda:{'responses':0,'input_tokens':0,'output_tokens':0,'estimated_usd':0.0})
    actual=0.0
    missing_usage=0
    counts=read(root/'token-counts.json')
    for job_id,r in gemini.items():
        envelope=read(root/'gemini'/job_id[7:]/'response.json')
        if 'batch_error_status' in envelope: continue
        u=envelope.get('usageMetadata') or {}
        prompt=u.get('promptTokenCount');total=u.get('totalTokenCount')
        if not isinstance(prompt,int) or not isinstance(total,int) or total<prompt:
            missing_usage+=1;continue
        model=counts[job_id]['model'];usage[model]+=1
        cost=(prompt*PRICES[model][0]+(total-prompt)*PRICES[model][1])/1e6
        actual+=cost
        usage_by_model[model]['responses']+=1
        usage_by_model[model]['input_tokens']+=prompt
        usage_by_model[model]['output_tokens']+=total-prompt
        usage_by_model[model]['estimated_usd']+=cost
    jev_usage=collections.Counter()
    for r in jev_results.values():
        for k,v in (r.get('usage') or {}).items():
            if isinstance(v,int):jev_usage[k]+=v
    budget=read(root/'gemini-budget.json') if (root/'gemini-budget.json').exists() else None
    gemini_finish=collections.Counter()
    for job_id in gemini:
        envelope=read(root/'gemini'/job_id[7:]/'response.json')
        if 'batch_error_status' in envelope:
            gemini_finish['batch_error']+=1
        else:
            gemini_finish[(envelope.get('candidates') or [{}])[0].get('finishReason','missing')]+=1
    ledger=read(root/'ledger/evaluation.json') if (root/'ledger/evaluation.json').exists() else {'attempts':[]}
    jev_attempts=sum(a['provider']=='jev' for a in ledger['attempts'])
    epoch={'epoch':'r12q-expanded/1','expanded_manifest_sha256':SEAL['expanded_manifest_sha256'],
           'baseline_sha256':pilot.file_hash(root/'baseline-predictions.json'),
           'runner_sha256':pilot.file_hash(Path(__file__)),
           'batch_transport_source_sha256':pilot.file_hash(Path(__file__).resolve().parents[2]/'crates/saccade-core/src/judge_provider/batch.rs'),
           'original_source_revalidation':original.get('source_revalidation'),
           'calibration_needed':True,'development_cases_dispatched':0,
           'new_held_out_cases':90,'new_calibration_cases':36,
           'original_cases_redispatched':0,'combined_held_out_known_truth':SEAL['held_out_known_truth'],
           'new_jobs_scheduled':dict(collections.Counter(j['provider'] for j in all_jobs)),
           'new_jobs_completed':{'gemini':len(gemini),'jev':len(jev_results)},
           'gemini_finish_reasons':dict(gemini_finish),
           'gemini_budget':budget,'gemini_actual_usage_estimate_usd':round(actual,6),
           'gemini_usage_missing':missing_usage,'gemini_response_models':dict(usage),
           'gemini_usage_by_model':{model:{**row,'estimated_usd':round(row['estimated_usd'],6)}
                                    for model,row in sorted(usage_by_model.items())},
           'gemini_pricing_limit':'3.6 and 3.7 are absent from the current price page; their eight frozen probes use the published 3.5 batch rate as a conservative assumption, not an invoice',
           'jev_usage':dict(jev_usage),'jev_attempts':jev_attempts,'jev_attempt_cap':788,
           'jev_job_outcomes':dict(collections.Counter(r['outcome'] for r in jev_results.values())),
           'rows':rows,'calibrators':calibrators,'qualified_questions':[],
           'processing_complete':len(gemini)==268 and len(jev_results)==630,
           'pricing_url':'https://ai.google.dev/gemini-api/docs/pricing',
           'batch_api_url':'https://ai.google.dev/gemini-api/docs/batch-api'}
    save(root/'preview-results.json',epoch)
    if publish:
        if any(x.get('epoch')==epoch['epoch'] for x in original.get('epochs',[])):
            raise RuntimeError('epoch already published; earlier epochs are immutable')
        source=(HERE/'RESULTS.json').read_text()
        if not source.endswith('}\n') or '"epochs"' in original:
            raise RuntimeError('cannot append epoch without preserving prior JSON')
        (HERE/'RESULTS.json').write_text(source[:-2]+',\n  "epochs": '+json.dumps([epoch],indent=2)+'\n}\n')
        with (HERE/'RESULTS.md').open('a') as out:
            out.write('\n## R12q expanded qualification epoch\n\n')
            out.write(f"Frozen expansion `{epoch['expanded_manifest_sha256']}`; 90 new held-out and 36 calibration cases evaluated, 54 development cases and the original 86 not redispatched. ")
            out.write(f"Provider jobs collected: Gemini {len(gemini)}/268, Jev {len(jev_results)}/630.\n\n")
            out.write(f"Gemini upper token-count reservation ${budget['reserved_usd']:.4f} of $8; observed usage estimate ${actual:.4f} ")
            out.write(f"from [batch pricing]({epoch['pricing_url']}); {missing_usage} responses lack usage. Gemini finish reasons: `{dict(gemini_finish)}`. ")
            out.write(f"Jev attempts {jev_attempts}/788; usage: `{dict(jev_usage)}`.\n\n")
            out.write('| Question | Provider / encoding | n committed | Coverage | Accuracy | Wilson 95% | Rule baseline | Margin | Gate |\n|---|---|---:|---:|---:|---|---:|---:|---|\n')
            for r in rows:
                fmt=lambda x:'—' if x is None else f'{100*x:.1f}%'
                ci='—' if r['wilson_95'] is None else f"{fmt(r['wilson_95'][0])}–{fmt(r['wilson_95'][1])}"
                out.write(f"| {r['question']} | {r['provider']} / {r['encoding']} | {r['n']} | {fmt(r['coverage'])} | {fmt(r['accuracy'])} | {ci} | {fmt(r['baseline_accuracy'])} | {fmt(r['margin'])} | not qualified |\n")
            out.write('\nWilson answer intervals use committed responses; paired presentation orders are correlated. Important-miss Wilson bounds in RESULTS.json group by case. These intervals cannot authorize routing.\n')
            out.write('Original recorded upstream sources remain unavailable for revalidation; the sealed original aggregate epoch is retained.\n')
            out.write('\nNo question qualifies: the frozen important-miss tolerance is undeclared. Other failed gates are recorded per row in RESULTS.json.\n')
    print(json.dumps({'processing_complete':epoch['processing_complete'],'gemini_actual_usd':epoch['gemini_actual_usage_estimate_usd'],
                      'gemini_jobs':len(gemini),'jev_jobs':len(jev_results),'rows':rows},indent=2))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('command',choices=['prepare','count','submit','poll','jev','report'])
    p.add_argument('--config',required=True)
    p.add_argument('--publish',action='store_true')
    args=p.parse_args()
    config=read(Path(args.config))
    if args.command=='report': report(config,args.publish)
    else: globals()[args.command](config)


if __name__ == '__main__': main()
