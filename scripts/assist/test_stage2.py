import copy
from pathlib import Path
import tempfile
import unittest
from corpus import freeze, verify
from stage2 import payload, priced, report, schedule, collect, REVISION, REQUEST_POLICY, REASONING_BUDGETS, reservation, response_format, ANSWER_SCHEMA_PATH, project_schema, PROJECTION_POLICY, PROJECTED_SCHEMA_NAME

class Stage2Tests(unittest.TestCase):
    def test_round2_prompt_epoch_convention_examples_and_same_slot_citations(self):
        import json,re
        from stage2 import INSTRUCTION,PROMPT_POLICY,PROMPT_EPOCH
        self.assertEqual(PROMPT_EPOCH,'g12-pilot/2')
        self.assertEqual(PROMPT_POLICY,'assist-openrouter-geometry-citations/2')
        self.assertIn('box [x,y,width,height]',INSTRUCTION)
        for rule in ('width>0','height>0','x+width<=1','y+height<=1','evidence_refs may only cite regions of the same slot'):
            self.assertIn(rule,INSTRUCTION)
        examples=re.findall(r'(check_ui|explain|audit_mask): (\{.*?\})(?:\. |\.$)',INSTRUCTION)
        self.assertEqual({task for task,_ in examples},{'check_ui','explain','audit_mask'})
        for task,raw in examples:
            obs=json.loads(raw);coords=obs['geometry']['pixels']
            self.assertTrue(all(0<=x<=1 for x in coords))
            if obs['geometry']['type']=='box':
                x,y,w,h=coords;self.assertTrue(w>0 and h>0 and x+w<=1 and y+h<=1)
            self.assertTrue(all(ref.startswith(obs['slot']+':') for ref in obs['evidence_refs']))
        from corpus import digest
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp)/'pilot';m=freeze(d,15,4406,REVISION,'jev-1.13.0',True)
            rows,p=report(m,d)
            self.assertEqual(p['prompt_epoch'],PROMPT_EPOCH)
            self.assertEqual(p['prompt_policy'],PROMPT_POLICY)
            self.assertEqual(p['prompt_hash'],digest(INSTRUCTION.encode()))
            self.assertEqual(m['campaign'],'g12-stage2/2')
            self.assertEqual(m['policy']['version'],'constructed-assist/4')
            self.assertTrue(all(row['payload']['messages'][0]['content']==INSTRUCTION for row in rows))

    def test_stage2_replan_interleaves_pilot_and_budget_bounded_larger_schedule(self):
        import json
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp)/'pilot'
            manifest=freeze(directory,15,4406,REVISION,'jev-1.13.0',True)
            rows,plan=report(manifest,directory)
            self.assertEqual(len(rows),216)
            self.assertEqual(rows,report(verify(directory)[0],directory)[0])
            self.assertLessEqual(plan['reservation_nano_usd'],5_000_000_000)
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

    def test_strict_schema_and_policy_are_pinned_in_both_regenerated_plans(self):
        import json
        from corpus import digest, encoded
        expected=response_format()
        self.assertEqual(expected,dict(type='json_schema',json_schema=dict(name=PROJECTED_SCHEMA_NAME,strict=True,
            schema=project_schema(json.loads(ANSWER_SCHEMA_PATH.read_text())))))
        self.assertEqual(REQUEST_POLICY,'assist-openrouter-provider-schema/1')
        def closed(schema):
            if schema.get('type')=='object':
                self.assertIs(schema['additionalProperties'],False)
                self.assertEqual(set(schema['required']),set(schema['properties']))
                for child in schema['properties'].values(): closed(child)
            if 'items' in schema: closed(schema['items'])
            for child in schema.get('anyOf',[]): closed(child)
        closed(expected['json_schema']['schema'])
        geometry=expected['json_schema']['schema']['properties']['observations']['items']['properties']['geometry']
        for variant in geometry['anyOf']:
            self.assertEqual(set(variant['properties']),{'type','pixels'})
            self.assertNotIn('minItems',variant['properties']['pixels'])
            self.assertNotIn('maxItems',variant['properties']['pixels'])
        full=json.loads(ANSWER_SCHEMA_PATH.read_text())
        def compare(full, projected):
            if isinstance(full, dict):
                self.assertEqual(set(projected),set(full)-{'minItems','maxItems'})
                return sum(key in ('minItems','maxItems') for key in full)+sum(compare(child,projected[key]) for key,child in full.items() if key not in ('minItems','maxItems'))
            if isinstance(full, list):
                self.assertEqual(len(full),len(projected))
                return sum(compare(a,b) for a,b in zip(full,projected))
            self.assertEqual(full,projected)
            return 0
        self.assertEqual(compare(full,expected['json_schema']['schema']),6)
        self.assertEqual(full['properties']['observations']['maxItems'],64)
        with tempfile.TemporaryDirectory() as tmp:
            for target,count,bounded in [(15,216,False),(60,864,True)]:
                directory=Path(tmp)/str(target)
                manifest=freeze(directory,target,4406,REVISION,'jev-1.13.0',True)
                rows,plan=report(manifest,directory,bounded)
                self.assertEqual(plan['requests'],count)
                self.assertEqual(plan['request_policy'],REQUEST_POLICY)
                self.assertEqual(plan['response_format_hash'],digest(encoded(expected)))
                self.assertEqual(plan['schema_projection'],PROJECTION_POLICY)
                self.assertEqual(plan['full_answer_schema_hash'],digest(encoded(json.loads(ANSWER_SCHEMA_PATH.read_text()))))
                for row in rows: self.assertEqual(row['payload']['response_format'],expected)
                for invalid in (None,dict(type='json_object'),dict(type='json_schema',json_schema=dict(expected['json_schema'],strict=False))):
                    drift=copy.deepcopy(rows[0]['payload'])
                    drift['response_format']=invalid
                    self.assertRaisesRegex(ValueError,'pinned strict answer schema',reservation,drift,[20,20])

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

    def test_invalid_answers_count_in_arm_workload_and_root_denominators(self):
        import json
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp)/'corpus'
            manifest=freeze(directory,5,4406,REVISION,'jev-1.13.0',True)
            rows,_=report(manifest,directory)
            out=Path(tmp)/'results';out.mkdir()
            outcomes=[]
            for i,row in enumerate(rows):
                outcomes.append(dict(index=i,root=row['root'],code='invalid_answer',answer_reason='citation_identity'))
                # Even a stale valid-looking answer cannot override invalid status.
                h=json.loads(row['payload']['messages'][1]['content'][0]['text'])['request_hash']
                (out/f'answer-{i}.json').write_text(json.dumps(dict(request_hash=h,outcome='unverifiable',observations=[])))
            (out/'smoke.json').write_text(json.dumps(dict(root_outcomes=outcomes)))
            result=collect(rows,out,manifest)
            denominator=len({tuple(r['root'].rsplit(':',3)[:2]) for r in rows})
            self.assertEqual(result['scheduled_root_arm_denominator'],denominator)
            self.assertEqual(result['invalid_root_arms'],denominator)
            self.assertEqual(result['invalid_answers'],len(rows))
            self.assertEqual(result['unavailable_root_arms'],0)
            self.assertTrue(all(r['outcome']=='invalid_answer' and not r['missing'] for r in result['root_arm_results']))
            self.assertEqual(len(result['invalid_answer_rates_per_arm_workload']),12)
            for rate in result['invalid_answer_rates_per_arm_workload']:
                self.assertEqual(rate['invalid_answer_rate'],1)
                self.assertEqual(rate['invalid_root_arm_rate'],1)
                self.assertEqual(rate['reason_breakdown'],{'citation_identity':rate['scheduled_requests']})
            # Preserve the full denominator after a valve or infrastructure stop.
            outcomes[0]['answer_reason']='geometry_bounds'
            for o in outcomes[1:]:
                o['code']='not_run_answer_safety_valve';o.pop('answer_reason')
            (out/'smoke.json').write_text(json.dumps(dict(root_outcomes=outcomes)))
            result=collect(rows,out,manifest)
            self.assertEqual(result['scheduled_root_arm_denominator'],denominator)
            self.assertEqual(result['invalid_root_arms'],1)
            self.assertEqual(result['invalid_answers'],1)
            self.assertEqual(sum(m['scheduled_root_arm_denominator'] for m in result['invalid_answer_rates_per_arm_workload']),denominator)
            rate=next(m for m in result['invalid_answer_rates_per_arm_workload'] if m['invalid_answers'])
            self.assertEqual(rate['reason_breakdown'],{'geometry_bounds':1})
            self.assertEqual(rate['invalid_answer_scheduled_rate'],1/rate['scheduled_requests'])
            self.assertFalse(result['qualified'])
            outcomes[0]['answer_reason']='invented'
            (out/'smoke.json').write_text(json.dumps(dict(root_outcomes=outcomes)))
            self.assertRaisesRegex(ValueError,'reason code',collect,rows,out,manifest)

    def test_frozen_schedule_binding_and_closed_budget(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory=Path(tmp)/'corpus'
            m=freeze(directory,5,4406,REVISION,'jev-1.13.0',True)
            self.assertEqual(verify(directory)[0]['epoch'],'wave4-constructed/4')
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
