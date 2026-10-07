import copy
import json
from pathlib import Path
import tempfile
import unittest
from corpus import freeze, digest, encoded, put
from stage2 import report, REVISION
from pilot_score import evaluate, normalize, semantic, summarize, bounds, markdown


class PilotScoreTests(unittest.TestCase):
    def test_exact_one_sided_support_and_no_empty_sample_claim(self):
        self.assertIsNone(bounds(0, 0)['upper95'])
        self.assertAlmostEqual(bounds(0, 9)['upper95'], 1-.05**(1/9))
        self.assertAlmostEqual(bounds(9, 9)['lower95'], .05**(1/9))
        self.assertGreater(bounds(0, 9)['upper95'], .28)

    def test_normalization_preserves_roles_geometry_and_order_identity(self):
        case = dict(dimensions=[200,100])
        answer = dict(request_hash='bound', outcome='observed', observations=[dict(slot='P1',
            geometry=dict(type='box',pixels=[.1,.2,.3,.4]), evidence_refs=['P1:R0'])])
        value = normalize(answer,case,'ba')
        self.assertEqual(value['observations'][0]['image_role'],'after')
        self.assertEqual(value['observations'][0]['geometry']['pixels'],[20,20,60,40])
        self.assertEqual(value['observations'][0]['evidence_refs'],['after:R0'])
        self.assertEqual(answer['observations'][0]['slot'],'P1')
        # Exact normalized comparisons do not silently accept differing orders.
        self.assertFalse(semantic({False:[value,None]},case,{},Path('.'))['complete'])
        other=copy.deepcopy(value);other['outcome']='not_observed'
        self.assertFalse(semantic({False:[value,other]},case,{},Path('.'))['complete'])

    def test_partial_unknown_transport_is_unavailable_and_keeps_full_charge(self):
        from receipts import source_fact
        with tempfile.TemporaryDirectory() as temp:
            base=Path(temp); corpus=base/'corpus'; live=base/'live'; live.mkdir(); (live/'ledger').mkdir()
            manifest=freeze(corpus,5,4406,REVISION,'jev-1.13.0',True)
            rows,_=report(manifest,corpus)
            requests=base/'requests.json';put(requests,rows)
            local=base/'local-results.json'
            put(local,[dict(root=c['root_id'],arm=a,outcome='observed' if source_fact(c) and c['complete'] else 'unverifiable',
                source_only=source_fact(c) and c['complete']) for c in manifest['cases'] if c['split']=='heldout'
                for a in ('rules','cascade') if a=='rules' or not c['complete'] or source_fact(c)])
            outcomes=[dict(index=i,root=r['root'],code='skipped_after_failure') for i,r in enumerate(rows)]
            outcomes[0].update(code='not_run_transport_failure',execution_id='attempt')
            receipt=dict(id='attempt',request_hash=digest(encoded(rows[0]['payload'])),outcome='settled_conservatively',
                actual_nano_usd=100,reserved_nano_usd=100,usage=dict(campaign_root_index=0))
            state=dict(money=dict(campaign_identity=dict(requests_hash=digest(requests.read_bytes())),receipts=[receipt]))
            put(live/'ledger/campaign.json',state);put(live/'smoke.json',dict(root_outcomes=outcomes))
            # A stale successful answer cannot override explicit unavailable transport.
            put(live/'answer-0.json',dict(outcome='observed',request_hash='stale'))
            result=evaluate(corpus,requests,live,local)
            self.assertFalse(result['qualified'])
            self.assertEqual(result['campaign']['known_cost_nano_usd'],100)
            self.assertEqual(result['campaign']['completed_answers'],0)
            self.assertEqual(result['campaign']['unavailable_request_codes']['not_run_transport_failure'],1)
            for split in ('development','calibration'):
                for arms in result['splits'][split].values():
                    for m in arms.values():
                        self.assertEqual(m['attempted_requests'],0)
                        self.assertIsNone(m['precision'])
            receipt['actual_nano_usd']=None;receipt['outcome']='incomplete'
            put(live/'ledger/campaign.json',state)
            result=evaluate(corpus,requests,live,local)
            self.assertEqual(result['campaign']['known_cost_nano_usd'],0)
            self.assertEqual(result['campaign']['unknown_cost_receipts'],1)
            self.assertEqual(result['campaign']['charged_with_unknown_reservations_nano_usd'],100)
            self.assertTrue(markdown(result).startswith('# PARTIAL, UNQUALIFIED PILOT'))
            # Bind one real completed-answer fixture through response, provenance,
            # request and money; drift must refuse rather than score forged success.
            from corpus import verify
            _, oracle = verify(corpus)
            by_root={c['root_id']:c for c in manifest['cases']}
            index=next(i for i,r in enumerate(rows) if r['root'].rsplit(':',3)[1]=='single_gemini' and
                by_root[r['root'].rsplit(':',3)[0]]['workload']=='check_ui' and
                oracle[r['root'].rsplit(':',3)[0]]['category']=='control')
            row=rows[index];root=row['root'].rsplit(':',3)[0];case=by_root[root]
            answer=dict(request_hash=json.loads(row['payload']['messages'][1]['content'][0]['text'])['request_hash'],
                outcome=oracle[root]['expected_outcome'],observations=[dict(slot='P1',kind='text',statement='text:'+case['label'],
                    geometry=dict(type='box',pixels=[0,0,1,1]),evidence_refs=['P1:R0'],uncertainty=0,visibility='visible')])
            put(live/f'answer-{index}.json',answer)
            put(live/f'response-{index}.json',dict(choices=[dict(message=dict(content=json.dumps(answer)))]))
            response_hash=digest((live/f'response-{index}.json').read_bytes())
            request_hash=digest(encoded(row['payload']))
            provenance=dict(execution_id='successful',request_hash=request_hash,response_hash=response_hash,elapsed_ms=42)
            put(live/f'receipt-{index}.json',provenance)
            state['money']['receipts'].append(dict(id='successful',request_hash=request_hash,actual_nano_usd=17,
                reserved_nano_usd=100,outcome='completed',usage=dict(campaign_root_index=index)))
            outcomes[index].update(code='completed',execution_id='successful')
            put(live/'smoke.json',dict(root_outcomes=outcomes));put(live/'ledger/campaign.json',state)
            result=evaluate(corpus,requests,live,local)
            metric=result['splits']['heldout']['check_ui']['single_gemini']
            self.assertEqual(metric['correct_roots'],1);self.assertEqual(metric['precision'],1)
            self.assertEqual(metric['latency_p50_ms'],42)
            self.assertEqual(result['campaign']['charged_with_unknown_reservations_nano_usd'],117)
            provenance['response_hash']='wrong';put(live/f'receipt-{index}.json',provenance)
            with self.assertRaisesRegex(ValueError,'provenance drift'):
                evaluate(corpus,requests,live,local)
            provenance['response_hash']=response_hash;put(live/f'receipt-{index}.json',provenance)
            # Truncated schedules may not increase availability by dropping missing orders.
            put(requests,rows[:-1]);outcomes.pop();put(live/'smoke.json',dict(root_outcomes=outcomes))
            state['money']['campaign_identity']['requests_hash']=digest(requests.read_bytes());put(live/'ledger/campaign.json',state)
            with self.assertRaisesRegex(ValueError,'schedule incomplete'):
                evaluate(corpus,requests,live,local)

    def test_completed_root_quality_uses_frozen_oracle_and_counterfactual_denominator(self):
        from corpus import verify
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp)/'corpus'
            manifest=freeze(directory,5,4406,REVISION,'jev-1.13.0',True)
            _,oracle=verify(directory)
            case=next(c for c in manifest['cases'] if c['split']=='heldout' and c['workload']=='check_ui'
                      and oracle[c['case_id']]['category']=='control')
            truth=oracle[case['case_id']]
            answer=normalize(dict(request_hash='fixture',outcome=truth['expected_outcome'],observations=[dict(
                slot='P1',kind='text',statement='text:'+case['label'], geometry=dict(type='box',pixels=[0,0,1,1]),
                evidence_refs=['P1:R0'],uncertainty=0,visibility='visible')]),case,'single')
            item=semantic({False:[answer]},case,truth,directory)
            self.assertTrue(item['correct']);self.assertTrue(item['complete'])
            answer['observations'][0]['statement']='text:unsupported invented text'
            self.assertFalse(semantic({False:[answer]},case,truth,directory)['correct'])
            childcase=next(c for c in manifest['cases'] if c['split']=='heldout' and c.get('counterfactual'))
            self.assertFalse(semantic({False:[answer]},childcase,oracle[childcase['case_id']],directory)['complete'])


if __name__=='__main__':
    unittest.main()
