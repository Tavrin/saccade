#!/usr/bin/env python3
"""Read-only schedule/support/reservation plan. Never authorizes or dispatches."""
import argparse
from collections import Counter
import json
from pathlib import Path
from corpus import verify
from policy import ARMS, POLICY, WORKLOADS
from receipts import stages

def plan(directory):
    manifest,oracle=verify(directory)
    calls=Counter();classes={w:Counter() for w in WORKLOADS};roots=0;descendants=0
    for case in manifest["cases"]:
        if case["split"]!="heldout":continue
        roots+=1;classes[case["workload"]][oracle[case["case_id"]]["category"]]+=1
        for arm in ARMS:
            calls.update(p for p,o in stages(case,arm,{"observations":["planned support"]}))
            if case.get("counterfactual") and arm not in ("rules","oracle_jev"):
                descendants+=1
                calls.update(p for p,o in stages(dict(case,counterfactual=None),arm,{"observations":["planned support"]}))
    conservative=sum(calls[p]*(16000*42 if p=="jev" else 16000*750+4096*3750) for p in calls)
    support=all(classes[w]==Counter(POLICY["classes"]) for w in WORKLOADS)
    return dict(schema="saccade-assist-qualification-plan.v1",manifest_hash=manifest["manifest_hash"],roots=roots,
        root_arm_evaluations=roots*len(ARMS),descendant_evaluations=descendants,provider_requests=dict(calls),
        classes={k:dict(v) for k,v in classes.items()},qualifying_support=support,
        conservative_local_reservation_usd=conservative/1e9,campaign_parent_cap_usd=30,
        reservation_plan_fits=conservative<=30_000_000_000,
        live_billing_verified=False,locally_admissible_live_payloads=False,authorized=False,
        blockers=["verified model/operation billing ceilings and provider-side hard limit unavailable",
                  "live API payload/transcript/binary execution plan requires separate review"]+([] if support else ["insufficient qualifying corpus support"])+([] if conservative<=30_000_000_000 else ["conservative full schedule exceeds campaign allowance"]))
if __name__=="__main__":
    p=argparse.ArgumentParser();p.add_argument("--corpus",type=Path,required=True);p.add_argument("--stage2",action="store_true");p.add_argument("--budget-bounded",action="store_true");p.add_argument("--out",type=Path);args=p.parse_args()
    if args.stage2:
        from stage2 import report
        manifest,_=verify(args.corpus)
        if manifest["campaign"]!="g12-stage2/2": raise ValueError("stage2 requires its frozen profile")
        rows,result=report(manifest,args.corpus,args.budget_bounded)
        if args.out:
            args.out.mkdir()
            from corpus import put
            put(args.out/"requests.json",rows)
            from receipts import source_fact
            local=[dict(root=c["root_id"],arm=a,outcome="observed" if source_fact(c) and c["complete"] else "unverifiable",source_only=source_fact(c) and c["complete"]) for c in manifest["cases"] if c["split"]=="heldout" for a in ("rules","cascade") if a=="rules" or not c["complete"] or source_fact(c)]
            put(args.out/"local-results.json",local)
            put(args.out/"plan.json",result)
            result["request_file_hash"]=__import__("corpus").digest((args.out/"requests.json").read_bytes())
            put(args.out/"plan.json",result)
        print(json.dumps(result,sort_keys=True))
    else:
        if args.budget_bounded: raise ValueError("--budget-bounded requires --stage2")
        if args.out: raise ValueError("--out requires --stage2")
        print(json.dumps(plan(args.corpus),sort_keys=True))
