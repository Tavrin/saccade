#!/usr/bin/env python3
"""CPU-light corpus/scoring contract tests; no provider, browser or model."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from corpus import freeze, verify, digest, encoded, render
from score import lower, upper, evaluate, validate_record, execution_cost

class ConstructedTests(unittest.TestCase):
    def test_incomplete_provider_stages_never_default_to_zero_cost(self):
        self.assertIsNone(execution_cost([],False))
        self.assertIsNone(execution_cost([{"cost_usd":.01}],False))
        self.assertIsNone(execution_cost([{"cost_usd":None}],True))
        self.assertEqual(execution_cost([],True),0)
        self.assertEqual(execution_cost([{"cost_usd":.01}],True),.01)

    def test_template_families_have_distinct_rendered_layouts(self):
        # Fixed root randomness isolates the actual family structure. Header rows
        # and columns define 32 unique layouts; development/calibration cannot
        # relabel a heldout rendering as a new family.
        headers=[]
        from PIL import Image
        import io
        for family in range(32):
            case=render(4406,family,"control","check_ui")
            self.assertIsNotNone(case)
            image=Image.open(io.BytesIO(case["before"]))
            dpr=case["dpr"]
            header=image.crop((0,0,image.width,20*dpr))
            headers.append(digest(header.tobytes()))
        self.assertEqual(len(set(headers)),32)

    def test_exact_confidence_bounds(self):
        self.assertAlmostEqual(upper(0,600),0.007646,places=5)
        self.assertAlmostEqual(lower(600,600),.05**(1/600),places=10)
        self.assertEqual(lower(0,600),0)
        self.assertEqual(upper(0,0),1)
        self.assertLess(lower(570,600),.95)

    def test_frozen_corpus_reproduces_without_labels_and_missing_runs_fail(self):
        with tempfile.TemporaryDirectory() as temp:
            left=Path(temp)/"left";right=Path(temp)/"right"
            a=freeze(left,10,4406,"fixture-r1","jev-1.13.0")
            b=freeze(right,10,4406,"fixture-r1","jev-1.13.0")
            self.assertEqual(a["manifest_hash"],b["manifest_hash"])
            verified,oracle=verify(left)
            self.assertEqual(len(verified["cases"]),120)
            families={}
            for case in verified["cases"]:
                self.assertEqual(case["case_id"],case["root_id"])
                self.assertEqual(families.setdefault(case["family"],case["split"]),case["split"])
                self.assertTrue(oracle[case["root_id"]]["oracle_verified"])
                if case["counterfactual"]:
                    self.assertEqual(case["structured_packet"],case["counterfactual"]["structured_packet"])
                    self.assertEqual(case["root_id"],case["counterfactual"]["root_id"])
            report=evaluate(left,None)
            self.assertTrue(all(d["status"]=="unqualified" for d in report["feature_decisions"].values()))
            self.assertEqual(report["workloads"]["explain"]["cascade"]["metrics"]["availability"],0)
            routing=report["feature_decisions"]["jev_evidence_routing"]
            self.assertFalse(routing["default_enabled"])
            self.assertEqual(routing["baseline_arm"],"cascade")
            self.assertEqual(routing["candidate_arm"],"cascade_jev_route")
            self.assertIsNone(report["workloads"]["routing"]["cascade_jev_route"]["metrics"]["incremental_cost_reduction"])
            self.assertIsNone(report["workloads"]["explain"]["cascade"]["metrics"]["cost_usd"])
            # Human-labelled or hand-scored results cannot enter the new policy.
            case=next(c for c in verified["cases"] if c["split"]=="heldout")
            bad={"case_id":case["case_id"],"arm":"cascade","outcome":"observed","human_label":"accept"}
            with self.assertRaises(ValueError):validate_record(bad,case,verified)
            bad_route={"case_id":case["case_id"],"arm":"cascade_jev_route","outcome":"observed","route_decision":"insufficient","used_vision":False}
            with self.assertRaises(ValueError):validate_record(bad_route,case,verified)
            # A changed rendered pixel is rejected even if a mutation receipt said success.
            fixture=left/case["after"];fixture.write_bytes(fixture.read_bytes()+b"changed")
            with self.assertRaises(ValueError):verify(left)

if __name__=="__main__":unittest.main()
