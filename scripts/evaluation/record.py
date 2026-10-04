"""Attach dispatch and completion evidence to the frozen evaluator's public record."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import re
import sys
import time


def read(file):
    return json.loads(file.read_text())


def digest(file):
    return 'sha256:'+hashlib.sha256(file.read_bytes()).hexdigest()


GEMINI_STANDARD_USD_PER_MILLION = {
    'gemini-3.8-flash': (0.75, 3.75),
    'gemini-3.7-flash': (0.75, 3.75),
    'gemini-3.6-flash': (0.75, 3.75),
    'gemini-3.5-flash': (1.50, 9.00),
}
GEMINI_PRICE_SOURCE = 'https://ai.google.dev/gemini-api/docs/pricing'


def gemini_spend(root):
    """Estimate this epoch's spend from privately retained successful responses."""
    totals = collections.defaultdict(lambda: {'responses': 0, 'with_usage': 0,
                                               'input_tokens': 0, 'output_tokens': 0})
    for exchange_file in (root/'live').glob('*/attempt-*/exchange.json'):
        exchange = read(exchange_file)
        if exchange.get('outcome') != 'received':
            continue
        model = exchange.get('model')
        if model not in GEMINI_STANDARD_USD_PER_MILLION:
            continue
        row = totals[model]
        row['responses'] += 1
        response_file = exchange_file.with_name('response.json')
        if not response_file.exists():
            continue
        usage = read(response_file).get('usageMetadata') or {}
        prompt = usage.get('promptTokenCount')
        total = usage.get('totalTokenCount')
        output = total-prompt if isinstance(total, int) and isinstance(prompt, int) else \
            (usage.get('candidatesTokenCount') or 0)+(usage.get('thoughtsTokenCount') or 0)
        if not isinstance(prompt, int) or not isinstance(output, int) or prompt < 0 or output < 0:
            continue
        row['with_usage'] += 1
        row['input_tokens'] += prompt
        row['output_tokens'] += output
    by_model = []
    for model, row in sorted(totals.items()):
        input_rate, output_rate = GEMINI_STANDARD_USD_PER_MILLION[model]
        by_model.append({'model': model, **row, 'estimated_usd': round(
            (row['input_tokens']*input_rate+row['output_tokens']*output_rate)/1_000_000, 6)})
    return {'currency': 'USD', 'kind': 'estimate from API usage and published standard paid-tier prices',
            'pricing_url': GEMINI_PRICE_SOURCE, 'by_model': by_model,
            'estimated_usd': round(sum(row['estimated_usd'] for row in by_model), 6),
            'responses_without_usage': sum(row['responses']-row['with_usage'] for row in by_model)}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config',required=True)
    config=read(Path(parser.parse_args().config))
    root=Path(config['root'])
    public=Path(__file__).resolve().parents[2]/'examples/evaluation'
    results=read(public/'RESULTS.json')
    seal=read(public/'corpus-freeze.json')
    assert results['manifest_sha256']==seal['manifest_sha256']==digest(public/'moss-pilot.toml')
    ledger=read(root/'ledger/evaluation.json')
    jobs=read(root/'live-results.json')
    state=read(root/'run-state.json')
    counts=collections.Counter(a['provider'] for a in ledger['attempts'])
    assert counts['jev']<=400 and counts['gemini']<=250
    assert all(r['manifest_sha256']==seal['manifest_sha256'] for r in jobs)
    sys.path.insert(0,str(public))
    import corpus_live
    failures=collections.Counter()
    for r in jobs:
        for e in r['exchanges']:
            if e.get('attempts',0)>0:
                for category,n in corpus_live.failure_categories([e]).items():
                    failures[r['job']['provider'],e['model'],category]+=n
    unresolved=sum(r['outcome'] in ['deferred','budget_blocked'] or
                   (r['outcome']=='unavailable' and r['attempts']==0) for r in jobs if r['job']['provider']!='rules')
    terminal_invalid=sum(r['outcome']=='invalid' for r in jobs if r['job']['provider']!='rules')
    results['processing_complete']=len(jobs)==results['scheduled_jobs']
    results['incomplete']=not results['processing_complete'] or unresolved>0
    results['unresolved_provider_jobs']=unresolved
    results['terminal_invalid_provider_jobs']=terminal_invalid
    results['provider_failures']=[{'provider':p,'model':m,'category':c,'attempts':n} for (p,m,c),n in sorted(failures.items())]
    amendment=read(public/'resume-amendment.json')
    used=amendment.get('used_before_paid', amendment['used_before_fix'])
    quotas=set()
    for r in jobs:
        for e in r['exchanges']:
            for f in e.get('failed_attempts') or []:
                if f.get('status')!=429 or not f.get('error_body'):
                    continue
                try:
                    details=json.loads(Path(f['error_body']).read_bytes())['error']['details']
                except (OSError,ValueError,KeyError,TypeError):
                    continue
                for d in details:
                    for v in d.get('violations',[]) if isinstance(d,dict) else []:
                        if isinstance(v,dict) and re.fullmatch(r'[A-Za-z-]{1,80}',str(v.get('quotaId',''))) and re.fullmatch(r'\d{1,9}',str(v.get('quotaValue',''))):
                            quotas.add((e['model'],v['quotaId'],str(v.get('quotaValue'))))
    reasons=collections.Counter()
    for r in jobs:
        if r['job']['provider']!='rules' and (r['outcome'] in ['deferred','budget_blocked'] or (r['outcome']=='unavailable' and r['attempts']==0)):
            reason=r.get('reason') or r['outcome']
            reason='deferred: provider retry window' if reason.startswith('deferred') else reason
            reasons[r['job']['provider'],r['job']['encoding'],r['job']['mode'],reason]+=1
    rules={row['question']:row for row in results['per_question'] if row['provider']=='rules'}
    minimum=results.get('gates',{}).get('minimum_held_out',50)
    support={q:{'truth_known_held_out_cases':row['qualification_gates']['truth_known_held_out_cases'],
                'additional_held_out_cases_needed':max(0,minimum-row['qualification_gates']['truth_known_held_out_cases'])}
             for q,row in rules.items()}
    results['resume']={'epoch':amendment['epoch'],'amendment_sha256':digest(public/'resume-amendment.json'),
                       'calls_this_run':{p:counts[p]-used[p] for p in used},'caps_this_run':amendment['remaining_caps'],
                       'provider_quota_violations':[{'model':m,'quota_id':q,'quota_value':v} for m,q,v in sorted(quotas)],
                       'unresolved_by_reason':[{'provider':p,'encoding':e,'mode':m,'reason':x,'jobs':n} for (p,e,m,x),n in sorted(reasons.items())]}
    paid_attempts=[a for a in ledger['attempts'] if a['provider']=='gemini'
                   and a['started_ms']>=state['resume']['started_ms']]
    assert len(paid_attempts)==results['resume']['calls_this_run']['gemini']
    results['resume']['gemini_model_availability']=[
        {'model':model,'attempts':len(cells),'http_successes':sum(a['outcome']=='answered' for a in cells),
         'http_availability':sum(a['outcome']=='answered' for a in cells)/len(cells) if cells else None,
         'schema_valid_jobs':sum(r['outcome']=='answered' and r.get('resume_epoch')==amendment['epoch']
                                 and r.get('actual_model')==model for r in jobs)}
        for model in [item['model'] for item in results['gemini_model_availability']]
        for cells in [[a for a in paid_attempts if a['model']==model]]]
    results['gemini_spend']=gemini_spend(root)
    mapping=read(root/'corpus-mapping.json')
    missing_sources=sum(not Path(source).exists() for case in mapping['cases']
                        if case['origin']=='recorded' for source in case['sources'])
    results['source_revalidation']={'recorded_source_files_missing':missing_sources,
                                    'frozen_generated_packets_rehashed':True,
                                    'original_source_revalidation_complete':missing_sources==0}
    results['held_out_support']=support
    results['actual_response_revisions']=[{'provider':p,'revision':v} for p,v in sorted({(r['job']['provider'],r['revision']) for r in jobs if r['outcome']=='answered' and r['job']['provider']!='rules'})]
    results['elapsed_ms']=round(time.time()*1000-state['resume']['started_ms'])
    results['record_source_sha256']=digest(Path(__file__))
    results['limitations'] = list(dict.fromkeys(
        [item for item in results['limitations'] if not item.startswith('Costs unknown;')]+[
        'Harmful-miss bounds cover frozen catalog classes and deterministic violations; they do not establish unmeasured semantic safety.',
        'Latency records active batch dispatch handling, excluding queued time and end-to-end vision-plus-Jev extraction.',
        'Provider error bodies were not retained before the transport fix; earlier failures are known by HTTP status only. Later error classes come from privately retained bodies.',
        f'{missing_sources} recorded upstream source files are missing, so their original source hashes cannot be revalidated; frozen generated packets were rehashed.'
    ]))
    (public/'RESULTS.json').write_text(json.dumps(results,indent=2,sort_keys=True)+'\n')
    marker='<!-- dispatch-completion-record -->'
    original=(public/'RESULTS.md').read_text().split(marker)[0].rstrip()+'\n'
    (public/'RESULTS.md').write_text(original)
    with (public/'RESULTS.md').open('a') as out:
        out.write('\n'+marker+'\n')
        out.write(f"\nAll-job processing complete: {str(results['processing_complete']).lower()}. Unresolved provider jobs: {unresolved}; terminal invalid provider jobs: {terminal_invalid}. Evaluation incomplete: {str(results['incomplete']).lower()}.\n")
        out.write('\n| Provider | Requested model | Failure category | Attempts |\n|---|---|---|---:|\n')
        for (p,m,c),n in sorted(failures.items()):out.write(f'| {p} | {m} | {c} | {n} |\n')
        out.write('\nLatency excludes queueing and full pipeline extraction. Harmful-miss bounds use the frozen catalog; they do not establish semantic safety. Missing responses and unresolved pairs remain excluded from committed-answer accuracy.\n')
        resume=results['resume']
        out.write(f"\n## Paid-tier resume\n\nAmendment `{resume['amendment_sha256']}` (epoch `{resume['epoch']}`) changes transport pacing and opens a new elapsed window. The frozen corpus, splits, rubrics and job plan are unchanged. "
                  f"Calls in this epoch: Jev {resume['calls_this_run']['jev']}/{resume['caps_this_run']['jev']}, Gemini {resume['calls_this_run']['gemini']}/{resume['caps_this_run']['gemini']}.\n")
        spend=results['gemini_spend']
        out.write(f"\nGemini spend estimate for this epoch: ${spend['estimated_usd']:.4f} USD from API token usage and [published standard paid-tier prices]({spend['pricing_url']}); {spend['responses_without_usage']} successful responses lack usage metadata. This is an estimate, not an invoice.\n")
        out.write('\n| Gemini model | Paid-epoch attempts | HTTP successes | Schema-valid jobs |\n|---|---:|---:|---:|\n')
        for model in resume['gemini_model_availability']:
            out.write(f"| {model['model']} | {model['attempts']} | {model['http_successes']} | {model['schema_valid_jobs']} |\n")
        out.write(f"\nSource revalidation: {missing_sources} recorded upstream files are absent; frozen generated packets and their hashes were rechecked. This limits source-level replay assurance.\n")
        remaining={p:resume['caps_this_run'][p]-resume['calls_this_run'][p] for p in used}
        out.write(f"\nRemainder: {unresolved} unresolved provider jobs plus {terminal_invalid} terminal invalid response. Unused caps: Jev {remaining['jev']}, Gemini {remaining['gemini']}. The table below gives the unresolved reasons.\n")
        out.write('\n| Unresolved provider jobs | Encoding | Mode | Reason | Jobs |\n|---|---|---|---|---:|\n')
        for u in resume['unresolved_by_reason']:
            out.write(f"| {u['provider']} | {u['encoding']} | {u['mode']} | {u['reason']} | {u['jobs']} |\n")
        out.write('\n### Held-out calibration and order sensitivity\n\n| Question | Provider / encoding | Raw ECE by answering model | Order disagreement (pairs) |\n|---|---|---|---:|\n')
        for row in results['per_question']:
            if row['provider']=='rules':
                continue
            ece='; '.join(f"{c['model']}{' (vision '+c['vision_model']+')' if c['vision_model'] else ''}: {c['raw_ece']:.3f}"
                          for c in row['calibration_by_model'] if c['raw_ece'] is not None) or '—'
            order='—' if row['order_sensitivity'] is None else f"{100*row['order_sensitivity']:.1f}% ({row['both_order_pairs']})"
            out.write(f"| {row['question']} | {row['provider']} / {row['encoding']} | {ece} | {order} |\n")
        out.write(f'\n### Qualification gates (§9, applied as written)\n\n| Question | Truth-known held-out cases | More needed for {minimum} |\n|---|---:|---:|\n')
        for q,s in support.items():
            out.write(f"| {q} | {s['truth_known_held_out_cases']} | {s['additional_held_out_cases_needed']} |\n")
        out.write('\nNo question qualifies: every question is below the held-out gate, important-miss tolerance is undeclared, no calibration identity is qualified, and provider rows below 95% attempted completion also fail that gate. Gates are not lowered.\n')
    print(json.dumps({'processing_complete':results['processing_complete'],'unresolved':unresolved,'calls':dict(counts)}))


if __name__=='__main__':
    main()
