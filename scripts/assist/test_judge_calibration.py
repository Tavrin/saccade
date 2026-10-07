"""Constructed scalar adapter regressions; synthetic answers establish no provider trust."""
import copy
import unittest
from judge_calibration import adapt, digest
from judge_show import render
import judge_gate


def fixture():
    manifest=dict(schema='saccade-motion-degradations.v1',entries=[dict(id='p0-frozen-s0')])
    requests=[];scores=[]
    for model in ('google/gemini-3.8-flash','openai/gpt-5.4-mini'):
        for repeat in range(2):
            for order in range(2):
                root=f'{model}-{repeat}-{order}'
                clips=[dict(source=source,view_id='p0-frozen-s0:'+label,kind='motion',sample_id=f'sample-{repeat}',fps=120,max_edge=128,sheet={}) for source,label in [('a','positive'),('b','negative')]]
                if order:clips.reverse()
                data=dict(request_hash=root,packet=dict(schema='saccade-video-judge.v1',rubric=dict(anchors=list(range(10))),clips=clips))
                import json
                requests.append(dict(root=root,model=model,revision='absent',payload=dict(messages=[{},dict(content=[dict(text=json.dumps(data))])],response_format=dict(json_schema=dict(name='projection')))))
                for c in clips:
                    scores.append(dict(root=root,model=model,revision='absent',provider='fixture-'+model,returned_model=model,returned_revision='absent',request_hash=root,**{k:c[k] for k in ('source','view_id','kind','sample_id')},
                                       order=order,outcome='scored',score=9 if c['source']=='a' else 2,preferred_source='a',replay=False,cues=[],criteria=[],forbidden=[]))
    schedule=dict(schema='saccade-judge-calibration-schedule.v1',manifest_sha256=digest(manifest),items=[dict(id='p0-frozen-s0',requests=requests)])
    return manifest,schedule,scores


class Calibration(unittest.TestCase):
    def test_two_arms_reduce_repeats_without_inflating_sources(self):
        manifest,schedule,rows=fixture();result=adapt(manifest,schedule,rows)
        self.assertEqual(result['unavailable'],[])
        self.assertEqual(len(result['scores']),2)
        self.assertEqual([(s['positive'],s['negative']) for s in result['scores']],[(9,2),(9,2)])
        self.assertEqual(result['trusted_for'],[])
        self.assertEqual({p['model'] for p in result['bindings'].values()},{'google/gemini-3.8-flash','openai/gpt-5.4-mini'})
        near=copy.deepcopy(rows);near[0]['score']=8.5
        self.assertFalse(adapt(manifest,schedule,near)['unavailable'])

    def test_incomplete_replay_inconsistent_and_identity_are_withheld(self):
        manifest,schedule,rows=fixture()
        for field,value in [('replay',True),('returned_revision','wrong'),('preferred_source','b'),('score',None),('provider','mixed')]:
            wrong=copy.deepcopy(rows);wrong[0][field]=value
            self.assertTrue(adapt(manifest,schedule,wrong)['unavailable'],field)
        self.assertTrue(adapt(manifest,schedule,rows[:-1])['unavailable'])
        self.assertTrue(adapt(manifest,schedule,rows+rows[:1])['unavailable'])
        for field,value in [('request_hash','other'),('sample_id','extra'),('order',1),('source','invented')]:
            wrong=copy.deepcopy(rows);wrong[0][field]=value
            with self.assertRaises(ValueError):adapt(manifest,schedule,wrong)
        wrong=copy.deepcopy(rows);wrong[0]['root']='extra'
        with self.assertRaises(ValueError):adapt(manifest,schedule,wrong)
        manifest['entries'].append(dict(id='missing'))
        schedule['manifest_sha256']=digest(manifest)
        self.assertTrue(adapt(manifest,schedule,rows)['unavailable'])

    def test_binding_changes_with_rubric_transform_or_kind(self):
        manifest,schedule,rows=fixture();original=adapt(manifest,schedule,rows)['bindings']
        import json
        for request in schedule['items'][0]['requests']:
            data=json.loads(request['payload']['messages'][1]['content'][0]['text'])
            data['packet']['rubric']['anchors']=['changed']*10
            request['payload']['messages'][1]['content'][0]['text']=json.dumps(data)
        self.assertNotEqual(original,adapt(manifest,schedule,rows)['bindings'])

    def test_required_gate_cases_and_local_show_escaping(self):
        cases=judge_gate.selftest()
        for name in ('oracle-perfect','oracle-near-perfect','oracle-wrong-wrong-score','oracle-inconsistent-order'):
            self.assertTrue(cases['cases'][name])
        manifest,schedule,rows=fixture()
        self.assertIn('Judge smoke hand check',render(schedule['items'][0]['requests']))


if __name__=='__main__':unittest.main()
