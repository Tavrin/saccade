#!/usr/bin/env python3
"""Offline partial live-pilot scoring; never grants synthetic/live qualification."""
import argparse
import copy
from collections import Counter, defaultdict
import json
import io
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path
from corpus import verify, digest, encoded, put
from score import assertion_correct, task_evidence, lower, upper, percentile
from stage2 import ARMS, payload, schedule, collect
from receipts import source_fact
from policy import WORKLOADS


def verified_corpus(directory, revision=None):
    if revision is None:
        return (*verify(directory), None)
    root = Path(__file__).resolve().parents[2]
    commit = subprocess.check_output(['git','rev-parse','--verify',revision+'^{commit}'],cwd=root,text=True).strip()
    archive = subprocess.check_output(['git','archive',commit],cwd=root)
    with tempfile.TemporaryDirectory(prefix='saccade-pilot-frozen-') as temp:
        frozen = Path(temp)
        with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
            tar.extractall(frozen,filter='data')
        # This adapter may use only unchanged scoring/oracle/payload semantics.
        for name in ('score.py','policy.py','receipts.py'):
            if (frozen/'scripts/assist'/name).read_bytes() != (root/'scripts/assist'/name).read_bytes():
                raise ValueError('frozen pilot scoring implementation drift: '+name)
        # Scheduling/freeze evolution must not change frozen rendering or payload semantics.
        import ast
        for name, functions in {'corpus.py': ('render',), 'stage2.py': ('schedule','payload','reservation','response_format','project_schema','collect')}.items():
            def semantics(path):
                tree=ast.parse(path.read_text())
                return {node.name:ast.dump(node,include_attributes=False) for node in tree.body if isinstance(node,(ast.FunctionDef,ast.AsyncFunctionDef)) and node.name in functions}
            if semantics(frozen/'scripts/assist'/name) != semantics(root/'scripts/assist'/name):
                raise ValueError('frozen pilot scoring implementation drift: '+name)
        def renderer_support(path):
            tree=ast.parse(path.read_text())
            tree.body=[n for n in tree.body if not (isinstance(n,ast.FunctionDef) and n.name in ('freeze','verify')) and not isinstance(n,ast.If)]
            return ast.dump(tree,include_attributes=False)
        if renderer_support(frozen/'scripts/assist/corpus.py') != renderer_support(root/'scripts/assist/corpus.py'):
            raise ValueError('frozen renderer support drift')
        # Constants bind prompt/reasoning/schema policies separately from functions.
        old_stage = (frozen/'scripts/assist/stage2.py').read_text().split('def priced(')[0]
        new_stage = (root/'scripts/assist/stage2.py').read_text().split('def priced(')[0]
        if old_stage != new_stage: raise ValueError('frozen payload policy drift')
        program = "import json,sys;from pathlib import Path;from corpus import verify;print(json.dumps(verify(Path(sys.argv[1]))))"
        data = subprocess.check_output([sys.executable,'-c',program,str(directory.resolve())],cwd=frozen/'scripts/assist')
        manifest, oracle = json.loads(data)
    return manifest, oracle, commit


def ratio(n, d):
    return n / d if d else None


def bounds(successes, n):
    return dict(events=successes, denominator=n,
                lower95=lower(successes, n) if n else None,
                upper95=upper(successes, n, .05) if n else None,
                best_all_success_lower95=lower(n, n) if n else None,
                best_zero_failure_upper95=upper(0, n, .05) if n else None)


def normalize(answer, case, order):
    """Convert normalized provider pixels to the frozen scorer's pixel dialect.

    Keep exact floating-point geometry: no padding, snapping or tolerance changes.
    """
    value = copy.deepcopy(answer)
    del value['request_hash']
    slots = {'P1': 'single'} if order == 'single' else (
        {'P1': 'after', 'P2': 'before'} if order == 'ba' else {'P1': 'before', 'P2': 'after'})
    for observation in value['observations']:
        observation['image_role'] = slots[observation.pop('slot')]
        observation['evidence_refs'] = [slots[r.split(':')[0]] + ':' + r.split(':')[1]
                                        for r in observation['evidence_refs']]
        coords = observation['geometry']['pixels']
        observation['geometry']['pixels'] = [n * case['dimensions'][i % 2] for i, n in enumerate(coords)]
    value['observations'].sort(key=encoded)
    return value


