#!/usr/bin/env python3
"""Development-only offline audit. Never dispatches, reads keys or decodes held-out truths."""
import argparse
from collections import Counter, defaultdict
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile


def value_end(raw, start):
    """Skip an encoded value lexically; unselected oracle values are never decoded."""
    stack = []; quoted = False; escaped = False
    for i in range(start, len(raw)):
        c = raw[i]
        if quoted:
            if escaped: escaped = False
            elif c == 92: escaped = True
            elif c == 34: quoted = False
        elif c == 34: quoted = True
        elif c in (123, 91): stack.append(c)
        elif c in (125, 93):
            if not stack: return i
            expected = 123 if c == 125 else 91
            if stack.pop() != expected: raise ValueError('malformed oracle nesting')
            if not stack: return i + 1
        elif not stack and c in (44, 10): return i
    return len(raw)


def object_members(raw, start=0):
    if raw[start] != 123: raise ValueError('oracle object required')
    i = start + 1
    while True:
        while raw[i] in b' \n\r\t,': i += 1
        if raw[i] == 125: return
        if raw[i] != 34: raise ValueError('oracle key required')
        end = i + 1; escaped = False
        while end < len(raw):
            if raw[end] == 34 and not escaped: break
            if raw[end] == 92 and not escaped: escaped = True
            else: escaped = False
            end += 1
        key = json.loads(raw[i:end+1]); i = end+1
        while raw[i] in b' \n\r\t': i += 1
        if raw[i] != 58: raise ValueError('oracle colon required')
        i += 1
        while raw[i] in b' \n\r\t': i += 1
        end = value_end(raw, i)
        yield key, i, end
        i = end


def development_oracle(path, manifest):
    from corpus import digest
    raw = path.read_bytes()
    if digest(raw.rstrip(b'\n')) != manifest['oracle_hash']:
        raise ValueError('oracle hash drift')
    wanted = {c['root_id'] for c in manifest['cases'] if c['split'] == 'development'}
    document = {}; seen = set()
    for key, start, end in object_members(raw):
        if key in document: raise ValueError('duplicate oracle field')
        if key == 'cases':
            cases = {}
            for root, a, b in object_members(raw, start):
                if root in seen: raise ValueError('duplicate oracle root')
                seen.add(root)
                cases[root] = json.loads(raw[a:b]) if root in wanted else None
            document[key] = cases
        elif key == 'schema': document[key] = json.loads(raw[start:end])
        else: raise ValueError('unknown oracle field')
    if wanted - seen: raise ValueError('missing development oracle')
    return document


def scoped_verify(directory):
    """Run the pinned verifier with only development oracle values and images.

    Hash opaque oracle bytes, preserve global topology checks, and skip other splits
    before any image/truth access. Compile in memory: frozen source bytes stay intact.
    """
    import corpus
    import inspect
    source = inspect.getsource(_frozen_verify)
    source = source.replace('oracle_document=json.loads((directory/"oracle.json").read_bytes())',
        'oracle_document=development_oracle(directory/"oracle.json",manifest)')
    source = source.replace(' or digest(encoded(oracle_document))!=manifest["oracle_hash"]', '')
    source = source.replace('for case in manifest["cases"]:',
        'for case in manifest["cases"]:\n        if case["split"] != "development": continue')
    source = source.replace('return manifest,oracle',
        'manifest["cases"]=[c for c in manifest["cases"] if c["split"]=="development"]\n'
        '    return manifest,{k:v for k,v in oracle.items() if v is not None}')
    namespace = dict(corpus.__dict__, development_oracle=development_oracle)
    exec(compile(source, corpus.__file__, 'exec'), namespace)
    return namespace['verify'](directory)


def covers(observation, rect):
    g = observation['geometry']
    if g['type'] != 'box': return False
    x,y,w,h = g['pixels']; a,b,c,d = rect
    return x<=a and y<=b and x+w>=a+c and y+h>=b+d


def order_differences(answers):
    changes = set()
    for orders in answers.values():
        if len(orders)<2 or any(a is None for a in orders): continue
        a,b=orders[:2]
        if a['outcome']!=b['outcome']: changes.add('outcome')
        if len(a['observations'])!=len(b['observations']): changes.add('observation_count')
        # Compare multisets for each field: independent of observation sorting.
        for field in ('image_role','statement','geometry','evidence_refs','uncertainty','visibility','kind'):
            project=lambda v: sorted(json.dumps(o.get(field),sort_keys=True) for o in v['observations'])
            if project(a)!=project(b): changes.add(field)
    return sorted(changes)


