#!/usr/bin/env python3
"""Mandatory offline oracle-perfect/wrong scorer gate; development data only."""
import argparse
import copy
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
from collections import defaultdict


def box(rect, dimensions):
    # Outward enclosure is an answer choice, not scorer padding/tolerance.
    x,y,w,h=rect; width,height=dimensions
    left=max(0,x-1);top=max(0,y-1)
    right=min(width,x+w+1);bottom=min(height,y+h+1)
    return [left/width,top/height,(right-left)/width,(bottom-top)/height]


def perfect(case, truth, directory, order, child=False):
    from dev_policy import exclusion_rect, payload
    request_hash=json.loads(payload(case,order,child,directory)['messages'][1]['content'][0]['text'])['request_hash']
    if not case['complete']:
        return dict(request_hash=request_hash,outcome='unverifiable',observations=[])
    c=dict(case,after=case['counterfactual']['path']) if child else case
    witness=truth['counterfactual_witness'] if child else truth['rendered_witness']
    slot='P1' if order in ('single','ba') else 'P2'  # always after/single
    def observation(statement, coords, refs):
        return dict(slot=slot,kind=statement.split(':')[0],statement=statement,
            geometry=dict(type='box',pixels=coords),visibility='visible',evidence_refs=refs,uncertainty=0)
    if (case.get('condition') or {}).get('kind')=='non_overlap':
        observations=[observation('overlap:separate',[0,0,1,1],[slot+':R0',slot+':R1'])]
    elif case['task']=='check_ui':
        statement='text:'+case['label'] if witness['label_complete'] else (
            'clipping:clipped' if witness['label_pixels'] else 'presence:absent')
        observations=[observation(statement,[0,0,1,1],[slot+':R0'])]
    else:
        def appearance(rect, ref):
            coords=box(rect,c['dimensions'])
            # Independent oracle-bound raster comparison: never ask the scorer
            # which statement it would accept when constructing expected answers.
            from PIL import Image
            import math
            width,height=c['dimensions']
            x,y,w,h=[v*c['dimensions'][i%2] for i,v in enumerate(coords)]
            bounds=(math.floor(x),math.floor(y),math.ceil(x+w),math.ceil(y+h))
            with Image.open(directory/c['before']) as before, Image.open(directory/c['after']) as after:
                changed=before.convert('RGB').crop(bounds).tobytes()!=after.convert('RGB').crop(bounds).tobytes()
            o=observation('appearance:changed' if changed else 'appearance:unchanged',coords,[ref])
            return o
        observations=[appearance(case['target'],slot+':R0')]
        for index,exclusion in enumerate(case['exclusions'],1):
            observations.append(appearance(exclusion_rect(exclusion),f'{slot}:R{index}'))
    return dict(request_hash=request_hash,outcome='observed' if child else truth['expected_outcome'],observations=observations)


def validate_closed(answer, case, order, child, directory):
    """Offline checks of the live transport's closed shape and citation contract."""
    from dev_policy import payload
    data=json.loads(payload(case,order,child,directory)['messages'][1]['content'][0]['text'])
    if set(answer)!={'request_hash','outcome','observations'} or answer['request_hash']!=data['request_hash']:
        raise ValueError('synthetic request-bound closed answer')
    if answer['outcome'] not in ('observed','not_observed','unverifiable'):raise ValueError('outcome')
    views={v['slot']:{r['id'] for r in v['regions']} for v in data['views']}
    for o in answer['observations']:
        if set(o)!={'slot','kind','statement','geometry','visibility','evidence_refs','uncertainty'}:raise ValueError('closed observation')
        if o['slot'] not in views or not o['evidence_refs'] or not set(o['evidence_refs'])<=views[o['slot']]:raise ValueError('citation identity')
        if o['kind']!=o['statement'].split(':',1)[0]:raise ValueError('kind statement mismatch')
        permitted={'presence':('presence:present','presence:absent'),'clipping':('clipping:clipped','clipping:contained'),
            'overlap':('overlap:overlap','overlap:separate'),'appearance':('appearance:changed','appearance:unchanged')}
        if o['kind']=='text':
            if not o['statement'][5:]:raise ValueError('empty text')
        elif o['statement'] not in permitted.get(o['kind'],()):raise ValueError('unsupported atomic statement')
        if o['visibility'] not in ('visible','partial','occluded','unavailable') or not 0<=o['uncertainty']<=1:raise ValueError('observation bounds')
        g=o['geometry'];v=g['pixels']
        if set(g)!={'type','pixels'} or g['type']!='box' or len(v)!=4 or not all(0<=n<=1 for n in v) or v[2]<=0 or v[3]<=0 or v[0]+v[2]>1 or v[1]+v[3]>1:raise ValueError('normalized geometry')


