#!/usr/bin/env python3
"""Offline campaign union with aggregate-only held-out publication.

The scorer and frozen protocol are unchanged. Individual response audit is
permitted only on development/calibration; absence of those runs is explicit.
"""
import argparse
from collections import Counter, defaultdict
import copy
import json
from pathlib import Path
import shutil
import tempfile

from corpus import digest, encoded, put
from pilot_score import evaluate, markdown, summarize
from stage2 import ARMS
from policy import WORKLOADS


def merge(requests, campaigns):
    """Validate all identities before selecting answers; retain every paid attempt.

    Campaigns are (request-file, result-directory) pairs in execution order.
    A replay of an already terminal answer is refused, even if identical.
    """
    by_root = {r['root']: i for i, r in enumerate(requests)}
    by_hash = defaultdict(dict)
    for i, row in enumerate(requests):
        by_hash[digest(encoded(row['payload']))][row['root']] = i
    if len(by_root) != len(requests):
        raise ValueError('duplicate canonical root')
    selected = {}
    attempts = []
    inputs = {}
    identities = set()
    for request_file, directory in campaigns:
        def read(path):
            raw = path.read_bytes()
            inputs[str(path)] = digest(raw)
            return json.loads(raw)
        rows = read(request_file)
        outcomes = read(directory/'smoke.json')['root_outcomes']
        money = read(directory/'ledger/campaign.json')['money']
        if money['campaign_identity']['requests_hash'] != inputs[str(request_file)]:
            raise ValueError('campaign request file hash mismatch')
        if len(outcomes) != len(rows):
            raise ValueError('campaign topology mismatch')
        receipts = {r['id']: r for r in money['receipts']}
        if len(receipts) != len(money['receipts']):
            raise ValueError('duplicate money receipt')
        seen = set()
        row_seen = set()
        for index, (row, outcome) in enumerate(zip(rows, outcomes)):
            canonical = by_root.get(row['root'])
            if canonical is None or canonical in row_seen:
                raise ValueError('unknown or duplicate campaign root')
            row_seen.add(canonical)
            if (row != requests[canonical] or
                    by_hash.get(digest(encoded(row['payload'])), {}).get(row['root']) != canonical):
                raise ValueError('campaign payload hash mismatch')
            if outcome['index'] != index or outcome['root'] != row['root']:
                raise ValueError('campaign outcome binding mismatch')
            receipt = receipts.get(outcome.get('execution_id'))
            if receipt:
                if (receipt['id'] in seen or receipt['id'] in identities or
                    receipt['usage']['campaign_root_index'] != index or
                    receipt['request_hash'] != digest(encoded(row['payload']))):
                    raise ValueError('campaign money binding mismatch')
                seen.add(receipt['id']); identities.add(receipt['id'])
                actual, reservation = receipt['actual_nano_usd'], receipt['reserved_nano_usd']
                if (type(reservation) is not int or reservation <= 0 or
                    actual is not None and (type(actual) is not int or actual < 0)):
                    raise ValueError('invalid campaign cost')
                hint = receipt['usage'].get('reasoning_hint', {})
                observed, requested = hint.get('observed_tokens'), hint.get('requested_tokens')
                over = None
                if all(type(n) is int and n >= 0 for n in (observed, requested)):
                    over = observed > requested
                    if receipt['usage'].get('provider_reasoning_over_hint') != over:
                        raise ValueError('campaign reasoning hint mismatch')
                elapsed = None
                if outcome['code'] == 'completed':
                    provenance = read(directory/f'receipt-{index}.json')
                    elapsed = provenance['elapsed_ms']
                    if type(elapsed) is not int or elapsed < 0:
                        raise ValueError('invalid campaign latency')
                attempts.append((row['root'], dict(code=outcome['code'], attempted=True,
                    actual=actual, reservation=reservation, over_hint=over, elapsed_ms=elapsed)))
            elif outcome['code'] in ('completed', 'invalid_answer'):
                raise ValueError('terminal answer lacks money receipt')
            previous = selected.get(canonical)
            terminal = ('completed', 'invalid_answer')
            if previous and previous[2]['code'] in terminal:
                if outcome['code'] in terminal or receipt:
                    raise ValueError('duplicate terminal request execution')
                continue
            # Never replace a paid failure by an undispatched skip.
            if previous and previous[3] and not receipt:
                continue
            selected[canonical] = (directory, index, outcome, receipt)
        if seen != set(receipts):
            raise ValueError('unbound campaign money receipt')
    if set(selected) != set(range(len(requests))):
        raise ValueError('combined schedule incomplete')
    return selected, attempts, inputs