def worker(args):
    # Imports resolve against the archived pinned checkout, never current corpus code.
    import corpus
    import pilot_score
    from score import assertion_correct, task_evidence
    from stage2 import payload, schedule
    from dev_policy import PROMPT_EPOCH, PROMPT_POLICY, SCORER_POLICY
    global _frozen_verify
    _frozen_verify = corpus.verify
    corpus.verify = scoped_verify
    pilot_score.verify = scoped_verify
    manifest, oracle = scoped_verify(args.corpus)
    if manifest['epoch']=='g12-fresh-heldout/1': raise ValueError('fresh heldout prohibited')
    from scorer_selftest import proof, markdown as proof_markdown
    proof_result=proof(manifest,oracle,args.corpus)
    proof_result.update(manifest_hash=manifest['manifest_hash'],oracle_hash=manifest['oracle_hash'],source_revision=args.source_revision,scoring_source_sha256={name:corpus.digest((Path(__file__).parent/name).read_bytes()) for name in ('dev_audit.py','dev_policy.py','scorer_selftest.py','pilot_score.py','score.py')})
    corpus.put(args.out/'oracle-proof.json',proof_result)
    (args.out/'oracle-proof.md').write_text(proof_markdown(proof_result))
    if proof_result['status']!='PASS': raise ValueError('mandatory oracle-perfect/wrong proof failed; no recommendation')
    result = pilot_score.evaluate(args.corpus,args.requests,args.results,args.local_results,split='development')
    rows=json.loads(args.requests.read_bytes())
    outcomes=json.loads((args.results/'smoke.json').read_bytes())['root_outcomes']
    cases={c['root_id']:c for c in manifest['cases']}
    records=[]; grouped=defaultdict(lambda:defaultdict(list)); fixed_grouped=defaultdict(lambda:defaultdict(list)); counts=Counter(); reasons=Counter()
    for i,(row,outcome) in enumerate(zip(rows,outcomes)):
        root,arm,variant,order=row['root'].rsplit(':',3)
        if root not in cases: raise ValueError('non-development request prohibited')
        case=cases[root]; truth=oracle[root]; child=variant=='counter'
        answer=None
        if outcome['code']=='completed':
            answer=pilot_score.normalize(json.loads((args.results/f'answer-{i}.json').read_bytes()),case,order)
        grouped[(root,arm)][child].append(answer)
        from dev_policy import normalize as fixed_normalize
        fixed_answer=fixed_normalize(json.loads((args.results/f'answer-{i}.json').read_bytes()),case,order) if answer is not None else None
        fixed_grouped[(root,arm)][child].append(fixed_answer)
        if answer is None: continue
        c=dict(case,after=case['counterfactual']['path']) if child else case
        t=dict(truth,rendered_witness=truth['counterfactual_witness'],expected_outcome='observed') if child else truth
        assertions=[assertion_correct(o,t,c,args.corpus) for o in answer['observations']]
        task=task_evidence(answer['observations'],c,t,args.corpus)
        if answer['outcome']=='unverifiable' or (all(assertions) and task and answer['outcome']==t['expected_outcome']): continue
        tags=[]; detail=[]
        from dev_policy import assertion_correct as fixed_assertion
        overlap_mapping=(case.get('condition') or {}).get('kind')=='non_overlap' and any(o['statement'].startswith('overlap:') and fixed_assertion(o,t,c,args.corpus) for o in answer['observations'])
        if overlap_mapping:
            tags.append('scorer_artefact');detail.append('non_overlap_condition_lacks_independent_overlap_oracle_mapping')
        if case['task']=='audit_mask':
            tags.append('task_definition');detail.append('exclusion_ID_not_expressible_in_region_only_protocol')
            if not any(covers(o,__import__('dev_policy').exclusion_rect(e)) for o in answer['observations'] for e in case['exclusions']):
                detail.append('exclusion_coverage_missing')
        if case['task']=='explain' and not any(o['statement'] in ('appearance:changed','appearance:unchanged') for o in answer['observations']):
            tags.append('task_definition');detail.append('pairwise_task_evidence_missing_from_independent_view_answer')
        if not task and case['task']=='explain' and any(ok and o['statement'].startswith('appearance:') for o,ok in zip(answer['observations'],assertions)):
            detail.append('exact_target_coverage_missing');tags.append('model_error')
        for obs,ok in zip(answer['observations'],assertions):
            if ok: continue
            if obs['statement']=='appearance:unchanged' and obs['image_role']=='before' and case['task']!='check_ui':
                tags.append('task_definition');detail.append('independent_view_vs_pairwise_statement_ambiguity')
            elif overlap_mapping and obs['statement'].startswith('overlap:') and fixed_assertion(obs,t,c,args.corpus):
                pass
            else:
                tags.append('model_error')
                detail.append('unsupported_wrong_or_insufficient_assertion:'+obs['statement'])
        if answer['outcome']!=t['expected_outcome']:
            tags.append('model_error');detail.append('wrong_task_outcome')
        if not task and not tags: tags.append('model_error');detail.append('missing_task_evidence')
        tags=sorted(set(tags));detail=sorted(set(detail))
        primary='task_definition' if 'task_definition' in tags else 'scorer_artefact' if 'scorer_artefact' in tags else 'model_error'
        counts[primary]+=1;reasons.update(detail)
        records.append(dict(index=i,root=root,arm=arm,variant=variant,order=order,workload=c['workload'],
            primary_class=primary,classes=tags,reasons=detail,expected_outcome=t['expected_outcome'],
            response=answer,assertions_correct=assertions,task_evidence=task,target=c['target'],label=c['label']))
    orders=[]
    for (root,arm),answers in sorted(grouped.items()):
        diff=order_differences(answers)
        if diff: orders.append(dict(root=root,arm=arm,workload=cases[root]['workload'],causes=diff))
    baseline=__import__('copy').deepcopy(result)
    fixed_items=[]
    for item in result['root_results']:
        if (item['root'],item['arm']) not in fixed_grouped:
            fixed_items.append(item);continue
        c=cases[item['root']]
        fixed=pilot_score.semantic(fixed_grouped[(item['root'],item['arm'])],c,oracle[item['root']],args.corpus,True)
        fixed.update({k:item[k] for k in ('root','arm','split','workload','important','family')})
        fixed_items.append(fixed)
    result['root_results']=fixed_items
    for w,arms in result['splits']['development'].items():
        for a,m in arms.items():
            quality=pilot_score.summarize([i for i in fixed_items if i['workload']==w and i['arm']==a],[])
            for k in ('available_roots','availability','committed_roots','correct_roots','precision','assertions','correct_assertions','assertion_precision','important_roots','detected_important_roots','important_change_recall','false_reassurance','false_reassurance_rate','abstentions','abstention_rate'):
                m[k]=quality[k]
            for k in ('availability','precision','important_change_recall','false_reassurance','abstention'):
                m['one_sided_95'][k]=quality['one_sided_95'][k]
    overall=pilot_score.summarize(fixed_items,[])
    for k in ('correct_roots','precision','correct_assertions','assertion_precision','detected_important_roots','important_change_recall','false_reassurance','false_reassurance_rate'):
        result['campaign'][k]=overall[k]
    for k in ('precision','important_change_recall','false_reassurance'):result['campaign']['one_sided_95'][k]=overall['one_sided_95'][k]
    result['task_evidence_policy']='epoch-3 mapping replay on epoch-2 answers; missing evidence stays missing'
    result['scoring_source_sha256'].update({n:corpus.digest((Path(__file__).parent/n).read_bytes()) for n in ('dev_policy.py','dev_audit.py','scorer_selftest.py')})
    result['limitations']=[s for s in result['limitations'] if not s.startswith('This heldout corpus')]
    result['limitations'] += ['Only development oracle entries and pixels decoded; other split denominators are zero, not evaluated.',
        'Epoch-3 prompt changes cannot be retroactively evaluated on epoch-2 answers. Strict epoch-2 metrics remain the development baseline.']
    result.update(frozen_source_commit=args.source_revision,development_only=True)
    from dev_policy import payload as new_payload
    from stage2 import reservation
    reservations=[reservation(new_payload(cases[r['root'].rsplit(':',3)[0]],r['root'].rsplit(':',3)[3],r['root'].rsplit(':',3)[2]=='counter',args.corpus), cases[r['root'].rsplit(':',3)[0]]['dimensions']) for r in rows]
    from dev_policy import adapt_plan
    old_plan=json.loads((args.requests.parent/'plan.json').read_bytes())
    next_rows,next_plan=adapt_plan(rows,manifest,args.corpus,old_plan)
    corpus.put(args.out/'epoch-3-requests.json',next_rows)
    corpus.put(args.out/'epoch-3-plan.json',next_plan)
    # Reservation shape is the same pre-dispatch upper bound as existing plans.
    new_cost=sum(reservations)
    audit=dict(schema='saccade-g12-development-audit.v1',status='DEVELOPMENT ONLY, UNQUALIFIED',
        heldout_entries_decoded=0,heldout_responses_opened=0,provider_calls=0,key_reads=0,
        units='Disjoint primary counts count disagreeing completed responses; multi-cause tags overlap. Order counts count root/arms separately.',
        reviewed_completed_responses=result['campaign']['completed_answers'],
        disagreement_responses=len(records),classification_counts={k:counts[k] for k in ('model_error','scorer_artefact','task_definition')},
        overlapping_response_counts={k:sum(k in r['classes'] for r in records) for k in ('model_error','scorer_artefact','task_definition')},
        order_disagreement_root_arms=len(orders),order_cause_counts=dict(Counter(c for r in orders for c in r['causes'])),
        reason_counts=dict(reasons),disagreements=records,order_disagreements=orders,
        representative_examples={k:[r for r in records if k in r['classes']][:5] for k in ('model_error','scorer_artefact','task_definition')},
        order_examples=orders[:5],fixes=[dict(kind='task_definition',prompt_epoch=PROMPT_EPOCH,prompt_policy=PROMPT_POLICY,
            scorer_policy=SCORER_POLICY,rationale='Explicit pairwise/task/outcome semantics; same-slot exclusion regions map to required exclusion IDs. Exact literals/order agreement; epoch-5 region IoU/coverage tolerance and one citation per statement.')],
        oracle_mapping_fix_count=2,mandatory_oracle_proof=proof_result,baseline=baseline,recommendation='b: new development run required before fresh held-out qualification',
        new_development_schedule_requests=len(rows),new_development_reservation_nano_usd=new_cost,
        cost_method='Full scheduled epoch-3 pre-dispatch reservations at pinned prices; no calls authorized or made. Expected spend unverified.',
        rescored=result)
    corpus.put(args.out/'audit.json',audit)
    (args.out/'SCORE-development.md').write_text(pilot_score.markdown(result))
    corpus.put(args.out/'SCORE-development.json',result)
    lines=[proof_markdown(proof_result),'','# G12 development disagreement audit','',audit['units'],'',
        f"Reviewed {audit['reviewed_completed_responses']} completed responses; {len(records)} response disagreements.",
        f"Primary classes: {audit['classification_counts']}. Overlapping causes: {audit['overlapping_response_counts']}.",
        f"Order disagreement root/arms: {len(orders)}; causes: {audit['order_cause_counts']}.",'',
        'Root fixes: region/exclusion citation bridge and independent source overlap mapping. Exact geometry, literal text and order agreement remain unchanged.',
        'Epoch 3 fixes task definitions and citation mapping prospectively. Old answers are replayed with source-bound overlap mapping; missing epoch-3 exclusion citations stay missing.','',
        audit['recommendation']+f". Full {len(rows)}-request new development reservation: ${new_cost/1e9:.8f}; actual cost unknown.",'']
    for cls in ('model_error','scorer_artefact','task_definition'):
        lines += [f'## {cls}','']
        examples=audit['representative_examples'][cls]
        if not examples: lines+=['No development examples established; none invented.','']
        for r in examples:
            lines += [f"- Request {r['index']} ({r['workload']}, {r['arm']}, {r['order']}): expected {r['expected_outcome']}, got {r['response']['outcome']}; "+'; '.join(r['reasons'])+f". Assertion truth: {r['assertions_correct']}; task evidence: {r['task_evidence']}."]
    lines += ['', '## Order disagreement examples','']
    lines += [f"- {r['root']} / {r['arm']} ({r['workload']}): {', '.join(r['causes'])}." for r in orders[:5]]
    lines += ['', '## Re-scored development (precision / important recall)', '', '| Workload | Arm | Available/roots | Precision | Important recall | False reassurance |', '|---|---|---:|---:|---:|---:|']
    pct=lambda v:'unavailable' if v is None else f'{v*100:.2f}%'
    for w,arms in result['splits']['development'].items():
        for a,m in arms.items():
            lines.append(f"| {w} | {a} | {m['available_roots']}/{m['roots']} | {pct(m['precision'])} | {pct(m['important_change_recall'])} | {m['false_reassurance']} |")
    lines += ['', 'All individual comparisons, statements, geometry and classifications are in audit.json. Input SHA-256 receipts are in SCORE-development.json.',
        'Only development inputs were inspected. Frozen manifest topology and opaque oracle bytes were hashed; non-development oracle values were skipped lexically and never decoded. The fresh holdout directory was never accessed.','']
    (args.out/'REPORT.md').write_text('\n'.join(lines))
    print(json.dumps({k:v for k,v in audit.items() if k in ('classification_counts','order_disagreement_root_arms','overlapping_response_counts','new_development_reservation_nano_usd','recommendation')}))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('corpus','requests','results','local-results','out'): p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--source-revision',required=True)
    p.add_argument('--worker',action='store_true',help=argparse.SUPPRESS)
    args=p.parse_args()
    if args.worker: worker(args);return
    # Preflight split before opening ANY response or oracle value.
    manifest=json.loads((args.corpus/'manifest.json').read_bytes())
    if manifest['epoch']=='g12-fresh-heldout/1': raise ValueError('fresh holdout off limits')
    roots={c['root_id'] for c in manifest['cases'] if c['split']=='development'}
    rows=json.loads(args.requests.read_bytes())
    if any(r['root'].rsplit(':',3)[0] not in roots for r in rows): raise ValueError('development schedule required')
    if args.out.exists(): raise ValueError('refuse to overwrite audit directory')
    root=Path(__file__).resolve().parents[2]
    revision=subprocess.check_output(['git','rev-parse','--verify',args.source_revision+'^{commit}'],cwd=root,text=True).strip()
    archive=subprocess.check_output(['git','archive',revision],cwd=root)
    with tempfile.TemporaryDirectory(prefix='saccade-development-audit-') as temp:
        frozen=Path(temp)
        with tarfile.open(fileobj=io.BytesIO(archive)) as tar: tar.extractall(frozen,filter='data')
        # Atomic epoch-2 scorer stays pinned; new mappings are separately versioned.
        for name in ('score.py','pilot_score.py','policy.py','receipts.py'):
            if name=='pilot_score.py': continue
            if (root/'scripts/assist'/name).read_bytes() != (frozen/'scripts/assist'/name).read_bytes():
                # The split-selection patch postdates the frozen corpus but preserves semantic().
                raise ValueError('unreviewed scoring drift: '+name)
        args.out.mkdir(parents=True)
        program=("import sys;sys.path.insert(0,"+repr(str(frozen/'scripts/assist'))+");"
            "sys.path.append("+repr(str(root/'scripts/assist'))+");from dev_audit import main;main()")
        command=[sys.executable,'-c',program,'--worker','--corpus',str(args.corpus.resolve()),'--requests',str(args.requests.resolve()),
            '--results',str(args.results.resolve()),'--local-results',str(args.local_results.resolve()),'--out',str(args.out.resolve()),'--source-revision',revision]
        # Use current split-capable pilot scorer; its dependencies still resolve frozen.
        command[2]=program.replace('from dev_audit import main;',
            'import importlib.util;spec=importlib.util.spec_from_file_location("score",'+repr(str(root/'scripts/assist/score.py'))+');m=importlib.util.module_from_spec(spec);sys.modules["score"]=m;spec.loader.exec_module(m);spec=importlib.util.spec_from_file_location("pilot_score",'+repr(str(root/'scripts/assist/pilot_score.py'))+');m=importlib.util.module_from_spec(spec);sys.modules["pilot_score"]=m;spec.loader.exec_module(m);from dev_audit import main;')
        subprocess.run(command,cwd=frozen,check=True)


if __name__=='__main__': main()
