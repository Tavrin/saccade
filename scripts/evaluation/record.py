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
    results['processing_complete']=len(jobs)==results['scheduled_jobs']
    results['incomplete']=not results['processing_complete'] or unresolved>0
    results['unresolved_provider_jobs']=unresolved
    results['provider_failures']=[{'provider':p,'model':m,'category':c,'attempts':n} for (p,m,c),n in sorted(failures.items())]
    amendment=read(public/'resume-amendment.json')
    used=amendment['used_before_fix']
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
    results['held_out_support']=support
    results['actual_response_revisions']=[{'provider':p,'revision':v} for p,v in sorted({(r['job']['provider'],r['revision']) for r in jobs if r['outcome']=='answered' and r['job']['provider']!='rules'})]
    results['elapsed_ms']=round(time.time()*1000-state['started_ms'])
    results['record_source_sha256']=digest(Path(__file__))
    results['limitations'] = list(dict.fromkeys(results['limitations']+[
        'Harmful-miss bounds cover frozen catalog classes and deterministic violations; they do not establish unmeasured semantic safety.',
        'Latency records active batch dispatch handling, excluding queued time and end-to-end vision-plus-Jev extraction.',
        'Provider error bodies were not retained before the transport fix; earlier failures are known by HTTP status only. Later error classes come from privately retained bodies.'
    ]))
    (public/'RESULTS.json').write_text(json.dumps(results,indent=2,sort_keys=True)+'\n')
    marker='<!-- dispatch-completion-record -->'
    original=(public/'RESULTS.md').read_text().split(marker)[0].rstrip()+'\n'
    (public/'RESULTS.md').write_text(original)
    with (public/'RESULTS.md').open('a') as out:
        out.write('\n'+marker+'\n')
        out.write(f"\nAll-job processing complete: {str(results['processing_complete']).lower()}. Unresolved provider jobs: {unresolved}. Evaluation incomplete: {str(results['incomplete']).lower()}.\n")
        out.write('\n| Provider | Requested model | Failure category | Attempts |\n|---|---|---|---:|\n')
        for (p,m,c),n in sorted(failures.items()):out.write(f'| {p} | {m} | {c} | {n} |\n')
        out.write('\nLatency excludes queueing and full pipeline extraction. Harmful-miss bounds use the frozen catalog; they do not establish semantic safety. Missing responses and unresolved pairs remain excluded from committed-answer accuracy.\n')
        resume=results['resume']
        out.write(f"\n## Resume after the transport fix\n\nAmendment `{resume['amendment_sha256']}` (epoch `{resume['epoch']}`) replaces only transport identities and opens a new elapsed window; corpus, splits, rubrics and plan are unchanged. "
                  f"Calls this run: Jev {resume['calls_this_run']['jev']}/{resume['caps_this_run']['jev']}, Gemini {resume['calls_this_run']['gemini']}/{resume['caps_this_run']['gemini']} (Jev includes one diagnosis replay of a refused request).\n")
        if resume['provider_quota_violations']:
            out.write('\n| Gemini model | Provider quota | Limit |\n|---|---|---:|\n')
            for q in resume['provider_quota_violations']:
                out.write(f"| {q['model']} | {q['quota_id']} | {q['quota_value']} |\n")
            out.write('\nThe 429s are a daily per-model free-tier quota, not a request rate; pacing cannot recover them. The transport honours the provider retry delay and stops spending attempts. '
                      'Every Gemini chain model hit the same limit, so the chain recommendation is unchanged: newest-first, with fallback as the only way past one model\'s daily quota on this tier. No model is consistently missing.\n')
        remaining={p:resume['caps_this_run'][p]-resume['calls_this_run'][p] for p in used}
        out.write(f"\nRemainder: {unresolved} provider jobs. Gemini jobs wait for the provider's daily reset; Jev enriched jobs wait on their Gemini observations. Unused caps: Jev {remaining['jev']}, Gemini {remaining['gemini']}. "
                  'Resuming after the reset continues from the ledger; raising the quota is a paid-tier (spending) decision.\n')
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