def semantic(answers, case, truth, directory, task_evidence_policy=False):
    """One root event; all required orders/descendants must survive collection."""
    assertion_fn, task_fn = assertion_correct, task_evidence
    if task_evidence_policy:
        from dev_policy import assertion_correct as assertion_fn, task_evidence as task_fn
    variants = {}
    for child in (False, True) if case.get('counterfactual') else (False,):
        orders = answers.get(child, [])
        if not orders or any(a is None for a in orders):
            return dict(complete=False, committed=False, correct=False, assertions=0,
                        correct_assertions=0, reassurance=False, abstention=False)
        if any(a != orders[0] for a in orders[1:]):
            return dict(complete=False, committed=False, correct=False, assertions=0,
                        correct_assertions=0, reassurance=False, abstention=False)
        variants[child] = orders[0]
    primary = variants[False]
    committed = primary['outcome'] != 'unverifiable' and bool(primary['observations'])
    obs = primary['observations'] if committed else []
    assertions = len(obs)
    correct_assertions = sum(assertion_fn(o, truth, case, directory) for o in obs)
    correct = (committed and correct_assertions == assertions and
               task_fn(obs, case, truth, directory) and primary['outcome'] == truth['expected_outcome'])
    if True in variants:
        child = variants[True]
        c = dict(case, after=case['counterfactual']['path'])
        t = dict(truth, rendered_witness=truth['counterfactual_witness'])
        obs = child['observations'] if child['outcome'] != 'unverifiable' else []
        count = sum(assertion_fn(o, t, c, directory) for o in obs)
        assertions += len(obs)
        correct_assertions += count
        correct = (correct and child['outcome'] == 'observed' and bool(obs) and count == len(obs) and
                   task_fn(obs, c, t, directory))
    # Same primary-outcome definition as score.evaluate; unsupported assertions
    # also reduce precision/recall but do not manufacture a reassurance outcome.
    return dict(complete=True, committed=bool(committed), correct=bool(correct),
                assertions=assertions, correct_assertions=correct_assertions,
                reassurance=bool(truth['important'] and committed and
                                 primary['outcome'] != truth['expected_outcome']),
                abstention=primary['outcome'] == 'unverifiable')


def summarize(items, calls):
    n = len(items)
    complete = sum(i['complete'] for i in items)
    committed = sum(i['committed'] for i in items)
    correct = sum(i['correct'] for i in items)
    important = sum(i['important'] for i in items)
    detected = sum(i['important'] and i['correct'] for i in items)
    reassurance = sum(i['reassurance'] for i in items)
    abstention = sum(i['abstention'] for i in items)
    assertions = sum(i['assertions'] for i in items)
    valid = sum(c['code'] == 'completed' for c in calls)
    invalid = sum(c['code'] == 'invalid_answer' for c in calls)
    over = sum(c['over_hint'] is True for c in calls)
    hints = sum(c['over_hint'] is not None for c in calls)
    attempted = sum(c['attempted'] for c in calls)
    known = sum(c['actual'] or 0 for c in calls)
    unknown = sum(c['attempted'] and c['actual'] is None for c in calls)
    charge = sum(c['actual'] if c['actual'] is not None else c['reservation'] for c in calls)
    latencies = [c['elapsed_ms'] for c in calls if c['elapsed_ms'] is not None]
    return dict(roots=n, available_roots=complete, availability=ratio(complete, n),
                committed_roots=committed, correct_roots=correct, precision=ratio(correct, committed),
                assertions=assertions, correct_assertions=sum(i['correct_assertions'] for i in items),
                assertion_precision=ratio(sum(i['correct_assertions'] for i in items), assertions),
                important_roots=important, detected_important_roots=detected,
                important_change_recall=ratio(detected, important), false_reassurance=reassurance,
                false_reassurance_rate=ratio(reassurance, important), abstentions=abstention,
                abstention_rate=ratio(abstention, n), scheduled_requests=len(calls), attempted_requests=attempted,
                completed_answers=valid, invalid_answers=invalid, invalid_answer_rate=ratio(invalid, valid+invalid),
                over_hint_requests=over, reasoning_observed_requests=hints, over_hint_rate=ratio(over, hints),
                known_cost_nano_usd=known, known_cost_usd=known/1e9, unknown_cost_receipts=unknown,
                charged_with_unknown_reservations_nano_usd=charge, charged_with_unknown_reservations_usd=charge/1e9,
                latency_samples=len(latencies), latency_p50_ms=percentile(latencies, .50),
                latency_p95_ms=percentile(latencies, .95), unavailable_request_codes=dict(Counter(
                    c['code'] for c in calls if c['code'] not in ('completed', 'invalid_answer'))),
                one_sided_95=dict(availability=bounds(complete, n), precision=bounds(correct, committed),
                    important_change_recall=bounds(detected, important), false_reassurance=bounds(reassurance, important),
                    abstention=bounds(abstention, n), invalid_answer=bounds(invalid, valid+invalid),
                    over_hint=bounds(over, hints)))


