#!/usr/bin/env python3
"""Complete-denominator advisory judge reduction; offline, never baseline approval."""
import argparse
import copy
import json
import math
from pathlib import Path
from statistics import median
from video_scores import read


def number(value):
    return type(value) in (int,float) and math.isfinite(value)


def identity(value):
    return tuple(value[k] for k in ('provider','model','revision'))


def validate(config):
    if set(config) != {'providers','evaluations','threshold','required_cues','criteria','forbidden','scorecard'}:
        raise ValueError('closed judge gate configuration')
    if not config['providers'] or not config['evaluations'] or not number(config['threshold']) or not 1 <= config['threshold'] <= 10:
        raise ValueError('judge gate denominator or threshold')
    providers=[identity(p) for p in config['providers']]
    if len(set(providers)) != len(providers) or any(not all(type(v) is str and v for v in p) for p in providers):
        raise ValueError('provider identity must be unique and explicit')
    keys=[]
    for e in config['evaluations']:
        if set(e) != {'source','view_id','kind','sample_ids','order_count'} or e['kind'] not in ('still','motion') or e['order_count'] not in (1,2):
            raise ValueError('evaluation contract')
        samples=e['sample_ids']
        if not samples or any(type(s) is not str or not s for s in samples) or len(set(samples)) != len(samples):
            raise ValueError('declared sample count must be positive and unique')
        keys.append((e['source'],e['view_id'],e['kind']))
    if len(set(keys)) != len(keys): raise ValueError('duplicate evaluation')
    for names in (config['required_cues'],config['forbidden']):
        if len(set(names)) != len(names) or any(type(n) is not str or not n for n in names): raise ValueError('closed vocabulary')
    ids=[]
    for c in config['criteria']:
        if set(c) != {'id','minimum','maximum','threshold'} or not all(number(c[k]) for k in ('minimum','maximum','threshold')) or not c['minimum'] <= c['threshold'] <= c['maximum'] or c['minimum'] >= c['maximum']:
            raise ValueError('criterion gate scale')
        ids.append(c['id'])
    if len(set(ids)) != len(ids): raise ValueError('duplicate criterion')
    policy=config['scorecard']
    if set(policy) != {'floor','mean','conjunction'} or type(policy['conjunction']) is not bool:
        raise ValueError('scorecard policy')
    for k in ('floor','mean'):
        if policy[k] is not None and not number(policy[k]): raise ValueError('scorecard threshold')
    if (policy['floor'] is not None or policy['mean'] is not None) and (not config['criteria'] or len({(c['minimum'],c['maximum']) for c in config['criteria']}) != 1):
        raise ValueError('floor/mean require a common scale; no implicit conversion')


def reduce(rows, config):
    validate(config)
    groups=[]
    expected=set()
    for e in config['evaluations']:
        for p in config['providers']:
            relevant=[r for r in rows if (r.get('source'),r.get('view_id'),r.get('kind')) == (e['source'],e['view_id'],e['kind']) and identity(r) == identity(p)]
            samples=[]
            for sample_id in e['sample_ids']:
                selected=[r for r in relevant if r.get('sample_id') == sample_id]
                for r in selected: expected.add(id(r))
                complete=(len(selected) == e['order_count'] and len({r.get('order') for r in selected}) == e['order_count']
                          and {r.get('order') for r in selected} == set(range(e['order_count'])))
                valid=complete and all(r.get('outcome') == 'scored' and number(r.get('score')) and 1 <= r['score'] <= 10
                    and r.get('returned_model') == p['model'] and r.get('returned_revision') == p['revision']
                    and r.get('replay') is False for r in selected)
                preferences={r.get('preferred_source') for r in selected}
                status='incomplete' if not valid else 'inconsistent' if len(preferences) != 1 else 'consistent'
                if valid and (None in preferences or 'abstain' in preferences): status='incomplete'
                cue_ok=True; forbidden_ok=True; criterion_values=[]
                for r in selected:
                    cues=r.get('cues',[])
                    cue_ok &= len({c['cue'] for c in cues}) == len(cues) and all(
                        any(c['cue'] == name and c['state'] == 'present' and c.get('timestamps_s') for c in cues) for name in config['required_cues'])
                    conditions=r.get('forbidden',[])
                    forbidden_ok &= len({c['condition'] for c in conditions}) == len(conditions) and all(
                        any(c['condition'] == name and c['state'] == 'absent' for c in conditions) for name in config['forbidden'])
                    criteria=r.get('criteria',[])
                    if len(criteria) != len(config['criteria']) or [c.get('id') for c in criteria] != [c['id'] for c in config['criteria']]:
                        valid=False
                    else:
                        for c, scale in zip(criteria,config['criteria']):
                            if not number(c.get('score')) or not scale['minimum'] <= c['score'] <= scale['maximum']: valid=False
                        criterion_values.append([c.get('score') for c in criteria])
                if not valid: status='incomplete'
                values=[sum(v[i] for v in criterion_values)/len(criterion_values) for i in range(len(config['criteria']))] if valid else []
                # Apply floor and mean to every order, so an averaging rescue cannot hide a forbidden/floor failure.
                policy=config['scorecard']
                card_ok=valid and all((policy['floor'] is None or min(v) >= policy['floor']) and
                    (policy['mean'] is None or sum(v)/len(v) >= policy['mean']) and
                    (not policy['conjunction'] or all(n >= c['threshold'] for n,c in zip(v,config['criteria']))) for v in criterion_values)
                samples.append(dict(sample_id=sample_id,status=status,winner=next(iter(preferences)) if status=='consistent' else None,
                    score=sum(r['score'] for r in selected)/len(selected) if status=='consistent' else None,
                    criteria=values,cues_present=bool(cue_ok and complete),forbidden_absent=bool(forbidden_ok and complete),scorecard_pass=bool(card_ok)))
            available=all(s['status']=='consistent' for s in samples)
            value=median(s['score'] for s in samples) if available else None
            passed=available and value >= config['threshold'] and all(s['cues_present'] and s['forbidden_absent'] and s['scorecard_pass'] for s in samples)
            groups.append(dict(**e,**p,median=value,samples=samples,**{'pass':bool(passed)}))
    # Unscheduled or extra samples cannot silently disappear from the denominator.
    extra=len(expected) != len(rows)
    return dict(schema='saccade-judge-gate.v1',authority='advisory; uncalibrated, no baseline approval',trusted_for=[],
                sample_count_rule='exact declared fresh sample_ids per provider/view/kind; both orders averaged once per sample, then median; even median averages the middle two; no replay',
                extra_evidence=extra,groups=groups,**{'pass':not extra and all(g['pass'] for g in groups)})