def proof(manifest, oracle, directory):
    from stage2 import ARMS
    from dev_policy import normalize
    from pilot_score import semantic, summarize
    groups=defaultdict(list); legacy=defaultdict(list)
    variants=('perfect','missing','hallucinated','wrong_text','wrong_geometry','wrong_order')
    for case in manifest['cases']:
        if case['split']!='development': raise ValueError('selftest accepts development roots only')
        truth=oracle[case['root_id']]
        for arm in ARMS:
            # Every arm exercises the real semantic scorer; rules/oracle arms here
            # are injected synthetic evidence, not measured routing/model behavior.
            orders=['single'] if case['task']=='check_ui' else ['ab','ba']
            answers={}
            for child in (False,True) if case.get('counterfactual') else (False,):
                answers[child]=[perfect(case,truth,directory,o,child) for o in orders]
            for child, order_answers in answers.items():
                for order, answer in zip(orders,order_answers):validate_closed(answer,case,order,child,directory)
            for variant in variants:
                values=copy.deepcopy(answers)
                if variant=='missing': values={False:[None]}
                else:
                    for child, values_order in values.items():
                        for a in values_order:
                            if variant!='perfect':
                                a['outcome']='observed'
                                if not a['observations']:
                                    a['observations']=[dict(slot='P1',kind='text',statement='text:invented',geometry=dict(type='box',pixels=[0,0,1,1]),visibility='visible',evidence_refs=['P1:R0'],uncertainty=0)]
                            if variant=='hallucinated':
                                a['observations'].append(dict(a['observations'][0],kind='text',statement='text:unsupported synthetic hallucination'))
                            elif variant=='wrong_text':
                                for obs in a['observations']: obs.update(kind='text',statement='text:deliberately wrong literal')
                            elif variant=='wrong_geometry':
                                for obs in a['observations']:
                                    obs.update(kind='presence',statement='presence:present',geometry=dict(type='box',pixels=[0,0,1/case['dimensions'][0],1/case['dimensions'][1]]))
                    if variant=='wrong_order':
                        # Exercise even single-view arms with a deliberately conflicting
                        # extra replica; no extra independent root is manufactured.
                        second=copy.deepcopy(values[False][0]);second['outcome']='not_observed'
                        values[False]=[values[False][0],second]
                normalized={child:[normalize(a,case,orders[min(i,len(orders)-1)]) if a is not None else None for i,a in enumerate(v)] for child,v in values.items()}
                if variant=='wrong_order':
                    # Intentional outcome disagreement must withhold even empty observations.
                    normalized[False][1]['outcome']='not_observed'
                    normalized[False][0]['outcome']='observed'
                result=semantic(normalized,case,truth,directory,task_evidence_policy=True)
                result.update(important=truth['important'],root=case['root_id'])
                groups[(case['workload'],arm,variant)].append(result)
                if variant=='perfect':
                    from pilot_score import normalize as old_normalize
                    old={child:[old_normalize(a,case,orders[i]) for i,a in enumerate(v)] for child,v in answers.items()}
                    old_result=semantic(old,case,truth,directory)
                    old_result.update(important=truth['important'],root=case['root_id'])
                    legacy[(case['workload'],arm)].append(old_result)
    rows=[]; failed=[]
    for (workload,arm,variant),items in sorted(groups.items()):
        m=summarize(items,[])
        flagged=sum(not i['correct'] and not (variant=='perfect' and i['abstention']) for i in items)
        passed=(m['precision']==1 and m['important_change_recall']==1 and m['false_reassurance']==0 and m['available_roots']==m['roots']) if variant=='perfect' else (m['correct_roots']==0 and flagged==len(items))
        if not passed: failed.append((workload,arm,variant))
        rows.append(dict(workload=workload,arm=arm,variant=variant,roots=m['roots'],precision=m['precision'],
            important_recall=m['important_change_recall'],false_reassurance=m['false_reassurance'],
            correct_roots=m['correct_roots'],flagged_roots=flagged,available_roots=m['available_roots'],passed=passed))
    from judge_gate import selftest as judge_selftest
    judge_proof=judge_selftest()
    if judge_proof['status'] != 'PASS': failed.append(('judge','offline','gate'))
    from image_quality import selftest as quality_selftest
    quality_proof=quality_selftest()
    if quality_proof['status'] != 'PASS': failed.append(('image-quality','offline','gate'))
    return dict(judge_gate=judge_proof,image_quality=quality_proof,schema='saccade-scorer-selftest.v1',status='FAIL' if failed else 'PASS',
        development_roots=len(manifest['cases']),provider_calls=0,heldout_entries_decoded=0,
        evidence='Synthetic oracle-injected closed-protocol answers through dev_policy.normalize and pilot_score.semantic (score.assertion_correct/task_evidence). All arms are injections, not provider/routing performance.',
        unavailable='Unavailable roots correctly abstain, excluded from precision; important recall keeps all challenge roots. Negative cases must be wrong or unavailable, never accepted.',
        rows=rows,failures=failed,legacy_perfect=[dict(workload=w,arm=a,**{k:summarize(items,[])[k] for k in ('precision','important_change_recall','false_reassurance')}) for (w,a),items in sorted(legacy.items())])