def evaluate(corpus, requests_file, result_dir, local_file, source_revision=None, split="heldout", task_evidence_policy=False):
    manifest, oracle, frozen_commit = verified_corpus(corpus, source_revision)
    payload_fn, normalize_fn = payload, normalize
    if task_evidence_policy:
        from dev_policy import payload as payload_fn, normalize as normalize_fn
    requests = json.loads(requests_file.read_bytes())
    smoke_file = result_dir/'smoke.json'
    smoke = json.loads(smoke_file.read_bytes())
    outcomes = smoke['root_outcomes']
    if len(outcomes) != len(requests):
        raise ValueError('pilot topology drift')
    ledger_file = result_dir/'ledger/campaign.json'
    money = json.loads(ledger_file.read_bytes())['money']
    if money['campaign_identity']['requests_hash'] != digest(requests_file.read_bytes()):
        raise ValueError('pilot request file identity drift')
    cases = {c['root_id']: c for c in manifest['cases']}
    ledger = {r['id']: r for r in money['receipts']}
    if len(ledger) != len(money['receipts']):
        raise ValueError('duplicate money receipt')
    seen = set()
    calls = defaultdict(list)
    answers = defaultdict(lambda: defaultdict(list))
    inputs = {str(requests_file): digest(requests_file.read_bytes()),
              str(smoke_file): digest(smoke_file.read_bytes()), str(ledger_file): digest(ledger_file.read_bytes()),
              str(corpus/'manifest.json'):digest((corpus/'manifest.json').read_bytes()),
              str(corpus/'oracle.json'):digest((corpus/'oracle.json').read_bytes())}

    def read(path):
        data = path.read_bytes()
        inputs[str(path)] = digest(data)
        return json.loads(data)

    indexed = set()
    for index, (row, outcome) in enumerate(zip(requests, outcomes)):
        root, arm, variant, order = row['root'].rsplit(':', 3)
        case = cases[root]
        child = variant == 'counter'
        if (arm not in ARMS or variant not in ('root', 'counter') or
            (child, order) not in schedule(case, arm) or (root, arm, child, order) in indexed or
            outcome['index'] != index or outcome['root'] != row['root'] or
            row['payload'] != payload_fn(case, order, child, corpus)):
            raise ValueError('pilot frozen request/root binding drift')
        indexed.add((root, arm, child, order))
        receipt = ledger.get(outcome.get('execution_id'))
        actual = None
        reservation = 0
        hint = None
        elapsed = None
        if receipt:
            if (receipt['id'] in seen or receipt['usage']['campaign_root_index'] != index or
                receipt['request_hash'] != digest(encoded(row['payload']))):
                raise ValueError('pilot money binding drift')
            seen.add(receipt['id'])
            actual = receipt['actual_nano_usd']
            reservation = receipt['reserved_nano_usd']
            if type(reservation) is not int or reservation <= 0 or (actual is not None and (type(actual) is not int or actual < 0)):
                raise ValueError('invalid pilot cost')
            meta = receipt['usage'].get('reasoning_hint', {})
            observed = meta.get('observed_tokens')
            requested = meta.get('requested_tokens')
            if type(observed) is int and type(requested) is int and observed >= 0 and requested >= 0:
                hint = observed > requested
                if receipt['usage'].get('provider_reasoning_over_hint') != hint:
                    raise ValueError('pilot reasoning hint drift')
        answer = None
        if outcome['code'] == 'completed':
            if not receipt or receipt['outcome'] != 'completed' or actual is None:
                raise ValueError('completed pilot answer lacks settled money')
            provenance = read(result_dir/f'receipt-{index}.json')
            response_path = result_dir/f'response-{index}.json'
            raw = response_path.read_bytes()
            inputs[str(response_path)] = digest(raw)
            if (provenance['execution_id'] != receipt['id'] or provenance['request_hash'] != receipt['request_hash'] or
                provenance['response_hash'] != digest(raw)):
                raise ValueError('pilot answer provenance drift')
            value = read(result_dir/f'answer-{index}.json')
            if json.loads(json.loads(raw)['choices'][0]['message']['content']) != value:
                raise ValueError('pilot answer/response drift')
            expected = json.loads(row['payload']['messages'][1]['content'][0]['text'])['request_hash']
            if value['request_hash'] != expected:
                raise ValueError('pilot answer request drift')
            answer = normalize_fn(value, case, order)
            elapsed = provenance['elapsed_ms']
            if type(elapsed) is not int or elapsed < 0:
                raise ValueError('pilot latency drift')
        answers[(root, arm)][child].append(answer)
        calls[(case['split'], case['workload'], arm)].append(dict(code=outcome['code'], attempted=bool(receipt),
            actual=actual, reservation=reservation, over_hint=hint, elapsed_ms=elapsed))
    if seen != set(ledger):
        raise ValueError('unbound pilot money receipts')
    # A changed/truncated schedule must not turn absent orders into available roots.
    expected = {(c['root_id'], a, child, order) for c in manifest['cases'] if c['split']==split
                for a in ARMS for child, order in schedule(c, a)}
    if indexed != expected:
        raise ValueError('pilot schedule incomplete or changed')
    local = read(local_file)
    expected_local = [dict(root=c['root_id'], arm=a, outcome='observed' if source_fact(c) and c['complete'] else 'unverifiable',
        source_only=source_fact(c) and c['complete']) for c in manifest['cases'] if c['split']==split
        for a in ('rules','cascade') if a=='rules' or not c['complete'] or source_fact(c)]
    if local != expected_local:
        raise ValueError('pilot local result drift')
    local = {(r['root'], r['arm']): r for r in local}
    grouped = defaultdict(list)
    root_results = []
    for case in manifest['cases']:
        if case['split'] != split: continue
        truth = oracle[case['case_id']]
        for arm in ARMS:
            key = (case['root_id'], arm)
            # Development/calibration are explicitly unobserved; never infer runs.
            if key in local:
                r = local[key]
                committed = r['source_only']
                correct = committed and r['outcome'] == truth['expected_outcome']
                item = dict(complete=True, committed=committed, correct=correct, assertions=int(committed),
                    correct_assertions=int(correct), reassurance=bool(truth['important'] and committed and not correct),
                    abstention=r['outcome']=='unverifiable')
            else:
                item = semantic(answers[key], case, truth, corpus, task_evidence_policy)
            item.update(root=case['root_id'], arm=arm, split=case['split'], workload=case['workload'],
                        important=truth['important'], family=case['family'])
            grouped[(case['split'], case['workload'], arm)].append(item)
            root_results.append(item)
    splits = {split: {w: {a: summarize(grouped[(split,w,a)], calls[(split,w,a)]) for a in ARMS}
                       for w in WORKLOADS} for split in ('development','calibration','heldout')}
    return dict(schema='saccade-g12-partial-score.v1', status='PARTIAL, UNQUALIFIED PILOT', qualified=False,
        live_model_qualification=False, geometry_mode=__import__('dev_policy').GEOMETRY_MODE if task_evidence_policy else 'strict',geometry_policy='assist-region-geometry/2' if task_evidence_policy else 'legacy-exact', task_evidence_policy='g12-pilot/5' if task_evidence_policy else 'g12-pilot/2', frozen_source_commit=frozen_commit,
        frozen_gate_source_hash=manifest['versions']['gate_source_hash'], manifest_hash=manifest['manifest_hash'], oracle_hash=manifest['oracle_hash'],
        scoring_source_sha256={name:digest((Path(__file__).parent/name).read_bytes()) for name in
            ('pilot_score.py','score.py','corpus.py','stage2.py','receipts.py','policy.py',*(['dev_policy.py','scorer_selftest.py'] if task_evidence_policy else []))},
        inputs_sha256=inputs, splits=splits, root_results=root_results,
        campaign=summarize([i for i in root_results if i['split']==split], [c for group in calls.values() for c in group]),
        mechanics=collect(requests,result_dir,manifest),
        methods=['Uses score.assertion_correct and score.task_evidence against the verified frozen corpus oracle.',
            'Precision is all-facts/task/outcome-correct roots divided by committed roots; assertion precision is separate.',
            'Availability includes all planned root/arms; every missing order/descendant or exact normalized order disagreement is unavailable.',
            'Abstention counts completed consistent unverifiable roots, separate from unavailable roots.',
            'False reassurance uses the existing primary outcome mismatch on important committed roots.',
            'Money includes all dispatches; unknown costs retain full reservations and are never reported as known billed cost.',
            'Latency is completed cold-call receipt latency only; no latency inferred for missing/failed calls.',
            'One-sided 95% Clopper-Pearson bounds use independent root events for quality; request rates are descriptive and correlated.',
            'No changes to existing qualification gates or synthetic receipt acceptance.'],
        limitations=[f'Campaign schedules only {split}; other splits have no executed observations.',
            'This heldout corpus has already informed prompt development; it is not a fresh prospective holdout.',
            'Root/arm samples share template families and arms; binomial bounds are nominal, not cluster-adjusted qualification evidence.',
            'Finite assertion vocabulary; unsupported facts count as incorrect. Epoch-5 region matching uses explicit IoU/coverage thresholds; strict mode retains exact enclosure.' if task_evidence_policy else 'Finite assertion vocabulary; unsupported facts count as incorrect. Geometry is not padded to repair decimal rounding.',
            'Epoch-5 audit-mask statements cite one region each; declared exclusion mappings preserve task evidence. Observations never approve masks.' if task_evidence_policy else 'Audit-mask task evidence requires exclusion-ID citations; the pilot allows same-slot region citations only, so those answers cannot satisfy the frozen audit task scorer.',
            'Exact normalized order comparison is conservative; an incomplete paired root cannot contribute a precision success.',
            'Generation reconciliation remains pending. Unknown transport cost is charged at reservation in this report only; source ledger is read-only.'])


