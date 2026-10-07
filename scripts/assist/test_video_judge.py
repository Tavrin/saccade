"""Offline generated media and complete calibration-denominator regressions."""
import hashlib
import json
import pathlib
import tempfile
import unittest
import video_corpus
import video_scores


class VideoJudge(unittest.TestCase):
    def test_procedural_corpus_is_reproducible_and_content_addressed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            video_corpus.generate(root/'one')
            video_corpus.generate(root/'two')
            for file in (root/'one').rglob('*'):
                if file.is_file():
                    self.assertEqual(file.read_bytes(), (root/'two'/file.relative_to(root/'one')).read_bytes())
            frames = json.loads((root/'one/continuous/frame-map.json').read_text())['frames']
            self.assertEqual([frame['timestamp_s'] for frame in frames], [0.0, 1.0])
            for frame in frames:
                self.assertEqual(frame['sha256'], hashlib.sha256((root/'one/continuous'/frame['file']).read_bytes()).hexdigest())

    def test_calibration_preserves_invalid_unrun_and_abstention_slots(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            rows = []
            for index in range(3):
                packet = dict(clips=[dict(source='source-a'), dict(source='source-b')])
                rows.append(dict(root=f'root-{index}', model='fixture-model', revision='fixture-revision',
                    payload=dict(messages=[{}, dict(content=[dict(text=json.dumps(dict(packet=packet, request_hash='request')))])])))
            requests = root/'requests.json'
            requests.write_text(json.dumps(rows))
            (root/'ledger').mkdir()
            (root/'ledger/campaign.json').write_text(json.dumps(dict(money=dict(campaign_identity=dict(
                requests_hash='sha256:'+hashlib.sha256(requests.read_bytes()).hexdigest())))))
            outcomes = [dict(root=f'root-{i}', code=code, execution_id=f'execution-{i}') for i, code in enumerate(['completed', 'invalid_answer', 'not_run_budget'])]
            (root/'smoke.json').write_text(json.dumps(dict(root_outcomes=outcomes)))
            answer = dict(request_hash='request', outcome='abstain', scores=[dict(slot=slot,score=None,cues=[]) for slot in ['A','B']])
            for i in range(3):
                (root/f'answer-{i}.json').write_text(json.dumps(answer))
            response = dict(provider='FixtureProvider',choices=[dict(message=dict(content=json.dumps(answer)))])
            response_file = root/'response-0.json'
            response_file.write_text(json.dumps(response))
            (root/'receipt-0.json').write_text(json.dumps(dict(response_hash='sha256:'+hashlib.sha256(response_file.read_bytes()).hexdigest(),
                execution_id='execution-0', requested_model='fixture-model', returned_model='fixture-model', returned_revision='absent', sampling_settings=rows[0]['payload'])))
            scores = list(video_scores.export(requests,root))
            self.assertEqual(len(scores),6)
            self.assertEqual([s['outcome'] for s in scores], ['abstain']*2+['invalid_answer']*2+['not_run_budget']*2)
            self.assertTrue(all(s['score'] is None and s['trusted_for']==[] for s in scores))
            requests.write_text(requests.read_text()+' ')
            with self.assertRaises(ValueError):
                list(video_scores.export(requests,root))
