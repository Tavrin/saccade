#!/usr/bin/env python3
"""Offline judge-to-constructed-calibration adapter. No credentials or provider execution."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from judge_gate import reduce
from video_scores import read, decode


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()).hexdigest()


def adapt(manifest, schedule, rows):
    """Exact manifest/request/sample denominator; unavailable evidence never gets a score."""
    if manifest['schema'] != 'saccade-motion-degradations.v1' or schedule['schema'] != 'saccade-judge-calibration-schedule.v1':
        raise ValueError('calibration schema')
    if schedule['manifest_sha256'] != digest(manifest):
        raise ValueError('constructed manifest identity')
    entries={e['id']:e for e in manifest['entries']}
    if len(entries) != len(manifest['entries']): raise ValueError('duplicate negative identity')
    if len({i['id'] for i in schedule['items']}) != len(schedule['items']):raise ValueError('duplicate scheduled negative')
    expected=set(); scores=[]; bindings={}; unavailable=[]
    for item in schedule['items']:
        if item['id'] not in entries: raise ValueError('unknown negative')
        requests=item['requests']
        if not requests or len({r['root'] for r in requests}) != len(requests): raise ValueError('request denominator')
        roots={r['root'] for r in requests}
        if expected & roots: raise ValueError('duplicate scheduled root')
        expected |= roots
        selected=[r for r in rows if r['root'] in roots]
        scheduled={r['root']:r for r in requests}
        for r in selected:
            request=scheduled[r['root']]
            data=json.loads(request['payload']['messages'][1]['content'][0]['text'])
            clips=data['packet']['clips']
            if (r['model'],r['revision'],r['request_hash']) != (request['model'],request['revision'],data['request_hash']):
                raise ValueError('score/request identity')
            clip=next((c for c in clips if (c['source'],c['view_id'],c['kind'],c['sample_id']) == (r['source'],r['view_id'],r['kind'],r['sample_id'])),None)
            order=0 if clips[0]['source'] < clips[1]['source'] else 1
            if clip is None or r['order'] != order:raise ValueError('sample or order identity')
        models=sorted({(r['model'],r['revision']) for r in requests})
        for model,revision in models:
            actual=[r for r in selected if (r['model'],r['revision']) == (model,revision)]
            providers={r.get('provider') for r in actual}
            if len(providers)!=1 or None in providers:
                unavailable.append(dict(id=item['id'],model=model,reason='missing or mixed actual provider identity'));continue
            p=dict(provider=next(iter(providers)),model=model,revision=revision)
            evaluations={}; pins=set()
            for r in requests:
                if (r['model'],r['revision']) != (model,revision): continue
                data=json.loads(r['payload']['messages'][1]['content'][0]['text']);packet=data['packet']
                if len(packet['clips']) != 2: raise ValueError('calibration requires both candidate slots')
                for c in packet['clips']:
                    key=(c['source'],c['view_id'],c['kind'])
                    evaluation=evaluations.setdefault(key,dict(source=c['source'],view_id=c['view_id'],kind=c['kind'],sample_ids=[],order_count=2))
                    if c['sample_id'] not in evaluation['sample_ids']: evaluation['sample_ids'].append(c['sample_id'])
                    transform=dict(fps=c['fps'],max_edge=c['max_edge'],contact_sheet=c.get('sheet') is not None,
                                   selector='fps-then-uniform-eight/1' if c.get('sheet') else 'fps/1',protocol=packet['schema'],
                                   schema_projection=r['payload']['response_format']['json_schema']['name'],
                                   prompt_sha256=digest(r['payload']['messages'][0]),output_budget=r['payload']['max_tokens'],reasoning_hint=r['payload']['reasoning']['max_tokens'])
                    pins.add(json.dumps(dict(**p,kind=c['kind'],rubric_sha256=digest(packet['rubric']),transform=transform),sort_keys=True))
            if len(pins)!=1: raise ValueError('mixed rubric, transform or kind')
            criteria=[dict(id=c['id'],minimum=c['minimum'],maximum=c['maximum'],threshold=c['minimum']) for c in packet['rubric'].get('criteria',[])]
            config=dict(providers=[p],evaluations=list(evaluations.values()),threshold=1,required_cues=[],criteria=criteria,forbidden=[],
                        scorecard=dict(floor=None,mean=None,conjunction=False))
            # Calibration separates scalar scores; cues/scorecards remain independent gate requirements.
            result=reduce(actual,config)
            groups={g['view_id']:g for g in result['groups']}
            if not result['pass'] or set(groups) != {item['id']+':positive',item['id']+':negative'}:
                unavailable.append(dict(id=item['id'],model=model,reason='incomplete, invalid, replayed or inconsistent orders'));continue
            pin=json.loads(next(iter(pins)))
            pin['sample_count']=len(next(iter(evaluations.values()))['sample_ids']);pin['orders']=2
            scorer='judge:'+digest(pin)
            bindings[scorer]=pin
            scores.append(dict(id=item['id'],scorer=scorer,positive=groups[item['id']+':positive']['median'],negative=groups[item['id']+':negative']['median']))
    if {r['root'] for r in rows} - expected: raise ValueError('unscheduled score evidence')
    # Each arm covers every negative exactly once; repetitions are reduced, never independent sources.
    expected_models={(r['model'],r['revision']) for i in schedule['items'] for r in i['requests']}
    for model,revision in expected_models:
        pins=[s for s,p in bindings.items() if (p['model'],p['revision'])==(model,revision)]
        if len(pins)!=1 or {s['id'] for s in scores if s['scorer'] in pins} != set(entries):
            unavailable.append(dict(model=model,reason='complete manifest denominator unavailable'))
    return dict(scores=scores,bindings=bindings,unavailable=unavailable,trusted_for=[],
                authority='advisory; constructed calibration required; untrusted scores labelled',
                sample_count_rule='exact fresh samples; average both orders then median; repeats do not inflate source counts',
                schedule_sha256=digest(schedule),manifest_sha256=digest(manifest))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest',type=Path,required=True)
    parser.add_argument('--schedule',type=Path,required=True)
    parser.add_argument('--scores',type=Path,required=True)
    parser.add_argument('--positive',type=Path,action='append',required=True)
    parser.add_argument('--bin',type=Path,required=True)
    parser.add_argument('--threshold',type=float,default=.8)
    parser.add_argument('--out',type=Path,required=True)
    args=parser.parse_args()
    manifest=read(args.manifest);schedule=read(args.schedule)
    rows=[decode(line) for line in args.scores.read_bytes().splitlines() if line.strip()]
    result=adapt(manifest,schedule,rows)
    args.out.mkdir()
    (args.out/'adapter.json').write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    if result['unavailable']: raise SystemExit(1)
    external=dict(schema='saccade-motion-scores.v1',manifest_sha256=hashlib.sha256(args.manifest.read_bytes()).hexdigest(),scores=result['scores'])
    score_path=args.out/'external-scores.json';score_path.write_text(json.dumps(external,indent=2)+'\n')
    strengths=sorted({e['strength'] for e in manifest['entries']})
    command=[str(args.bin),'experiment','calibrate-degradations','--strengths',','.join(map(str,strengths)),
             '--seed',str(manifest['seed']),'--threshold',str(args.threshold),'--scores',str(score_path),'--out',str(args.out/'calibrated'),'--json']
    for p in args.positive: command+=['--positive',str(p)]
    subprocess.run(command,check=True,capture_output=True)
    report=read(args.out/'calibrated/calibration.json')
    if report['manifest_sha256'] != external['manifest_sha256']: raise ValueError('calibration manifest changed')
    result['calibration_sha256']=hashlib.sha256((args.out/'calibrated/calibration.json').read_bytes()).hexdigest()
    result['external_scores_sha256']=hashlib.sha256(score_path.read_bytes()).hexdigest()
    result['trust']=[dict(binding=pin,scorer=scorer,trusted_for=next(s['trusted_for'] for s in report['scorers'] if s['scorer']=='external:'+scorer),
                          untrusted_elsewhere=True) for scorer,pin in result['bindings'].items()]
    (args.out/'trust.json').write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')


if __name__=='__main__':main()
