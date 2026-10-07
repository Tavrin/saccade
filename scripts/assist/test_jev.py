"""Offline native Jev parsing, frozen scorer and budget regressions."""
import copy
import unittest
import jev_eval as j


class JevTests(unittest.TestCase):
    def test_required_selftest(self):
        result = j.self_test()
        self.assertEqual(result['status'], 'pass')
        self.assertGreaterEqual(len(result['adversarial']), 20)
        for row in result['table'].values():
            self.assertEqual(row['oracle_perfect'], 1)
            self.assertEqual(row['corrupted'], 0)

    def test_partial_two_arm_results(self):
        cases, oracle = j.corpus()
        rows, receipts = [], []
        for w, choices in j.WORKLOADS.items():
            for arm in ('compact_jev', 'enriched_jev'):
                for reverse in (False, True):
                    p = j.payload(cases[0], w, reverse, arm)
                    truth = oracle[cases[0]['root']]['expected'][w]
                    response = None if reverse else dict(model=j.MODEL,
                        answers=dict(q=dict(type='choice', choice=truth, confidence=1.,
                            probabilities={x: float(x == truth) for x in choices})),
                        usage=dict(input_tokens=30, output_tokens=5))
                    rows.append(dict(root=cases[0]['root'], workload=w, arm=arm,
                        order='reverse' if reverse else 'forward', payload=p, response=response))
                    r = j.receipt(p, response, str(len(rows)))
                    if reverse:
                        r.update(reserved_nano_usd=0, tariff_nano_usd=0, charged_nano_usd=0)
                    receipts.append(r)
        result = j.score(rows, oracle, receipts)
        self.assertEqual(len(result['task_arm_table']), 10)
        for metrics in result['task_arm_table']:
            self.assertEqual(metrics['valid_answers'], 1)
            self.assertEqual(metrics['variant_accuracy'], 1)
            self.assertEqual(metrics['variant_availability'], .5)
            self.assertEqual(metrics['availability'], 0)
            self.assertEqual(metrics['accuracy'], 0)
            self.assertEqual(metrics['option_disagreement'], 0)
        self.assertEqual(result['accounted_attempts'], 10)
        self.assertFalse(result['qualified'])

    def test_rounded_probability_sum_is_normalised(self):
        choices = j.WORKLOADS['cause']
        response = dict(model=j.MODEL, usage=dict(input_tokens=569, output_tokens=79),
            answers=dict(q=dict(type='choice', choice='global_tone', confidence=.63,
                probabilities=dict(global_tone=.68, local_structure=.16, misaligned=.01,
                    noise=.02, config_mismatch=0., ambiguous=.03, unknown=.09))))
        self.assertAlmostEqual(sum(response['answers']['q']['probabilities'].values()), .99)
        metadata = {}
        self.assertEqual(j.parse_native(response, choices, metadata=metadata), 'global_tone')
        self.assertAlmostEqual(metadata['original_sum'], .99)
        self.assertTrue(metadata['renormalised'])
        self.assertAlmostEqual(sum(metadata['probabilities'].values()), 1.)

    def test_probability_boundaries_and_required_values(self):
        base = dict(model=j.MODEL, answers=dict(q=dict(type='choice', choice='vision', confidence=.9,
            probabilities={'vision': .9, 'insufficient': .1})))
        for total in (.98, .99, 1., 1.01, 1.02, .97, 1.03):
            v = copy.deepcopy(base)
            v['answers']['q']['probabilities']['vision'] = total-.1
            if total in (.97, 1.03):
                self.assertRaises(ValueError, j.parse_native, v, j.WORKLOADS['routing'])
            else:
                metadata = {}
                self.assertEqual(j.parse_native(v, j.WORKLOADS['routing'], metadata=metadata), 'vision')
                self.assertAlmostEqual(sum(metadata['probabilities'].values()), 1.)
        for probabilities in (None, {'vision': 1.}, {'vision': 1., 'insufficient': -.01}):
            v = copy.deepcopy(base)
            v['answers']['q']['probabilities'] = probabilities
            self.assertRaises(ValueError, j.parse_native, v, j.WORKLOADS['routing'])

    def test_invalid_settled_answer_remains_in_denominator(self):
        cases, oracle = j.corpus()
        p = j.payload(cases[0], 'routing')
        response = dict(model=j.MODEL, usage=dict(input_tokens=20, output_tokens=3),
            answers=dict(q=dict(type='choice', choice='vision', confidence=.9,
                probabilities={'vision': .87, 'insufficient': .1})))
        row = dict(root=cases[0]['root'], workload='routing', payload=p, response=response)
        report = j.score([row], oracle, [j.receipt(p, response, 'invalid')])
        metrics = report['task_arm_table'][0]
        self.assertEqual(metrics['valid_answers'], 0)
        self.assertEqual(metrics['variant_availability'], 0)
        self.assertEqual(report['accounted_attempts'], 1)
        self.assertEqual(report['charged_nano_usd'], 840)

    def test_no_oracle_in_candidate_payload(self):
        cases, oracle = j.corpus()
        for c in cases:
            for w in j.WORKLOADS:
                value = j.payload(c, w)
                self.assertNotIn('intervention', value['state'])
                self.assertNotIn('expected', value['state'])
                self.assertNotIn('diagnostic_truth', value['state'])
                self.assertIn(c['root'], oracle)

    def test_native_usage_and_timeout(self):
        c = j.corpus()[0][0]
        p = j.payload(c, 'routing')
        r = j.receipt(p, None, 'timeout')
        self.assertIsNone(r['tariff_nano_usd'])
        self.assertEqual(r['charged_nano_usd'], j.reservation(p))
        v = dict(model=j.MODEL, answers=dict(q=dict(type='choice',choice='vision',probabilities={'vision':1.,'insufficient':0.},confidence=1.)))
        self.assertEqual(j.parse_native(v, j.WORKLOADS['routing']), 'vision')
        v['usage'] = dict(input_tokens=100000, output_tokens=0)
        self.assertRaises(ValueError, j.receipt, p, v, 'breach')

    def test_campaign_hard_max_and_every_reservation(self):
        self.assertRaises(ValueError, j.OfflineAllowance, 5000000001)
        ledger = j.OfflineAllowance(100)
        ledger.reserve(60)
        self.assertRaises(ValueError, ledger.reserve, 41)
        self.assertEqual(ledger.charged, 60)

    def test_collision_requires_ambiguity(self):
        cases, oracle = j.corpus()
        pair = [c for c in cases if c['root'].startswith('f00-collision')]
        self.assertEqual(pair[0]['evidence'], pair[1]['evidence'])
        self.assertEqual(oracle[pair[0]['root']]['expected']['cause'], 'ambiguous')
        self.assertEqual(oracle[pair[1]['root']]['expected']['cause'], 'ambiguous')

    def test_plan_bound(self):
        import tempfile
        from pathlib import Path
        for smoke, count in [(True, 25), (False, 1000)]:
            with tempfile.TemporaryDirectory() as temp:
                plan = j.plan(Path(temp)/'plan', smoke)
                self.assertEqual(plan['requests'], count)
                self.assertLess(plan['reservation_nano_usd'], 500000000)
                self.assertFalse(plan['authorized'])

    def test_unknown_cost_never_zero(self):
        cases, oracle = j.corpus()
        p = j.payload(cases[0], 'routing')
        response = dict(model=j.MODEL, answers=dict(q=dict(type='choice',choice='vision',probabilities={'vision':1.,'insufficient':0.},confidence=1.)))
        r = j.receipt(p, response, 'attempt')
        score = j.score([dict(root=cases[0]['root'], workload='routing', payload=p, response=response)], oracle, [r])
        self.assertEqual(score['unknown_costs'], 1)
        self.assertFalse(score['qualified'])


if __name__ == '__main__': unittest.main()
