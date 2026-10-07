"""Synthetic regressions for the development audit and paid-run proof gate."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import corpus
from corpus import freeze, verify, encoded, digest
from dev_audit import development_oracle
from dev_policy import exclusion_rect, normalize, payload, assertion_correct, task_evidence
from pilot_score import semantic
from scorer_selftest import proof
from stage2 import REVISION


class ScorerSelftestTests(unittest.TestCase):
    def test_oracle_perfect_and_all_wrong_cases_use_real_scorer(self):
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp)
            freeze(directory,5,4406,REVISION,'jev-1.13.0',True)
            manifest,oracle=verify(directory)
            manifest['cases']=[c for c in manifest['cases'] if c['split']=='development']
            result=proof(manifest,oracle,directory)
            self.assertEqual(result['status'],'PASS',result['failures'])
            self.assertEqual(len(result['rows']),96)
            for row in result['rows']:
                if row['variant']=='perfect':
                    self.assertEqual((row['precision'],row['important_recall'],row['false_reassurance']),(1,1,0))
                else:
                    self.assertEqual(row['correct_roots'],0)
                    self.assertEqual(row['flagged_roots'],row['roots'])
            with patch('score.task_evidence',return_value=False):
                broken=proof(manifest,oracle,directory)
                self.assertEqual(broken['status'],'FAIL')
                self.assertTrue(any(w=='explain' and v=='perfect' for w,a,v in broken['failures']))
            real_assertion=__import__('score').assertion_correct
            def inverted(observation,*args):
                result=real_assertion(observation,*args)
                return not result if observation['statement'].startswith('appearance:') else result
            with patch('score.assertion_correct',side_effect=inverted):
                inverted_result=proof(manifest,oracle,directory)
                self.assertEqual(inverted_result['status'],'FAIL')
                self.assertTrue(any(w=='explain' and v=='perfect' for w,a,v in inverted_result['failures']))

    def test_output_fit_gate_fails_for_an_insufficient_budget(self):
        from dev_policy import output_fit
        request=dict(max_tokens=4096,reasoning=dict(max_tokens=1024))
        answer=dict(request_hash='bound', observations=['x'*2000])
        self.assertFalse(output_fit(answer,request)['passed'])
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp)
            freeze(directory,5,4406,REVISION,'jev-1.13.0',True)
            manifest,oracle=verify(directory)
            manifest['cases']=[c for c in manifest['cases'] if c['split']=='development']
            from dev_policy import payload as actual
            def short(*args,**kwargs):
                value=actual(*args,**kwargs);value['max_tokens']=100;return value
            with patch('dev_policy.payload',side_effect=short):
                result=proof(manifest,oracle,directory)
            self.assertEqual(result['status'],'FAIL')
            self.assertTrue(any(a=='output_budget' for w,a,v in result['failures']))

    def test_epoch4_names_candidate_and_keeps_per_image_examples(self):
        from dev_policy import INSTRUCTION, PROMPT_EPOCH, PROMPT_POLICY
        from scorer_selftest import perfect
        self.assertEqual(PROMPT_EPOCH, 'g12-pilot/4')
        self.assertEqual(PROMPT_POLICY, 'assist-openrouter-task-evidence/4')
        self.assertIn('appearance:unchanged', INSTRUCTION)
        self.assertIn('Report BOTH on candidate_slot', INSTRUCTION)
        self.assertIn('Other statements stay per image/slot', INSTRUCTION)
        self.assertIn('check_ui: slot P1, kind text', INSTRUCTION)
        self.assertIn('explain: slot P2, kind appearance', INSTRUCTION)
        self.assertIn('audit_mask: slot P2, kind appearance', INSTRUCTION)
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            freeze(directory, 5, 4406, REVISION, 'jev-1.13.0', True)
            manifest, oracle = verify(directory)
            cases = [c for c in manifest['cases'] if c['split'] == 'development'
                     and c['complete'] and c['task'] != 'check_ui']
            self.assertTrue(cases)
            for case in cases:
                answers = []
                for order, slot in (('ab', 'P2'), ('ba', 'P1')):
                    data = json.loads(payload(case, order, False, directory)['messages'][1]['content'][0]['text'])
                    self.assertEqual(data['candidate_slot'], slot)
                    self.assertEqual(data['prompt_epoch'], PROMPT_EPOCH)
                    answer = perfect(case, oracle[case['root_id']], directory, order)
                    for observation in answer['observations']:
                        self.assertEqual(observation['slot'], slot)
                        self.assertTrue(all(ref.startswith(slot + ':') for ref in observation['evidence_refs']))
                    answers.append(normalize(answer, case, order))
                self.assertTrue(semantic({False: answers}, case, oracle[case['root_id']], directory, True)['correct'])

    def test_unselected_oracle_values_are_never_decoded(self):
        document=dict(schema='synthetic',cases={'dev':{'public':'development'},'held':{'secret':'DO_NOT_DECODE'}})
        raw=encoded(document)+b'\n'
        manifest=dict(oracle_hash=digest(encoded(document)),cases=[dict(root_id='dev',split='development'),dict(root_id='held',split='heldout')])
        loads=json.loads; decoded=[]
        def watch(value,*args,**kwargs):
            decoded.append(value)
            return loads(value,*args,**kwargs)
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'oracle.json';path.write_bytes(raw)
            with patch('dev_audit.json.loads',side_effect=watch): result=development_oracle(path,manifest)
        self.assertEqual(result['cases']['dev'],{'public':'development'})
        self.assertIsNone(result['cases']['held'])
        self.assertFalse(any('DO_NOT_DECODE' in str(v) for v in decoded))

    def test_exclusion_mapping_requires_explicit_region_and_full_coverage(self):
        case=dict(task='audit_mask',dimensions=[20,20],target=[2,2,8,4],label='Synthetic',before='b.png',after='a.png',exclusions=[dict(id='exclude',dimensions=[20,20],runs=[[42,3]])])
        truth=dict(expected_outcome='not_observed',important=False,rendered_witness=dict(label_pixels=[],before_label_pixels=[],label_complete=False,before_label_complete=False))
        from PIL import Image
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp)
            for name in ('b.png','a.png'): Image.new('RGB',(20,20),'white').save(directory/name)
            obs=dict(slot='P2',kind='appearance',statement='appearance:unchanged',geometry=dict(type='box',pixels=[0,0,1,1]),visibility='visible',uncertainty=0,evidence_refs=['P2:R0'])
            answer=dict(request_hash='synthetic',outcome='not_observed',observations=[obs])
            value=normalize(answer,case,'ab')
            self.assertFalse(semantic({False:[value]},case,truth,directory,True)['correct'])
            obs['evidence_refs'].append('P2:R1');value=normalize(answer,case,'ab')
            self.assertTrue(semantic({False:[value]},case,truth,directory,True)['correct'])
            obs['geometry']['pixels']=[0,0,.1,.1];value=normalize(answer,case,'ab')
            self.assertFalse(semantic({False:[value]},case,truth,directory,True)['correct'])
        self.assertEqual(exclusion_rect(dict(dimensions=[20,20],runs=[[305,50]])),[0,15,20,3])

    def test_overlap_requires_bound_source_both_citations_and_both_nodes(self):
        case=dict(condition=dict(kind='non_overlap',first='a',second='b'),source='source.json')
        obs=dict(image_role='single',statement='overlap:separate',geometry=dict(type='box',pixels=[0,0,20,20]),evidence_refs=['single:R0','single:R1'])
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp);path=directory/'source.json'
            path.write_text(json.dumps(dict(nodes=[dict(id='a',bounds=[1,1,3,3]),dict(id='b',bounds=[1,10,3,3])])))
            self.assertTrue(assertion_correct(obs,{},case,directory))
            self.assertTrue(task_evidence([obs],case,{},directory))
            self.assertFalse(assertion_correct(dict(obs,evidence_refs=['single:R0']),{},case,directory))
            self.assertFalse(assertion_correct(dict(obs,statement='overlap:overlap'),{},case,directory))
            self.assertFalse(assertion_correct(dict(obs,geometry=dict(type='box',pixels=[0,0,5,5])),{},case,directory))


if __name__=='__main__':unittest.main()
