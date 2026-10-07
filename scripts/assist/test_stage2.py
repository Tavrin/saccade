import copy
from pathlib import Path
import tempfile
import unittest
from corpus import freeze, verify
from stage2 import payload, priced, report, schedule, collect, REVISION

class Stage2Tests(unittest.TestCase):
    def test_frozen_schedule_binding_and_closed_budget(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp)/'corpus'
            m=freeze(directory,5,4406,REVISION,'jev-1.13.0',True)
            self.assertEqual(verify(directory)[0]['epoch'],'wave4-constructed/2')
            rows,plan=report(m,directory)
            self.assertFalse(plan['qualified'])
            self.assertEqual(len(rows),72)
            self.assertEqual(plan['requests'],sum(t['calls'] for t in plan['per_arm_workload']))
            self.assertEqual(len({r['root'] for r in rows}),len(rows))
            c=next(c for c in m['cases'] if c['split']=='heldout' and c['counterfactual'])
            self.assertNotEqual(payload(c,'single',False)['messages'],payload(c,'single',True)['messages'])
            self.assertEqual(len(schedule(c,'two_gemini')),2)
            c=next(c for c in m['cases'] if c['source'])
            self.assertEqual(schedule(c,'cascade'),[])
            self.assertEqual(len(schedule(c,'two_gemini')),1)
            c=next(c for c in m['cases'] if not c['complete'])
            self.assertEqual(schedule(c,'two_gemini'),[])
            self.assertRaises(ValueError,freeze,directory,5,4406,REVISION,'jev-1.13.0',True)
            inflated=copy.deepcopy(m)
            inflated['cases']=m['cases']*10
            self.assertRaises(ValueError,report,inflated,directory)
            import json
            # Select a real two-order root; missing output stays unavailable.
            root=next(r['root'].rsplit(':',1)[0] for r in rows if r['root'].endswith(':ab') and ':two_gemini:root:' in r['root'])
            pair=[r for r in rows if r['root'].rsplit(':',1)[0]==root]
            out=Path(tmp)/'results';out.mkdir()
            self.assertTrue(collect(pair,out)['root_arm_results'][0]['missing'])
            for index,r in enumerate(pair):
                h=json.loads(r['payload']['messages'][1]['content'][0]['text'])['request_hash']
                (out/f'answer-{index}.json').write_text(json.dumps(dict(request_hash=h,outcome='unverifiable',observations=[])))
            self.assertFalse(collect(pair,out)['root_arm_results'][0]['order_disagreement'])
            a=json.loads((out/'answer-1.json').read_text());a['outcome']='not_observed'
            (out/'answer-1.json').write_text(json.dumps(a))
            self.assertTrue(collect(pair,out)['root_arm_results'][0]['order_disagreement'])
            a['request_hash']='sha256:'+'0'*64;(out/'answer-1.json').write_text(json.dumps(a))
            self.assertRaises(ValueError,collect,pair,out)
            for r in rows:
                text=str(r['payload'])
                for forbidden in ('generator_seed','expected_outcome','category','oracle'):
                    self.assertNotIn(forbidden,text)

if __name__=='__main__': unittest.main()
