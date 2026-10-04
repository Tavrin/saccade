"""Acceptance tests use private temporary copies; they never add human labels to the pilot."""
import copy
import http.client
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('pilot',Path(__file__).with_name('pilot.py'))
pilot=importlib.util.module_from_spec(spec)
spec.loader.exec_module(pilot)


class PilotContract(unittest.TestCase):
    def setUp(self):
        if not (pilot.HERE/'.local/mapping.json').exists() or not (pilot.HERE/'.local/bin/pilot_offline').exists():
            self.skipTest('superseded preparation fixture is absent from this checkout')
        self.temp=tempfile.TemporaryDirectory()
        self.root=Path(self.temp.name)
        self.original=pilot.HERE
        shutil.copy(self.original/'rubrics.json',self.root/'rubrics.json')
        shutil.copy(self.original/'preparation-pilot.toml',self.root/'moss-pilot.toml')
        (self.root/'.local/bin').mkdir(parents=True)
        shutil.copy(self.original/'.local/mapping.json',self.root/'.local/mapping.json')
        shutil.copy(self.original/'.local/bin/pilot_offline',self.root/'.local/bin/pilot_offline')
        self.patches=[patch.object(pilot,'HERE',self.root),patch.object(pilot,'LOCAL',self.root/'.local'),patch.object(pilot,'MANIFEST',self.root/'moss-pilot.toml')]
        for p in self.patches:p.start()
        self.m,self.mapping=pilot.validate()
        self.case=self.mapping['cases'][0]

    def tearDown(self):
        for p in reversed(self.patches):p.stop()
        self.temp.cleanup()

    def records(self,c=None):
        c=c or self.case
        exposure=dict(model='no',mapping='no',implementation='no')
        initial=dict(case_id=c['case_id'],stage='structured',answers={},exposure=exposure,
                     vision=dict(supports_disposition='undetermined',unresolved_visual_fact='test uncertainty',provisional_route='human_directly',uncertainty='test only'))
        choices={'triage.route.v1':'needs_eyes','vision.route.v1':'human_directly','perf.interpret.v1':'collect_more_evidence','capture.disposition.v1':'needs_human','intent.match.v1':'insufficient_intent'}
        final=dict(case_id=c['case_id'],stage='final',answers={q:choices[q] for q in c['questions']},exposure=exposure,
                   vision=dict(requires_pixels='undetermined',minimum_scope='none',requires_human=True,necessary_visual_fact='test unavailable fact',route='human_directly'))
        return initial,final

    def label_case(self,c,phase='initial'):
        initial,final=self.records(c)
        pilot.record_label(c,initial,phase,'test-session')
        time.sleep(.003)
        pilot.record_label(c,final,phase,'test-session')

    def test_freeze_and_caps_exclude_denied(self):
        b=pilot.plan(self.mapping['cases'])
        self.assertEqual(b,self.m['budget'])
        self.assertEqual(b['global_maximum'],sum(b['maximum'].values()))
        self.assertEqual(b['authorized_calls'],0)
        denied=copy.deepcopy(self.mapping['cases'])
        for c in denied:c['egress_allowed']=False
        self.assertEqual(pilot.plan(denied)['global_initial'],0)

    def test_shared_extraction_has_its_own_counts_and_gemini_cap(self):
        c=next(c for c in self.mapping['cases'] if c['egress_allowed'])
        p=next(p for p in pilot.read(Path(c['folder'])/'packet-index.json')['presentations'] if p['view']==0 and p['order']=='ab')
        row=dict(case_id=c['case_id'],question='visual-observation/1',encoding='shared_observations',
                 model=pilot.MODELS[0],vision_model=None,order='ab',outcome='unavailable',attempts=self.m['attempts_caps']['gemini']+1,
                 manifest_sha256=pilot.file_hash(pilot.MANIFEST),presentation_identity=p['identity'])
        with self.assertRaisesRegex(ValueError,'R11 evaluation caps'):pilot.check_attempt_caps([row],self.m)
        row['attempts']=1
        pilot.check_attempt_caps([row],self.m)
        key=(c['case_id'],row['question'],row['encoding'],row['model'],None,'ab','pinned_provider')
        scheduled=set()
        results=pilot.replay_extractions(self.mapping['cases'],{key:row},self.m,scheduled)
        stats=next(s for s in results if s['mode']=='pinned_provider' and s['model']==row['model'])
        self.assertEqual(stats['attempts'],1)
        self.assertEqual(stats['unavailable'],1)
        self.assertEqual(stats['quality_support'],0)
        self.assertIsNone(stats['conditional_accuracy'])
        self.assertEqual(len(scheduled),len(self.mapping['cases'])*len(pilot.MODELS)*2*len(self.m['modes']))
        row['presentation_identity']='stale'
        with self.assertRaisesRegex(ValueError,'frozen presentation'):pilot.replay_extractions(self.mapping['cases'],{key:row},self.m,set())

    def test_mapping_and_rubric_tampering_fail(self):
        changed=copy.deepcopy(self.mapping)
        changed['cases'][0]['split']='other'
        pilot.save(pilot.LOCAL/'mapping.json',changed)
        with self.assertRaisesRegex(ValueError,'mapping changed'):pilot.validate()
        pilot.save(pilot.LOCAL/'mapping.json',self.mapping)
        (pilot.HERE/'rubrics.json').write_text('{}')
        with self.assertRaisesRegex(ValueError,'rubric changed'):pilot.validate()

    def test_staging_canonical_binding_and_duplicate_label(self):
        initial,final=self.records()
        with self.assertRaisesRegex(ValueError,'structured-first'):pilot.record_label(self.case,final,'initial','test')
        self.label_case(self.case)
        old=pilot.labels('initial')
        self.assertEqual(len(old),2)
        canonical=pilot.read(pilot.LOCAL/'canonical-labels/initial'/f"{self.case['case_id'][7:]}.json")
        self.assertEqual(canonical['labels']['schema'],'saccade-labels.v2')
        self.assertTrue(all(r['eligible'] for r in canonical['eligibility']))
        with self.assertRaisesRegex(ValueError,'initial labels'):pilot.record_label(self.case,final,'initial','test')
        self.assertEqual(pilot.labels('initial'),old)

    def test_vision_inconsistent_label_is_rejected_by_r9(self):
        a,b=self.records()
        pilot.record_label(self.case,a,'initial','test')
        b['answers']['vision.route.v1']='text_sufficient'
        b['vision'].update(route='text_sufficient',requires_human=False)
        with self.assertRaisesRegex(ValueError,'route disagrees'):pilot.record_label(self.case,b,'initial','test')
        self.assertEqual(len(pilot.labels('initial')),1)

    def test_retest_delays_and_both_labels_survive(self):
        held=[c for c in self.mapping['cases'] if c['case_id'] in self.m['test_retest']['case_ids']]
        with self.assertRaisesRegex(ValueError,'incomplete'):pilot.retest_ids(self.m,[],int(time.time()*1000))
        for c in held:self.label_case(c)
        with self.assertRaisesRegex(ValueError,'seven days'):pilot.retest_ids(self.m,pilot.labels('initial'),int(time.time()*1000))
        first=pilot.labels('initial')
        future=time.time()+8*86400
        with patch.object(pilot.time,'time',return_value=future):
            c=held[0];a,b=self.records(c)
            pilot.record_label(c,a,'retest','second')
        with patch.object(pilot.time,'time',return_value=future+1):pilot.record_label(c,b,'retest','second')
        self.assertEqual(pilot.labels('initial'),first)
        canonical=pilot.read(pilot.LOCAL/'canonical-labels/retest'/f"{c['case_id'][7:]}.json")
        self.assertEqual(sum(bool(r['test_retest_of']) for r in canonical['records']['items']),len(c['questions']))
        result=pilot.retest_metrics(first,pilot.labels('retest'))
        self.assertEqual(result['agreement'],1)
        self.assertGreaterEqual(result['minimum_actual_delay_days'],7)

    def test_local_http_stage_token_and_blinding(self):
        server=pilot.http.server.HTTPServer(('127.0.0.1',0),pilot.Workbench)
        server.cases=[self.case];server.phase='initial';server.seed=120
        server.token='test-token';server.region_seen=set();server.full_seen=set()
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        conn=http.client.HTTPConnection('127.0.0.1',server.server_port)
        def request(method,path,body=None):
            conn.request(method,path,json.dumps(body) if body else None,{'Content-Type':'application/json'})
            response=conn.getresponse();data=response.read();return response.status,data
        try:
            self.assertEqual(request('GET','/bad/case')[0],400)
            code,body=request('GET','/test-token/case');self.assertEqual(code,200)
            data=json.loads(body);self.assertEqual(data['stage'],'structured')
            self.assertNotIn('previous_labels',data);self.assertNotIn('model_answers',data)
            self.assertEqual(request('GET','/test-token/image/0/full-frame-P1.png')[0],400)
            a,b=self.records()
            self.assertEqual(request('POST','/test-token/label',a)[0],200)
            code,body=request('GET','/test-token/case');self.assertEqual(json.loads(body)['stage'],'final')
            self.assertEqual(request('GET','/test-token/image/0/full-frame-P1.png')[0],200)
            time.sleep(.003)
            self.assertEqual(request('POST','/test-token/label',b)[0],200)
            self.assertTrue(json.loads(request('GET','/test-token/case')[1])['complete'])
        finally:
            conn.close();server.shutdown();server.server_close();thread.join()

    def test_denied_and_stale_recorded_answers_fail(self):
        denied=copy.deepcopy(self.mapping)
        c=denied['cases'][0]
        c['egress_allowed']=False
        q=c['questions'][0]
        row=dict(case_id=c['case_id'],question=q,encoding='direct',model='jev-latest',vision_model=None,order='structured',outcome='answered',answer='abstain',manifest_sha256=pilot.file_hash(pilot.MANIFEST),request_sha256=pilot.file_hash(Path(c['folder'])/'packet'/f'{q}.json'))
        f=pilot.LOCAL/'answers.json';pilot.save(f,[row])
        with patch.object(pilot,'validate',return_value=(self.m,denied)):
            with self.assertRaisesRegex(ValueError,'denied content'):pilot.replay(f,pilot.LOCAL/'bin/pilot_offline')
        c=next(c for c in self.mapping['cases'] if c['egress_allowed'])
        row.update(case_id=c['case_id'],request_sha256='stale')
        pilot.save(f,[row])
        with self.assertRaisesRegex(ValueError,'frozen request'):pilot.replay(f,pilot.LOCAL/'bin/pilot_offline')


if __name__=='__main__':unittest.main()