def selftest():
    config=dict(providers=[dict(provider='fixture',model='model',revision='revision')],evaluations=[dict(source='source',view_id='view',kind='still',sample_ids=['sample-0'],order_count=2)],
        threshold=7,required_cues=['cue'],criteria=[dict(id='criterion',minimum=0,maximum=3,threshold=2)],forbidden=['condition'],scorecard=dict(floor=2,mean=2.3,conjunction=True))
    rows=[dict(provider='fixture',model='model',revision='revision',returned_model='model',returned_revision='revision',source='source',view_id='view',kind='still',sample_id='sample-0',order=i,
        outcome='scored',score=8,replay=False,preferred_source='source',cues=[dict(cue='cue',state='present',timestamps_s=[0])],criteria=[dict(id='criterion',score=3)],forbidden=[dict(condition='condition',state='absent')]) for i in range(2)]
    cases={'oracle-perfect':reduce(rows,config)['pass']}
    for name, field, value in [('null','score',None),('wrong-score','score',1),('flip','preferred_source','other'),('unknown-cue','cues',[dict(cue='cue',state='unknown',timestamps_s=[])]),('forbidden','forbidden',[dict(condition='condition',state='present')]),('floor','criteria',[dict(id='criterion',score=0)]),('replay','replay',True),('revision','returned_revision','other')]:
        wrong=copy.deepcopy(rows);wrong[0][field]=value
        cases['oracle-wrong-'+name]=not reduce(wrong,config)['pass']
    cases['oracle-wrong-missing']=not reduce(rows[:1],config)['pass']
    cases['oracle-wrong-extra']=not reduce(rows+rows[:1],config)['pass']
    return dict(status='PASS' if all(cases.values()) else 'FAIL',provider_calls=0,cases=cases)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scores',type=Path)
    parser.add_argument('--config',type=Path)
    parser.add_argument('--out',type=Path)
    parser.add_argument('--markdown',type=Path)
    parser.add_argument('--strict',action='store_true')
    parser.add_argument('--self-test',action='store_true')
    args=parser.parse_args()
    if args.self_test:
        result=selftest();print(json.dumps(result));return 0 if result['status']=='PASS' else 1
    if not all((args.scores,args.config,args.out)): parser.error('--scores, --config and --out required')
    # Each line uses the same duplicate-key/nonfinite parser as artifacts.
    from video_scores import decode
    rows=[decode(line) for line in args.scores.read_bytes().splitlines() if line.strip()]
    report=reduce(rows,read(args.config))
    with args.out.open('x') as output: json.dump(report,output,indent=2,allow_nan=False);output.write('\n')
    if args.markdown:
        with args.markdown.open('x') as output:
            output.write('# Advisory judge gate\n\n'+('PASS' if report['pass'] else 'FAIL')+'\n\n'+report['sample_count_rule']+'\n\n')
            for group in report['groups']: output.write(f"- {group['view_id']} / {group['kind']} / {group['provider']} / {group['model']} / {group['revision']}: median {group['median']}, pass {group['pass']}\n")
    return 1 if args.strict and not report['pass'] else 0

if __name__=='__main__':raise SystemExit(main())
