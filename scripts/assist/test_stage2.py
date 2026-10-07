import copy
from pathlib import Path
import tempfile
import unittest
from corpus import freeze, verify
from stage2 import payload, priced, report, schedule, collect, REVISION, REQUEST_POLICY, REASONING_BUDGETS, reservation

class Stage2Tests(unittest.TestCase):
    def test_stage2_replan_interleaves_pilot_and_budget_bounded_larger_schedule(self):
        import json
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp)/'pilot'
            manifest=freeze(directory,15,4406,REVISION,'jev-1.13.0',True)
            rows,plan=report(manifest,directory)
            self.assertEqual(len(rows),216)
            self.assertEqual(rows,report(verify(directory)[0],directory)[0])
            self.assertEqual(plan['reservation_nano_usd'],4621723500)
            self.assertFalse(plan['budget_bounded'])
            self.assertTrue(plan['full_reservation_fits'])
            workloads={c['root_id']:c['workload'] for c in manifest['cases']}
            def group(row):
                root,arm,_,_=row['root'].rsplit(':',3)
                return arm,workloads[root]
            active={group(r) for r in rows}
            # Every arm/workload receives one request before any receives two.
            self.assertEqual(len(active),12)
            self.assertEqual({group(r) for r in rows[:12]},active)
            self.assertEqual({group(r) for r in rows[12:24]},active)
            larger=Path(tmp)/'larger'
            manifest=freeze(larger,60,4406,REVISION,'jev-1.13.0',True)
            self.assertRaises(ValueError,report,manifest,larger)
            rows,plan=report(verify(larger)[0],larger,True)
            self.assertEqual(len(rows),864)
            self.assertTrue(plan['budget_bounded'])
            self.assertFalse(plan['full_reservation_fits'])
            self.assertGreater(plan['reservation_nano_usd'],plan['allowance_nano_usd'])
            self.assertFalse(plan['authorized'])
            self.assertFalse(plan['qualified'])
            workloads={c['root_id']:c['workload'] for c in manifest['cases']}
            self.assertEqual({group(r) for r in rows[:12]},active)

    def test_reasoning_policy_is_pinned_in_both_regenerated_plans(self):
        import json
        with tempfile.TemporaryDirectory() as tmp:
            for target, count, bounded in [(15,216,False),(60,864,True)]:
                directory=Path(tmp)/str(target)
                manifest=freeze(directory,target,4406,REVISION,'jev-1.13.0',True)
                rows,plan=report(manifest,directory,bounded)
                self.assertEqual(plan['requests'],count)
                self.assertEqual(plan['request_policy'],REQUEST_POLICY)
                self.assertEqual(plan['reasoning_budgets'],REASONING_BUDGETS)
                self.assertEqual(plan['aggregate_output_limit'],4096)
                dimensions={c['root_id']:c['dimensions'] for c in manifest['cases']}
                total=0
                for row in rows:
                    request=row['payload']
                    task=json.loads(request['messages'][1]['content'][0]['text'])['task']
                    self.assertEqual(request['reasoning'],dict(max_tokens=REASONING_BUDGETS[task]))
                    self.assertLessEqual(request['reasoning']['max_tokens'],1024)
                    total+=reservation(request,dimensions[row['root'].rsplit(':',3)[0]])
                self.assertEqual(total,plan['reservation_nano_usd'])
                self.assertFalse(plan['qualified'])

    def test_budget_and_deadline_stops_remain_unavailable_in_full_denominator(self):
        import json
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp)/'corpus'
            manifest=freeze(directory,5,4406,REVISION,'jev-1.13.0',True)
            rows,_=report(manifest,directory)
            out=Path(tmp)/'results';out.mkdir()
            for code in ('not_run_budget','not_run_deadline','truncated_output'):
                outcomes=[dict(index=i,root=r['root'],code=code) for i,r in enumerate(rows)]
                (out/'smoke.json').write_text(json.dumps(dict(root_outcomes=outcomes)))
                # A stale answer cannot turn an explicitly unrun root into success.
                h=json.loads(rows[0]['payload']['messages'][1]['content'][0]['text'])['request_hash']
                (out/'answer-0.json').write_text(json.dumps(dict(request_hash=h,outcome='observed',observations=[])))
                result=collect(rows,out)
                denominator=len({(r['root'].rsplit(':',3)[0],r['root'].rsplit(':',3)[1]) for r in rows})
                self.assertEqual(result['scheduled_requests'],len(rows))
                self.assertEqual(result['scheduled_root_arm_denominator'],denominator)
                self.assertEqual(result['unavailable_root_arms'],denominator)
                self.assertEqual(result['unavailable_request_codes'],{code:len(rows)})
                self.assertTrue(all(r['missing'] and r['outcome']=='unverifiable' for r in result['root_arm_results']))
                self.assertFalse(result['qualified'])
            outcomes.pop()
            (out/'smoke.json').write_text(json.dumps(dict(root_outcomes=outcomes)))
            self.assertRaises(ValueError,collect,rows,out)

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
