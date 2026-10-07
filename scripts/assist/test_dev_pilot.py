"""Explicit split selection and independent fresh draw regressions."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from corpus import freeze, verify
from stage2 import REVISION, report

class DevPilotTests(unittest.TestCase):
    def test_missing_split_refuses(self):
        result=subprocess.run([sys.executable,str(Path(__file__).with_name('plan.py')),'--corpus','unused','--stage2'],capture_output=True,text=True)
        self.assertEqual(result.returncode,2)
        self.assertIn('--split',result.stderr)

    def test_splits_and_fresh_draw(self):
        with tempfile.TemporaryDirectory() as temp:
            old=freeze(Path(temp)/'old',5,4406,REVISION,'jev-1.13.0',True)
            fresh=freeze(Path(temp)/'fresh',5,975031,REVISION,'jev-1.13.0',True,True)
            self.assertEqual(verify(Path(temp)/'fresh')[0]['epoch'],'g12-fresh-heldout/1')
            for split in ('development','calibration','heldout'):
                rows,plan=report(old,Path(temp)/'old',True,split,750_000_000)
                roots={c['root_id'] for c in old['cases'] if c['split']==split}
                self.assertEqual({r['root'].rsplit(':',3)[0] for r in rows}, {c['root_id'] for c in old['cases'] if c['split']==split and c['complete']})
                self.assertTrue(all(r['root'].rsplit(':',3)[0] in roots for r in rows))
                self.assertEqual(plan['split'],split)
                self.assertEqual(plan['allowance_nano_usd'],750_000_000)
            for field in ('family','generator_seed','root_id'):
                self.assertTrue({c[field] for c in old['cases']}.isdisjoint({c[field] for c in fresh['cases'] if c['split']=='heldout'}))

    def test_dev_cal_scoring_keeps_split_denominators(self):
        from corpus import put, digest
        from receipts import source_fact
        from pilot_score import evaluate
        with tempfile.TemporaryDirectory() as temp:
            base=Path(temp);corpus=base/'corpus'
            manifest=freeze(corpus,5,4406,REVISION,'jev-1.13.0',True)
            for split in ('development','calibration'):
                live=base/split; (live/'ledger').mkdir(parents=True)
                rows,_=report(manifest,corpus,True,split)
                requests=live/'requests.json';put(requests,rows)
                put(live/'smoke.json',dict(root_outcomes=[dict(index=i,root=r['root'],code='not_run_budget') for i,r in enumerate(rows)]))
                put(live/'ledger/campaign.json',dict(money=dict(campaign_identity=dict(requests_hash=digest(requests.read_bytes())),receipts=[])))
                local=live/'local.json'
                put(local,[dict(root=c['root_id'],arm=a,outcome='observed' if source_fact(c) and c['complete'] else 'unverifiable',source_only=source_fact(c) and c['complete']) for c in manifest['cases'] if c['split']==split for a in ('rules','cascade') if a=='rules' or not c['complete'] or source_fact(c)])
                result=evaluate(corpus,requests,live,local,split=split)
                self.assertEqual(result['campaign']['scheduled_requests'],len(rows))
                self.assertEqual(result['campaign']['completed_answers'],0)
                self.assertFalse(result['qualified'])
                self.assertRaisesRegex(ValueError,'schedule incomplete',evaluate,corpus,requests,live,local,split='heldout')
