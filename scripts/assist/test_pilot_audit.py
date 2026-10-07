import copy
import json
from pathlib import Path
import tempfile
import unittest

from corpus import digest, encoded, freeze, put, verify
from pilot_audit import audit, merge, render
from pilot_score import semantic
from PIL import Image
from score import assertion_correct, task_evidence
from stage2 import REVISION, report
from receipts import source_fact


class PilotAuditTests(unittest.TestCase):
    def test_literal_protocol_and_geometry_are_not_relaxed(self):
        # Fully invented small pixel witness: not a corpus answer.
        case = dict(label='Café TEST', dimensions=[20,20], target=[2,2,8,4], task='check_ui')
        truth = dict(rendered_witness=dict(label_pixels=[[42,3]], before_label_pixels=[[42,3]],
            label_complete=True, before_label_complete=True))
        observation = dict(image_role='single', statement='text:Café TEST',
            geometry=dict(type='box',pixels=[2,2,8,4]))
        self.assertTrue(assertion_correct(observation,truth,case,Path('.')))
        self.assertTrue(task_evidence([observation],case,truth,Path('.')))
        for literal in ('Cafe\u0301 TEST', 'Café\nTEST', 'Café  TEST', 'café TEST', 'Café TEST '):
            altered = dict(observation,statement='text:'+literal)
            self.assertFalse(assertion_correct(altered,truth,case,Path('.')))
        presence = dict(observation,statement='presence:present')
        self.assertTrue(assertion_correct(presence,truth,case,Path('.')))
        self.assertFalse(task_evidence([presence],case,truth,Path('.')))
        altered = copy.deepcopy(observation);altered['geometry']['pixels'][0] += 1e-9
        self.assertFalse(assertion_correct(altered,truth,case,Path('.')))
        absent = dict(observation,statement='presence:absent')
        empty = dict(rendered_witness=dict(label_pixels=[],before_label_pixels=[],
            label_complete=False,before_label_complete=False))
        self.assertTrue(assertion_correct(absent,empty,case,Path('.')))
        altered = copy.deepcopy(absent);altered['geometry']['pixels'][2] -= 1e-9
        self.assertFalse(assertion_correct(altered,empty,case,Path('.')))

    def test_exact_order_abstention_and_exclusion_protocol_incompatibility(self):
        case=dict(dimensions=[20,20],target=[2,2,8,4],label='Invented',
            task='audit_mask',before='before.png',after='after.png',
            exclusions=[dict(id='excluded-A',runs=[[42,3]])])
        truth=dict(expected_outcome='not_observed',important=False,
            rendered_witness=dict(label_pixels=[],before_label_pixels=[],
                label_complete=False,before_label_complete=False))
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp)
            for name in ('before.png','after.png'):
                Image.new('RGB',(20,20),'white').save(directory/name)
            obs=dict(image_role='after',statement='appearance:unchanged',
                geometry=dict(type='box',pixels=[2,2,8,4]),
                evidence_refs=['after:R0'],uncertainty=0,visibility='visible')
            self.assertTrue(assertion_correct(obs,truth,case,directory))
            self.assertFalse(task_evidence([obs],case,truth,directory))
            cited=dict(obs,evidence_refs=['excluded-A'])
            self.assertTrue(task_evidence([cited],case,truth,directory))
            answer=dict(outcome='not_observed',observations=[obs])
            varied=copy.deepcopy(answer);varied['observations'][0]['uncertainty']=.01
            self.assertFalse(semantic({False:[answer,varied]},case,truth,directory)['complete'])
            abstain=dict(outcome='unverifiable',observations=[])
            result=semantic({False:[abstain,abstain]},case,truth,directory)
            self.assertTrue(result['complete']);self.assertTrue(result['abstention'])
            self.assertFalse(semantic({False:[abstain,None]},case,truth,directory)['abstention'])

    def test_union_preserves_failed_attempt_and_refuses_payload_mismatch(self):
        with tempfile.TemporaryDirectory() as temp:
            base=Path(temp);corpus=base/'corpus'
            manifest=freeze(corpus,5,4406,REVISION,'jev-1.13.0',True)
            rows,_=report(manifest,corpus)
            canonical=base/'requests.json';put(canonical,rows)
            local=base/'local.json'
            put(local,[dict(root=c['root_id'],arm=a,
                outcome='observed' if source_fact(c) and c['complete'] else 'unverifiable',
                source_only=source_fact(c) and c['complete']) for c in manifest['cases'] if c['split']=='heldout'
                for a in ('rules','cascade') if a=='rules' or not c['complete'] or source_fact(c)])
            first=base/'first';second=base/'second'
            for d in (first,second):
                d.mkdir();(d/'ledger').mkdir()
            outcomes=[dict(index=i,root=r['root'],code='skipped_after_failure') for i,r in enumerate(rows)]
            outcomes[0].update(code='transport_failure',execution_id='failure')
            failure=dict(id='failure',request_hash=digest(encoded(rows[0]['payload'])),outcome='incomplete',
                reserved_nano_usd=100,actual_nano_usd=None,usage=dict(campaign_root_index=0))
            put(first/'smoke.json',dict(root_outcomes=outcomes))
            put(first/'ledger/campaign.json',dict(money=dict(campaign_identity=dict(requests_hash=digest(canonical.read_bytes())),receipts=[failure])))
            subset=base/'subset.json';put(subset,[rows[0]])
            # A synthetic abstention exercises a successful retry without truth copying.
            answer=dict(request_hash=json.loads(rows[0]['payload']['messages'][1]['content'][0]['text'])['request_hash'],
                outcome='unverifiable',observations=[])
            put(second/'answer-0.json',answer)
            put(second/'response-0.json',dict(choices=[dict(message=dict(content=json.dumps(answer)))]))
            success=dict(id='success',request_hash=failure['request_hash'],outcome='completed',
                reserved_nano_usd=100,actual_nano_usd=7,usage=dict(campaign_root_index=0))
            put(second/'receipt-0.json',dict(execution_id='success',request_hash=success['request_hash'],
                response_hash=digest((second/'response-0.json').read_bytes()),elapsed_ms=9))
            put(second/'smoke.json',dict(root_outcomes=[dict(index=0,root=rows[0]['root'],code='completed',execution_id='success')]))
            state=dict(money=dict(campaign_identity=dict(requests_hash=digest(subset.read_bytes())),receipts=[success]))
            put(second/'ledger/campaign.json',state)
            campaigns=[(canonical,first),(subset,second)]
            result=audit(corpus,canonical,campaigns,local,None)
            self.assertEqual(result['campaign']['attempted_requests'],2)
            self.assertEqual(result['campaign']['known_cost_nano_usd'],7)
            self.assertEqual(result['campaign']['charged_with_unknown_reservations_nano_usd'],107)
            self.assertEqual(result['campaign']['unknown_cost_receipts'],1)
            self.assertEqual(result['campaign']['completed_answers'],1)
            self.assertLessEqual(result['mechanics']['unavailable_root_arms'],
                result['mechanics']['scheduled_root_arm_denominator'])
            self.assertNotIn('root_results',result)
            self.assertNotIn('root_arm_results',result['mechanics'])
            self.assertNotIn('saccade-pilot-union-',encoded(result).decode())
            self.assertTrue(render(result).startswith('# UNQUALIFIED PILOT'))
            with self.assertRaisesRegex(ValueError,'money binding|duplicate terminal'):
                merge(rows,campaigns+[(subset,second)])
            modified=copy.deepcopy(rows[0]);modified['payload']['temperature']=.3
            put(subset,[modified]);state['money']['campaign_identity']['requests_hash']=digest(subset.read_bytes())
            put(second/'ledger/campaign.json',state)
            with self.assertRaisesRegex(ValueError,'payload hash mismatch'):
                merge(rows,campaigns)


if __name__=='__main__':
    unittest.main()