def markdown(result):
    lines=['# Oracle-perfect / oracle-wrong offline proof','',result['status'], '',result['evidence'],'',result['unavailable'],'',
        '| Workload | Arm | Synthetic case | Precision | Important recall | False reassurance | Flagged/roots | Pass |',
        '|---|---|---|---:|---:|---:|---:|---|']
    pct=lambda v:'withheld' if v is None else f'{100*v:.0f}%'
    for r in result['rows']:
        lines.append(f"| {r['workload']} | {r['arm']} | {r['variant']} | {pct(r['precision'])} | {pct(r['important_recall'])} | {r['false_reassurance']} | {r['flagged_roots']}/{r['roots']} | {r['passed']} |")
    lines += ['', '## Legacy oracle-perfect diagnostic', '', '| Workload | Arm | Precision | Important recall | False reassurance |', '|---|---|---:|---:|---:|']
    for r in result['legacy_perfect']:
        lines.append(f"| {r['workload']} | {r['arm']} | {pct(r['precision'])} | {pct(r['important_change_recall'])} | {r['false_reassurance']} |")
    lines += ['', 'Judge parity gate: '+result['judge_gate']['status'], 'Image quality gate: '+result['image_quality']['status'], '']
    lines += ['', 'Legacy region-only citation scorer cannot establish audit-mask success even with perfect answers; epoch-3 explicit region/exclusion mapping repairs that protocol mismatch. No tolerance or statement matching changes.','']
    return '\n'.join(lines)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--corpus',type=Path,required=True);p.add_argument('--source-revision',required=True)
    p.add_argument('--out',type=Path);p.add_argument('--worker',action='store_true',help=argparse.SUPPRESS)
    args=p.parse_args()
    if args.worker:
        import corpus
        import dev_audit
        dev_audit._frozen_verify=corpus.verify
        manifest,oracle=dev_audit.scoped_verify(args.corpus)
        result=proof(manifest,oracle,args.corpus)
        result['manifest_hash']=manifest['manifest_hash'];result['oracle_hash']=manifest['oracle_hash']
        result['source_revision']=args.source_revision
        current=Path(__file__).parent
        result['scoring_source_sha256']={name:corpus.digest((current/name).read_bytes()) for name in ('scorer_selftest.py','dev_policy.py','pilot_score.py','score.py','dev_audit.py','judge_gate.py','video_scores.py','image_quality.py')}
        if args.out:
            corpus.put(args.out.with_suffix('.json'),result)
            args.out.with_suffix('.md').write_text(markdown(result))
        print(json.dumps(dict(status=result['status'],development_roots=result['development_roots'],proof_rows=len(result['rows']),failures=result['failures'])))
        if result['failures']: raise SystemExit(1)
        return
    manifest=json.loads((args.corpus/'manifest.json').read_bytes())
    if manifest['epoch']=='g12-fresh-heldout/1': raise ValueError('fresh heldout is off limits')
    root=Path(__file__).resolve().parents[2]
    revision=subprocess.check_output(['git','rev-parse','--verify',args.source_revision+'^{commit}'],cwd=root,text=True).strip()
    archive=subprocess.check_output(['git','archive',revision],cwd=root)
    with tempfile.TemporaryDirectory(prefix='saccade-scorer-selftest-') as temp:
        frozen=Path(temp)
        with tarfile.open(fileobj=io.BytesIO(archive)) as tar:tar.extractall(frozen,filter='data')
        # Only current scorer fixes/adapters execute; the pinned renderer/verifier
        # verifies original development truth without changing frozen source bytes.
        program='import sys,importlib.util;sys.path.insert(0,'+repr(str(frozen/'scripts/assist'))+');sys.path.append('+repr(str(root/'scripts/assist'))+');'
        for name in ('video_scores','judge_gate','image_quality'):
            program+='spec=importlib.util.spec_from_file_location('+repr(name)+','+repr(str(root/'scripts/assist'/f'{name}.py'))+');m=importlib.util.module_from_spec(spec);sys.modules['+repr(name)+']=m;spec.loader.exec_module(m);'
        program+='spec=importlib.util.spec_from_file_location("score",'+repr(str(root/'scripts/assist/score.py'))+');m=importlib.util.module_from_spec(spec);sys.modules["score"]=m;spec.loader.exec_module(m);'
        program+='spec=importlib.util.spec_from_file_location("pilot_score",'+repr(str(root/'scripts/assist/pilot_score.py'))+');m=importlib.util.module_from_spec(spec);sys.modules["pilot_score"]=m;spec.loader.exec_module(m);spec=importlib.util.spec_from_file_location("scorer_selftest",'+repr(str(root/'scripts/assist/scorer_selftest.py'))+');m=importlib.util.module_from_spec(spec);sys.modules["scorer_selftest"]=m;spec.loader.exec_module(m);m.main()'
        cmd=[sys.executable,'-c',program,'--worker','--corpus',str(args.corpus.resolve()),'--source-revision',revision]
        if args.out:
            if args.out.with_suffix('.json').exists() or args.out.with_suffix('.md').exists():raise ValueError('refuse to overwrite proof')
            args.out.parent.mkdir(parents=True,exist_ok=True);cmd+=['--out',str(args.out.resolve())]
        subprocess.run(cmd,cwd=frozen,check=True)


if __name__=='__main__':main()
