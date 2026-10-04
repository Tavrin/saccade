"""R12 invariants: independence, budgets, blind observation attribution and frozen truth."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import corpus
import corpus_live
import pilot


class CorpusContracts(unittest.TestCase):
    def case(self, n, family='local_regression', origin='constructed'):
        return {'case_id': pilot.digest(['case', n]), 'group_sha256': pilot.digest(['scene', n//3, family]),
                'split': 'held_out', 'truth_source': 'construction', 'origin': origin,
                'questions': pilot.QUESTIONS, 'truth': dict(zip(pilot.QUESTIONS,
                ['suspected_regression', 'inspect_regions', 'collect_more_evidence', 'continue_review', 'insufficient_intent']))}

    def test_all_views_of_scene_family_stay_in_one_split(self):
        cases = [self.case(n) for n in range(60)]
        counts = corpus.split_cases(cases)
        self.assertEqual(sum(counts.values()), 60)
        groups = {}
        for c in cases:
            self.assertEqual(groups.setdefault(c['group_sha256'], c['split']), c['split'])

    def test_budget_is_actual_batches_and_unknown_truth_can_measure_availability(self):
        cases = [self.case(n) for n in range(60)]
        for c in cases[:20]: c['split'] = 'development'
        for c in cases[20:40]: c['split'] = 'calibration'
        cases.append(dict(self.case(100), truth_source='unknown'))
        jobs, initial = corpus.schedule(cases)
        self.assertEqual(initial, {'jev': 205, 'gemini': 98})
        self.assertTrue(any(j['case_id'] == cases[-1]['case_id'] for j in jobs))
        self.assertEqual(len({j['job_id'] for j in jobs}), len(jobs))
        self.assertTrue(all(j['questions'] for j in jobs))

    def test_over_budget_plan_fails_before_dispatch(self):
        with self.assertRaisesRegex(ValueError, 'exceeds hard caps'):
            corpus.schedule([self.case(n) for n in range(100)])

    def test_payload_does_not_contain_constructed_truth_or_private_mapping(self):
        request = {'question': {'id': 'triage.route.v1', 'text': 'catalog', 'answers': ['needs_eyes', 'abstain']}}
        body = corpus_live.jev_payload([request])
        self.assertEqual(set(body['questions']), {'q0'})
        self.assertNotIn('truth', json.dumps(body))
        self.assertNotIn('mapping', body['state'])

    def test_no_priors_control_has_distinct_policy_and_observation_content_hash(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); response = root/'response.json'; response.write_text('{}')
            case = dict(self.case(1), folder=str(root))
            vision = {'actual_model': 'gemini-3.7-flash', 'revision': 'gemini-3.7-flash-001',
                      'response_file': str(response), 'response_sha256': pilot.file_hash(response),
                      'presentation_identity': pilot.digest('transform'), 'fallback_outcome_identity': pilot.digest('outcome'),
                      'observations': [{'observation': 'visible change', 'baseline_slot': 'P2', 'capture_slot': 'P1'}]}
            with patch.object(corpus, 'call', return_value='{"requests":[]}'):
                corpus_live.enriched({'helper':'offline'}, {'gemini_chain':pilot.MODELS}, case,
                    {'encoding':'enriched', 'questions':[]}, vision, root/'a')
                corpus_live.enriched({'helper':'offline'}, {'gemini_chain':pilot.MODELS}, case,
                    {'encoding':'enriched_without_priors', 'questions':[]}, vision, root/'b')
            a = corpus.load(root/'a/enrichment.json'); b = corpus.load(root/'b/enrichment.json')
            self.assertNotEqual(a['policy'], b['policy'])
            self.assertEqual(a['response_sha256'], vision['response_sha256'])
            self.assertEqual(a['context']['extractor']['model'], vision['actual_model'])

    def test_observation_citations_and_presentation_are_checked(self):
        with tempfile.TemporaryDirectory() as tmp:
            response=Path(tmp)/'response.json'; response.write_text('{}')
            job={'order':'ba','questions':['vision.route.v1']}; identity=pilot.digest('p')
            answer={'presentation_identity':identity,'answers':{'vision.route.v1':'inspect_regions'},
                    'observations':[{'region_id':'full_frame','observation':'change','evidence_refs':['f/P1','f/P2'],'uncertainty':'none'}]}
            def envelope():
                return {'modelVersion':'actual-001','candidates':[{'finishReason':'STOP','content':{'parts':[{'text':json.dumps(answer)}]}}]}
            with patch.object(corpus_live,'presentation',return_value=(identity,[{'label':'f/P1','slot':'P1'},{'label':'f/P2','slot':'P2'}])):
                result=corpus_live.decode_gemini({},job,envelope(),identity,'actual',response,[])
                self.assertEqual(result['observations'][0]['baseline_slot'],'P2')
                answer['observations'][0]['evidence_refs']=['f/P1']
                with self.assertRaisesRegex(ValueError,'both anonymous slots'):
                    corpus_live.decode_gemini({},job,envelope(),identity,'actual',response,[])
                answer['presentation_identity']='stale'
                with self.assertRaisesRegex(ValueError,'stale Gemini'):
                    corpus_live.decode_gemini({},job,envelope(),identity,'actual',response,[])

    def test_elapsed_resume_preserves_durable_receipts_without_dispatch(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); job={'job_id':pilot.digest('job'),'case_id':'case','provider':'gemini',
                               'mode':'production_policy','encoding':'gemini_alone','order':'ab'}
            manifest={'jobs':[job],'elapsed_window_ms':1}
            seal=pilot.file_hash(corpus.MANIFEST)
            result={'job':job,'case_id':'case','manifest_sha256':seal,'outcome':'unavailable','attempts':1}
            corpus.save(root/'run-state.json',{'started_ms':0,'manifest_sha256':seal})
            corpus.save(root/'live'/job['job_id'][7:]/'result.json',result)
            with patch.object(corpus,'validate',return_value=(manifest,{'cases':[{'case_id':'case'}]})), \
                 patch.object(corpus,'amendment',return_value=None), \
                 patch.object(corpus_live,'execute_job') as dispatch, patch.object(corpus_live,'score'):
                corpus_live.run({'root':str(root)})
                dispatch.assert_not_called()
            self.assertEqual(corpus.load(root/'live-results.json'),[result])

    def test_miss_bounds_group_orders_and_exclude_missing_responses(self):
        row={'case_id':'one','truth':'suspected_regression','outcome':'answered','critical_error':True}
        unavailable=dict(row,case_id='two',outcome='unavailable',critical_error=False)
        unknown=dict(row,case_id='three',truth=None)
        result=corpus_live.miss_bounds([row,row,unavailable,unknown])
        self.assertEqual((result['cases'],result['misses'],result['rate']),(1,1,1))
        self.assertGreater(result['ci95'][0],0)

    def test_jev_token_limit_batches_split_without_changing_question_requests(self):
        # Recorded refusal shape: four closed questions over one case's structured evidence,
        # 62,229 encoded bytes, answered 400 {"detail":{"error_type":"max_tokens_exceeded"}}.
        def request(q, filler):
            facts = [{'name': f'fact-{i}', 'value': {'availability': 'available',
                      'value': {'type': 'text', 'value': pilot.digest([q, i])*2}}} for i in range(filler)]
            return {'question': {'id': q, 'text': 'Choose a route.', 'answers': ['a', 'b', 'abstain']},
                    'constraints': [], 'evidence': {'facts': facts, 'observations': []}}
        questions = pilot.QUESTIONS[:4]
        large = [request(q, 65) for q in questions]
        size = len(pilot.encoded(corpus_live.jev_payload(large)))
        self.assertTrue(62000 < size < 66000, size)
        batches = corpus_live.jev_chunks(large, set())
        self.assertGreater(len(batches), 1)
        self.assertEqual([i for b in batches for i in b], [0, 1, 2, 3])
        for b in batches:
            payload = corpus_live.jev_payload([large[i] for i in b])
            self.assertLessEqual(len(pilot.encoded(payload)), corpus_live.JEV_SPLIT_BYTES)
            self.assertEqual(payload['state']['requests'], [large[i] for i in b])
        small = [request(q, 10) for q in questions]
        self.assertEqual(corpus_live.jev_chunks(small, set()), [[0, 1, 2, 3]])
        self.assertEqual(corpus_live.jev_chunks(small, {'0,1,2,3'}), [[0, 1], [2, 3]])
        with tempfile.TemporaryDirectory() as tmp:
            body = Path(tmp)/'error-0.body'
            body.write_bytes(b'{"detail":{"error_type":"max_tokens_exceeded"}}')
            exchange = {'failure': 'rejected', 'failed_attempts': [{'status': 400, 'reservation': 'r', 'error_body': str(body)}]}
            self.assertEqual(corpus_live.error_type(exchange), 'max_tokens_exceeded')
            self.assertEqual(corpus_live.failure_categories([exchange]), {'400 max_tokens_exceeded': 1})


if __name__ == '__main__':
    unittest.main()
