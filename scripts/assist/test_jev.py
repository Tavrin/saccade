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
