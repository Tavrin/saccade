#!/usr/bin/env python3
"""Pre-spend regressions: each positive proof is followed by a failing mutation."""
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest
from corpus import render, freeze, verify, digest, encoded, put
from score import assertion_correct, task_evidence, validate_record, execution_cost, summarize, cluster, gates, lower, evaluate
from dry_run import FakeCampaign
from receipts import verify_execution

class G12Tests(unittest.TestCase):
    def good_items(self):
        items=[]
        for n in range(1000):
            category="challenge" if n<600 else "control" if n<800 else "unavailable"
            committed=category!="unavailable"
            items.append(dict(root=str(n),family=f"family-{n%20}",category=category,eligible=committed,complete=True,
                committed=committed,case_correct=committed,important=category=="challenge",detected=committed,
                reassurance=False,assertions=int(committed),correct_assertions=int(committed),order_disagreement=False,
                paired_order=committed,necessary=category=="challenge",routing_error=False,abstention=not committed,
                contradiction_withheld=True,mechanical_valid=True,cost=.001,provenance=[],elapsed_ms=1,queue_delay_ms=0,retries=0))
        return items
    def checks(self,items):
        m=summarize(items);families={f:summarize([i for i in items if i["family"]==f]) for f in {i["family"] for i in items}}
        return gates(m,cluster(items),families,True)
    def test_positive_qualification_fixture_and_wrong_control_mutation(self):
        items=self.good_items();checks=self.checks(items)
        self.assertTrue(all(checks.values()),checks)
        for item in items:
            if item["category"]=="control":item["case_correct"]=False
        self.assertFalse(self.checks(items)["semantic_outcomes"])
    def test_zero_roots_all_abstentions_zero_pairs_unknown_cost_and_safety_fail(self):
        self.assertFalse(all(self.checks([]).values()))
        items=self.good_items()
        for i in items:i.update(committed=False,assertions=0,correct_assertions=0,paired_order=False)
        self.assertFalse(self.checks(items)["nonvacuous"])
        self.assertFalse(self.checks(items)["order_handling"])
        items=self.good_items();items[0]["cost"]=None
        self.assertFalse(self.checks(items)["cost_accounting"])
        items=self.good_items();items[0]["mechanical_valid"]=False
        self.assertFalse(self.checks(items)["authority_and_input_safety"])
    def test_assertions_and_descendants_do_not_inflate_root_confidence(self):
        items=self.good_items();bound=summarize(items)["assertion_precision_lower95"]
        for i in items:
            i["assertions"]*=100;i["correct_assertions"]*=100
        self.assertEqual(bound,summarize(items)["assertion_precision_lower95"])
        self.assertAlmostEqual(bound,lower(800,800))
    def test_unavailable_commitment_is_independently_rejected(self):
        items=self.good_items();items[-1].update(committed=True,case_correct=True,assertions=1,correct_assertions=1)
        self.assertFalse(self.checks(items)["semantic_outcomes"])
    def test_localization_partial_presence_and_incidental_task_facts(self):
        rendered=render(24406,15,"challenge","explain")
        witness=rendered["witnesses"]
        case=dict(dimensions=rendered["dimensions"],target=rendered["target"],label=rendered["label"],task="check_ui")
        oracle={"rendered_witness":witness}
        o=dict(image_role="after",statement="text:"+case["label"],geometry={"type":"box","pixels":[*case["target"][:2],1,1]})
        self.assertFalse(assertion_correct(o,oracle,case,Path('.')))
        self.assertTrue(witness["label_pixels"])
        self.assertFalse(witness["label_complete"])
        o["statement"]="presence:absent";o["geometry"]["pixels"]=case["target"]
        self.assertFalse(assertion_correct(o,oracle,case,Path('.')))
        o["statement"]="presence:present";o["geometry"]["pixels"]=[0,0,*case["dimensions"]]
        self.assertTrue(assertion_correct(o,oracle,case,Path('.')))
        self.assertFalse(task_evidence([o],case,oracle,Path('.')))
        o["statement"]="clipping:clipped"
        self.assertTrue(task_evidence([o],case,oracle,Path('.')))
    def test_labels_have_no_seed_or_class_suffix(self):
        for family in (12,15,20):
            labels={render(seed,family,category,"check_ui")["label"] for seed in (24406,25006,25206) for category in ("challenge","control","unavailable")}
            self.assertEqual(len(labels),1)
            self.assertFalse(any(ch.isdigit() for ch in next(iter(labels))))
    def test_gate_receipt_rejects_zero_counts_and_changed_binary(self):
        from corpus import gate_source_hash
        from policy import GATES
        from score import verify_gate_receipt
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp);binary=directory/"fixture-binary";binary.write_bytes(b"offline fixture binary")
            identity=gate_source_hash()
            receipt=dict(schema="saccade-assist-gates.v1",source_hash=identity,gates={g:"PASS" for g in GATES},
                execution=dict(source_before=identity,test_counts={k:1 for k in ("core-tests","cli-tests","wave4-heavy-cli","wave4-batch-routing")},
                    binary=str(binary),binary_hash=digest(binary.read_bytes()),toolchain="fixture rustc",features="assist,schema",build_env={}))
            path=directory/"receipt.json";put(path,receipt)
            self.assertTrue(verify_gate_receipt(path))
            receipt["execution"]["test_counts"]["core-tests"]=0;put(path,receipt)
            with self.assertRaises(ValueError):verify_gate_receipt(path)
            receipt["execution"]["test_counts"]["core-tests"]=1;put(path,receipt)
            binary.write_bytes(b"changed fixture binary")
            with self.assertRaises(ValueError):verify_gate_receipt(path)

    def test_gate_receipt_write_failure_cannot_exit_successfully(self):
        import subprocess,shlex
        from corpus import ROOT,gate_source_hash
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp);(directory/"debug").mkdir();(directory/"debug/saccade").write_bytes(b"fixture binary")
            counts=directory/"counts.jsonl";counts.write_text(json.dumps({k:1 for k in ("core-tests","cli-tests","wave4-heavy-cli","wave4-batch-routing")})+"\n")
            destination=directory/"unwritable-receipt";destination.mkdir()
            script=(ROOT/"scripts/gates-wave4.sh").read_text();tail=script[script.index('if [[ "$failed" -eq 0 ]]'):]
            prefix="\n".join("%s=%s"%(k,shlex.quote(str(v))) for k,v in {"failed":0,"source_before":gate_source_hash(),"counts":counts,"CARGO_TARGET_DIR":directory,"SACCADE_WAVE4_GATE_RECEIPT":destination}.items())+"\n"
            result=subprocess.run(["bash","-c",prefix+tail],cwd=ROOT,capture_output=True,text=True)
            self.assertNotEqual(result.returncode,0)
            self.assertIn("IsADirectoryError",result.stderr)

    def test_transcripts_are_mandatory_and_reconcile_usage_identity_orders(self):
        with tempfile.TemporaryDirectory() as temp:
            directory=Path(temp);manifest=freeze(directory/"corpus",5,4406,"fixture-r1","jev-1.13.0")
            manifest,oracle=verify(directory/"corpus")
            from plan import plan
            frozen_plan=plan(directory/"corpus")
            self.assertFalse(frozen_plan["qualifying_support"])
            self.assertFalse(frozen_plan["authorized"])
            self.assertGreater(sum(frozen_plan["provider_requests"].values()),0)
            campaign=FakeCampaign(manifest,directory/"corpus")
            case=next(c for c in manifest["cases"] if c["split"]=="heldout" and c["workload"]=="explain" and c["complete"])
            row=campaign.execute(case,"two_gemini",oracle[case["case_id"]])
            executions={r["request_id"]:r for r in campaign.executions};ledger={r["request_id"]:r for r in campaign.ledger}
            validate_record(row,case,manifest)
            deadline=campaign.deadline;campaign.deadline=-1
            before=len(campaign.executions)
            with self.assertRaises(TimeoutError):campaign.execute(case,"two_gemini",oracle[case["case_id"]])
            self.assertEqual(before,len(campaign.executions));campaign.deadline=deadline
            verify_execution(row,case,manifest,executions,ledger,set())
            bad=copy.deepcopy(row);bad["provenance"]=[]
            with self.assertRaises(ValueError):validate_record(bad,case,manifest)
            bad=copy.deepcopy(row);bad["source_only"]=True
            with self.assertRaises(ValueError):validate_record(bad,case,manifest)
            self.assertIsNone(execution_cost([],True))
            for mutate in (lambda r:r["provenance"][0].update(request_id="fabricated"),lambda r:r["provenance"][0].update(order="ba" if r["provenance"][0]["order"]=="ab" else "ab"),lambda r:r["provenance"][0].update(response_hash=digest(b"fake")),lambda r:r["provenance"][0].update(cost_usd=0),lambda r:r.update(outcome="not_observed")):
                bad=copy.deepcopy(row);mutate(bad)
                with self.assertRaises(ValueError):verify_execution(bad,case,manifest,executions,ledger,set())
            unknown=copy.deepcopy(executions);identity=row["provenance"][0]["request_id"];unknown[identity]["response"].pop("usageMetadata")
            with self.assertRaises(ValueError):verify_execution(row,case,manifest,unknown,ledger,set())
            # Gate-receipt acceptance must not overwrite a run's own safety failure.
            from unittest.mock import patch
            from policy import ARMS
            full=FakeCampaign(manifest,directory/"corpus")
            rows=[full.execute(c,arm,oracle[c["case_id"]]) for c in manifest["cases"] if c["split"]=="heldout" for arm in ARMS]
            result=directory/"results.jsonl"
            def persist():
                result.write_bytes(b"\n".join(encoded(r) for r in rows)+b"\n")
                result.with_suffix(".executions.jsonl").write_bytes(b"\n".join(encoded(r) for r in full.executions)+b"\n")
                result.with_suffix(".ledger.jsonl").write_bytes(b"\n".join(encoded(r) for r in full.ledger)+b"\n")
            persist()
            with patch("score.verify_gate_receipt",return_value=True):
                good=evaluate(directory/"corpus",result,directory/"fixture-gate.json")
                self.assertTrue(good["workloads"]["explain"]["two_gemini"]["gates"]["authority_and_input_safety"])
                next(r for r in rows if r["case_id"]==case["case_id"] and r["arm"]=="two_gemini")["mechanical_valid"]=False
                persist();bad=evaluate(directory/"corpus",result,directory/"fixture-gate.json")
                self.assertFalse(bad["workloads"]["explain"]["two_gemini"]["gates"]["authority_and_input_safety"])
            # A properly rehashed subset is still an invalid frozen topology.
            document=json.loads((directory/"corpus/manifest.json").read_bytes());document["cases"].pop();document.pop("manifest_hash");document["manifest_hash"]=digest(encoded(document));put(directory/"corpus/manifest.json",document)
            with self.assertRaises(ValueError):verify(directory/"corpus")

if __name__=="__main__":unittest.main()