def audit(corpus, requests_file, campaigns, local_file, source_revision):
    requests = json.loads(requests_file.read_bytes())
    selected, attempts, inputs = merge(requests, campaigns)
    # Temporary transport adapter only; all originals stay read-only. The legacy
    # scorer checks body/provenance/oracle binding internally, never printing them.
    with tempfile.TemporaryDirectory(prefix='saccade-pilot-union-') as temp:
        directory = Path(temp); (directory/'ledger').mkdir()
        outcomes, receipts = [], []
        original_paths = {}
        for canonical, (origin, index, outcome, receipt) in sorted(selected.items()):
            outcome = dict(outcome, index=canonical)
            outcomes.append(outcome)
            if receipt:
                receipt = copy.deepcopy(receipt)
                receipt['usage']['campaign_root_index'] = canonical
                receipts.append(receipt)
            if outcome['code'] == 'completed':
                for prefix in ('receipt', 'response', 'answer'):
                    src = origin/f'{prefix}-{index}.json'
                    dst = directory/f'{prefix}-{canonical}.json'
                    shutil.copyfile(src, dst)
                    original_paths[str(dst)] = str(src)
        put(directory/'smoke.json', dict(root_outcomes=outcomes))
        put(directory/'ledger/campaign.json', dict(money=dict(
            campaign_identity=dict(requests_hash=digest(requests_file.read_bytes())), receipts=receipts)))
        report = evaluate(corpus, requests_file, directory, local_file, source_revision)
        for path, sha in report['inputs_sha256'].items():
            if path in original_paths:
                inputs[original_paths[path]] = sha
            elif not Path(path).is_relative_to(directory):
                inputs[path] = sha
    manifest = json.loads((corpus/'manifest.json').read_bytes())
    cases = {c['root_id']: c for c in manifest['cases']}
    calls = defaultdict(list)
    for row_root, call in attempts:
        root, arm, _, _ = row_root.rsplit(':', 3)
        case = cases[root]
        calls[(case['split'], case['workload'], arm)].append(call)
    for canonical, (_, _, outcome, receipt) in selected.items():
        if not receipt:
            root, arm, _, _ = requests[canonical]['root'].rsplit(':', 3)
            case = cases[root]
            calls[(case['split'], case['workload'], arm)].append(dict(code=outcome['code'], attempted=False,
                actual=None, reservation=0, over_hint=None, elapsed_ms=None))
    grouped = defaultdict(list)
    for item in report.pop('root_results'):
        grouped[(item['split'], item['workload'], item['arm'])].append(item)
    report['splits'] = {s: {w: {a: summarize(grouped[(s,w,a)], calls[(s,w,a)]) for a in ARMS}
        for w in WORKLOADS} for s in ('development', 'calibration', 'heldout')}
    all_items = [i for group in grouped.values() for i in group if i['split'] == 'heldout']
    report['campaign'] = summarize(all_items, [c for group in calls.values() for c in group])
    report['campaign']['scheduled_requests'] = len(requests)
    for (s,w,a), group in calls.items():
        # Physical retries are attempts, not additional planned requests.
        report['splits'][s][w][a]['scheduled_requests'] = sum(
            cases[r['root'].rsplit(':',3)[0]]['split'] == s and
            cases[r['root'].rsplit(':',3)[0]]['workload'] == w and r['root'].rsplit(':',3)[1] == a
            for r in requests)
    mechanics = report['mechanics']
    reasons = defaultdict(Counter)
    for item in mechanics.pop('root_arm_results'):
        case = cases[item['root']]
        reason = 'invalid_answer' if item['invalid_answer'] else 'missing_sample' if item['missing'] else 'exact_order_disagreement' if item['order_disagreement'] else 'consistent'
        reasons[(case['split'],case['workload'],item['arm'])][reason] += 1
    mechanics['aggregate_variant_reasons'] = [dict(split=s,workload=w,arm=a,counts=dict(c))
        for (s,w,a),c in sorted(reasons.items())]
    scheduled_keys = {tuple(r['root'].rsplit(':', 3)[:2]) for r in requests}
    mechanics['unavailable_root_arms'] = sum(not i['complete'] and (i['root'], i['arm']) in scheduled_keys
        for i in all_items)
    report.update(schema='saccade-g12-pilot-score.v1', status='UNQUALIFIED PILOT', inputs_sha256=inputs,
        combination=dict(verified_scheduled_requests=len(requests),
            distinct_payload_hashes=len({digest(encoded(r['payload'])) for r in requests}),
            identity='payload hash with canonical root/arm/order binding; identical cross-arm payloads remain separate scheduled events'),
        scorer_changes=[], union_adapter_sha256=digest(Path(__file__).read_bytes()), run_b_recommendation='Do not run B. First obtain a separately authorized development/calibration audit; no prompt/policy/scorer tuning from this held-out pilot.',
        audit=dict(inspected_development_calibration_responses=0, scored_disagreements_reviewed=0,
            model_error_count=None, scorer_artefact_count=None,
            empirical_classification='unavailable: all executed requests are heldout',
            protocol_findings=[
                'Literal text comparison preserves line breaks, whitespace, Unicode code points and case; no normalization is authorized by the atomic protocol.',
                'Two-order arms require exact normalized answers, including geometry, citations, uncertainty and visibility; disagreement withholds the entire root. check_ui has one sample by schedule, not an independent two-sample arm.',
                'presence:present can be a correct assertion but does not establish check_ui task success; abstention requires a consistent completed unverifiable outcome.',
                'Geometry uses exact half-open glyph coverage and full target coverage; no IoU threshold or rounding tolerance.',
                'audit_mask requires exclusion-ID citations, but the live validator permits same-slot region IDs only. This is a protocol incompatibility, not demonstrated model error; unchanged pending a development audit.']))
    report['limitations'] = [s for s in report['limitations'] if not s.startswith('Generation reconciliation')]
    report['limitations'] += ['Both campaigns retain all physical attempts. The earlier unknown-cost transport reservation is counted even when its request later completes.',
        'No individual held-out result is published or inspected for tuning. Automated scorer reads are aggregate evaluation only.',
        'No empirical development/calibration disagreement classification is possible with these inputs. Zero reviewed disagreements does not mean zero model/scorer errors.']
    return report