def markdown(report):
    lines = ['# PARTIAL, UNQUALIFIED PILOT', '', 'Offline scoring of the existing pilot only. No provider calls or key reads.', '',
        f"Frozen manifest: `{report['manifest_hash']}`; oracle: `{report['oracle_hash']}`.", '',
        'Quality denominators are root/arms, including missing orders and descendants. Rates below are percentages; FR is false reassurance.', '']
    pct = lambda v: 'unavailable' if v is None else f'{v*100:.2f}%'
    for split, workloads in report['splits'].items():
        lines += [f'## {split}', '', '| Workload | Arm | Available/roots | Precision | Important recall | FR | Abstention | Invalid | Over hint | Known cost $ | Charged $ | p50/p95 ms |',
            '|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|']
        for workload, arms in workloads.items():
            for arm, m in arms.items():
                lines.append(f"| {workload} | {arm} | {m['available_roots']}/{m['roots']} | {pct(m['precision'])} | {pct(m['important_change_recall'])} | {m['false_reassurance']}/{m['important_roots']} | {pct(m['abstention_rate'])} | {pct(m['invalid_answer_rate'])} | {pct(m['over_hint_rate'])} | {m['known_cost_usd']:.8f} | {m['charged_with_unknown_reservations_usd']:.8f} | {m['latency_p50_ms']}/{m['latency_p95_ms']} |")
        lines += ['', '### Exact nominal one-sided 95% bounds', '',
            '| Workload | Arm | Availability lower | Precision lower | Recall lower | FR upper | Abstention upper | Invalid upper | Over-hint upper | Best zero-FR upper (n) |',
            '|---|---|---:|---:|---:|---:|---:|---:|---:|---:|']
        for workload, arms in workloads.items():
            for arm, m in arms.items():
                b = m['one_sided_95']
                fields = [b[k][bound] for k, bound in [('availability','lower95'), ('precision','lower95'),
                    ('important_change_recall','lower95'), ('false_reassurance','upper95'), ('abstention','upper95'),
                    ('invalid_answer','upper95'), ('over_hint','upper95')]]
                lines.append(f"| {workload} | {arm} | " + ' | '.join(pct(v) for v in fields) +
                    f" | {pct(b['false_reassurance']['best_zero_failure_upper95'])} ({m['important_roots']}) |")
        lines += ['']
    m = report['campaign']
    lines += ['## Accounting and support', '',
        f"{m['completed_answers']} completed answers; {m['attempted_requests']} monetary attempts; {m['invalid_answers']} invalid answers; {m['over_hint_requests']}/{m['reasoning_observed_requests']} observed calls over hint.",
        f"Known billed cost ${m['known_cost_usd']:.8f}; {m['unknown_cost_receipts']} unknown-cost receipt(s); charge including full unknown reservations ${m['charged_with_unknown_reservations_usd']:.8f}.", '',
        'For n=0, confidence limits are unavailable. For n>0, best all-success lower = 0.05^(1/n); best zero-failure upper = 1 - 0.05^(1/n). JSON contains counts and full-precision bounds for every metric.', '',
        'Unscheduled splits describe absence of execution, not measured model performance. No budget or quality qualification is implied.', '',
        '## Method and limitations', '']
    lines += ['- '+s for s in report['methods']+report['limitations']]
    lines += ['', '## Input identity', '']
    lines += [f'- `{p}`: `{h}`' for p,h in sorted(report['inputs_sha256'].items())]
    return '\n'.join(lines)+'\n'


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--corpus',type=Path,required=True)
    parser.add_argument('--requests',type=Path,required=True)
    parser.add_argument('--results',type=Path,required=True)
    parser.add_argument('--local-results',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--source-revision',help='Verify the original frozen source snapshot; current scorer semantics must match.')
    parser.add_argument("--split",choices=("development","calibration","held-out"),default="held-out")
    parser.add_argument("--task-evidence-policy", action="store_true", help="Score epoch-5 task-evidence requests; epoch-2 replay is refused.")
    parser.add_argument("--geometry-mode", choices=("tolerance","strict"), default="tolerance", help="Epoch-5 qualification defaults to region tolerance; strict retains exact enclosure.")
    args = parser.parse_args()
    if args.task_evidence_policy:
        import dev_policy
        dev_policy.GEOMETRY_MODE=args.geometry_mode
    report = evaluate(args.corpus,args.requests,args.results,args.local_results,args.source_revision,args.split.replace("held-out","heldout"),args.task_evidence_policy)
    put(args.out.with_suffix('.json'),report)
    args.out.with_suffix('.md').write_text(markdown(report))
    print(json.dumps({k:report['campaign'][k] for k in ('completed_answers','invalid_answers','over_hint_requests','known_cost_usd','charged_with_unknown_reservations_usd')}))
