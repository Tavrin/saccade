#!/usr/bin/env python3
"""CPU-only fake-provider campaign: frozen pixels -> dispatch receipts -> scoring.

This is local pipeline evidence, never evidence of a live model's accuracy.
"""
import argparse
from pathlib import Path
import json
import tempfile
import time
from PIL import Image
from corpus import freeze, verify, encoded, digest, put
from policy import ARMS
from receipts import stages, request, actual_nano, PRICE_ID, source_fact
from score import evaluate

EXPECTED=Path(__file__).parent/"fixtures/dry-run-expected.json"

def observations(case, truth, directory):
    width,height=case["dimensions"];target=case["target"]
    role="single" if case["task"]=="check_ui" else "after"
    out=[]
    def add(statement,rect,refs):
        out.append(dict(observation_id=f"fact-{len(out)}",image_role=role,kind=statement.split(":")[0],statement=statement,
                        geometry={"type":"box","pixels":rect},visibility="visible",evidence_refs=refs,uncertainty=0.))
    if case["task"]=="check_ui":
        witness=truth["rendered_witness"]
        add("text:"+case["label"] if witness["label_complete"] else "clipping:clipped" if witness["label_pixels"] else "presence:absent",[0,0,width,height],["target"])
    else:
        before=Image.open(directory/case["before"]).convert("RGB");after=Image.open(directory/case["after"]).convert("RGB")
        def changed(rect):
            x,y,w,h=rect
            return before.crop((x,y,x+w,y+h)).tobytes()!=after.crop((x,y,x+w,y+h)).tobytes()
        add("appearance:"+("changed" if changed(target) else "unchanged"),target,["target"])
        for exclusion in case["exclusions"]:
            indices=[n for start,length in exclusion["runs"] for n in (start,start+length-1)]
            xs=[i%width for i in indices];ys=[i//width for i in indices]
            rect=[min(xs),min(ys),max(xs)-min(xs)+1,max(ys)-min(ys)+1]
            add("appearance:"+("changed" if changed(rect) else "unchanged"),rect,[exclusion["id"]])
    return out

class FakeCampaign:
    def __init__(self, manifest, directory):
        self.manifest=manifest;self.directory=directory;self.executions=[];self.ledger=[];self.reserved=0;self.deadline=time.monotonic()+300;self.root_deadlines={}
    def execute(self,case,arm,truth,child=False):
        root_deadline=self.root_deadlines.setdefault(case["case_id"],time.monotonic()+300)
        if time.monotonic()>=min(self.deadline,root_deadline): raise TimeoutError("campaign/root deadline exhausted; no extension for descendants")
        source=source_fact(case) and arm in ("rules","cascade","cascade_jev_route")
        semantic=source or (case["complete"] and arm!="rules")
        row=dict(case_id=case["case_id"],arm=arm,outcome="observed" if source else truth["expected_outcome"] if semantic else "unverifiable",
            complete=True,mechanical_valid=True,observations=observations(case,truth,self.directory) if semantic and not source else [],provenance=[],
            source_only=source,used_vision=False,order_disagreement=False,contradiction_withheld=True,
            counterfactual_checked=not case.get("counterfactual"),elapsed_ms=1,queue_delay_ms=0,retries=0,route_decision=None)
        for provider,order in stages(case,arm,row):
            if time.monotonic()>=min(self.deadline,root_deadline): raise TimeoutError("campaign/root deadline exhausted before dispatch")
            payload=request(case,arm,provider,order,row["observations"]);request_hash=digest(encoded(payload))
            identity=f"fake-execution-{len(self.executions):06}"
            # Reservation exists before the fake response, just as it must before HTTP.
            allowance=30_000_000
            if self.reserved+allowance>30_000_000_000: raise ValueError("fake campaign cap exhausted")
            self.reserved+=allowance
            money=dict(request_id=identity,request_hash=request_hash,campaign=self.manifest["campaign"],reserved_nano_usd=allowance,outcome="reserved",actual_nano_usd=None)
            self.ledger.append(money)
            answer=dict(request_hash=request_hash,outcome=row["outcome"],observations=row["observations"])
            response=dict(id=identity,model=self.manifest["models"][provider])
            revision=self.manifest["models"][provider+"_revision"]
            if provider=="gemini":
                response.update(modelVersion=revision,usageMetadata={"promptTokenCount":100,"candidatesTokenCount":20,"thoughtsTokenCount":10,"totalTokenCount":130},candidates=[{"finishReason":"STOP","content":{"parts":[{"text":encoded(answer).decode()}]}}])
            elif provider=="openrouter":
                response.update(object="chat.completion",system_fingerprint=revision,provider="fixture-routing-provider",usage={"prompt_tokens":100,"completion_tokens":30,"total_tokens":130,"cost":.0001875},choices=[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":encoded(answer).decode(),"refusal":None}}])
            else:
                response.update(modelVersion=revision,usage={"input_tokens":100,"output_tokens":0},answers={"q":{"choice":"vision" if order=="route" else "supported"}})
            actual=actual_nano(response,provider);money.update(actual_nano_usd=actual,outcome="completed")
            self.reserved-=allowance-actual
            self.executions.append(dict(request_id=identity,request=payload,response=response))
            row["provenance"].append(dict(request_id=identity,provider=provider,order=order,requested_model=response["model"],returned_model=response["model"],returned_revision=revision,
                request_hash=request_hash,response_hash=digest(encoded(response)),usage=response.get("usageMetadata",response.get("usage")),cost_usd=actual/1e9,
                started_ms=1,finished_ms=2,elapsed_ms=1,cache_status="miss",price_version=PRICE_ID))
            row["used_vision"] |= provider!="jev"
            if order=="route":row["route_decision"]="vision"
        if case.get("counterfactual") and arm not in ("rules","oracle_jev"):
            descendant=dict(case,after=case["counterfactual"]["path"],after_hash=case["counterfactual"]["hash"],counterfactual=None)
            child_truth=dict(truth,expected_outcome="observed",rendered_witness=truth["counterfactual_witness"])
            row["counterfactual"]=self.execute(descendant,arm,child_truth,True)
            row["counterfactual_checked"]=True
        return row

def run(out,target=20):
    out.mkdir(parents=True,exist_ok=False)
    corpus=out/"corpus";freeze(corpus,target,4406,"fixture-r1","jev-1.13.0")
    manifest,oracle=verify(corpus);campaign=FakeCampaign(manifest,corpus)
    cases=sorted((c for c in manifest["cases"] if c["split"]=="heldout"),key=lambda c:digest(encoded(["balanced-schedule/1",c["case_id"]])))
    rows=[campaign.execute(case,arm,oracle[case["case_id"]]) for case in cases for arm in sorted(ARMS,key=lambda arm:digest(encoded([case["case_id"],arm])))]
    result=out/"results.jsonl";result.write_bytes(b"\n".join(encoded(row) for row in rows)+b"\n")
    result.with_suffix(".executions.jsonl").write_bytes(b"\n".join(encoded(r) for r in campaign.executions)+b"\n")
    result.with_suffix(".ledger.jsonl").write_bytes(b"\n".join(encoded(r) for r in campaign.ledger)+b"\n")
    report=evaluate(corpus,result);put(out/"saccade-constructed-truth.v1.json",report)
    summary=dict(mode="synthetic-offline/1",provider_calls=0,spent_usd=0,frozen_heldout_roots=len(cases),result_rows=len(rows),
        execution_receipts=len(campaign.executions),ledger_receipts=len(campaign.ledger),
        features={k:v["status"] for k,v in report["feature_decisions"].items()},
        cascade={w:{k:report["workloads"][w]["cascade"]["metrics"][k] for k in ("coverage","case_precision","control_false_positives","unavailable_commitments")} for w in report["workloads"]})
    put(out/"summary.json",summary)
    return summary

if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("--out",type=Path);p.add_argument("--record-expected",action="store_true");args=p.parse_args()
    if args.out:summary=run(args.out)
    else:
        with tempfile.TemporaryDirectory(prefix="saccade-dry-run-") as temp:summary=run(Path(temp)/"run")
    if args.record_expected:EXPECTED.parent.mkdir(parents=True,exist_ok=True);put(EXPECTED,summary)
    elif summary!=json.loads(EXPECTED.read_bytes()):raise SystemExit("dry-run output differs from frozen expected fixture")
    print(json.dumps(summary,sort_keys=True))