def render(report):
    text = markdown(report).replace('# PARTIAL, UNQUALIFIED PILOT', '# UNQUALIFIED PILOT', 1)
    preface = ('Scorer changes: none. The frozen literal protocol, exact geometry and exact order agreement remain unchanged. '
        'Development/calibration have no executed responses; individual disagreement audit is unavailable.\n\n'
        + report['run_b_recommendation'] + '\n\n')
    text = text.replace('Offline scoring', preface+'Offline scoring', 1)
    text += '\n## Scorer audit (source and synthetic evidence only)\n\n'
    text += '\n'.join('- '+s for s in report['audit']['protocol_findings'])+'\n'
    text += '\nReviewed disagreements: 0. Model-error and scorer-artefact counts: unavailable.\n'
    text += '\n### Aggregate combination reasons (variants, not independent roots)\n\n'
    text += '| Split | Workload | Arm | Counts |\n|---|---|---|---|\n'
    for r in report['mechanics']['aggregate_variant_reasons']:
        text += f"| {r['split']} | {r['workload']} | {r['arm']} | {r['counts']} |\n"
    return text


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, required=True)
    parser.add_argument('--requests', type=Path, required=True)
    parser.add_argument('--campaign', nargs=2, action='append', type=Path, required=True,
                        metavar=('REQUESTS', 'RESULTS'))
    parser.add_argument('--local-results', type=Path, required=True)
    parser.add_argument('--source-revision', required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.out.with_suffix('.json').exists() or args.out.with_suffix('.md').exists():
        raise ValueError('refuse to overwrite pilot report')
    result = audit(args.corpus,args.requests,args.campaign,args.local_results,args.source_revision)
    put(args.out.with_suffix('.json'), result)
    args.out.with_suffix('.md').write_text(render(result))
    print(json.dumps(dict(status=result['status'], **result['campaign'])))
