#!/usr/bin/env python3
"""Constructed-only scoring: missing results, abstentions and unknown costs remain explicit."""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import random
import sys
from PIL import Image
from corpus import verify, digest, encoded, put, gate_source_hash
from receipts import verify_execution, stages, source_fact
from policy import SCHEMA, POLICY, ARMS, WORKLOADS, GATES

def binomial_cdf(k, n, p):
    if k < 0: return 0.
    if k >= n: return 1.
    if p <= 0: return 1.
    if p >= 1: return 0.
    log_mass = n * math.log1p(-p)
    terms = [log_mass]
    for index in range(k):
        log_mass += math.log(n-index)-math.log(index+1)+math.log(p)-math.log1p(-p)
        terms.append(log_mass)
    largest=max(terms)
    return min(1.,math.exp(largest)*sum(math.exp(t-largest) for t in terms))

def lower(successes, n, alpha=.05):
    if not n or not successes: return 0.
    lo,hi=0.,1.
    for _ in range(64):
        mid=(lo+hi)/2
        if 1-binomial_cdf(successes-1,n,mid)<alpha: lo=mid
        else: hi=mid
    return (lo+hi)/2

def upper(failures, n, alpha=.01):
    if not n or failures==n: return 1.
    if not failures: return -math.expm1(math.log(alpha)/n)
    lo,hi=0.,1.
    for _ in range(64):
        mid=(lo+hi)/2
        if binomial_cdf(failures,n,mid)>alpha: lo=mid
        else: hi=mid
    return (lo+hi)/2

def fraction(num, den): return num/den if den else 0.

def percentile(values, percent):
    if not values: return None
    values=sorted(values);return values[min(len(values)-1,math.ceil(len(values)*percent)-1)]

def assertion_correct(observation, oracle, case, directory):
    """Finite preregistered facts only. Unknown paraphrases are unsupported, never dropped."""
    statement=observation.get("statement","")
    role=observation.get("image_role")
    if role not in ("before","after","single"): return False
    geom=observation.get("geometry",{})
    values=geom.get("pixels",[])
    if geom.get("type")=="point" and len(values)==2:
        box=[*values,1,1]
    elif geom.get("type")=="box" and len(values)==4:
        box=values
    else: return False
    if any(not isinstance(v,(int,float)) or not math.isfinite(v) for v in box): return False
    x,y,w,h=box;tx,ty,tw,th=case["target"]
    width,height=case["dimensions"]
    if x<0 or y<0 or w<=0 or h<=0 or x+w>width or y+h>height: return False
    covers=lambda rect: x<=rect[0] and y<=rect[1] and x+w>=rect[0]+rect[2] and y+h>=rect[1]+rect[3]
    witness=oracle["rendered_witness"]
    glyphs=witness["before_label_pixels" if role=="before" else "label_pixels"]
    inside=lambda index: x<=index%width<x+w and y<=index//width<y+h
    visible=witness["before_label_complete" if role=="before" else "label_complete"]
    if statement.startswith("presence:") or statement.startswith("text:"):
        if statement=="presence:present": return any(inside(n) for start,length in glyphs for n in range(start,start+length))
        if statement=="presence:absent": return not glyphs and covers(case["target"])
        if statement=="text:"+case["label"]: return visible and bool(glyphs) and all(inside(n) for start,length in glyphs for n in range(start,start+length))
        return False
    if statement in ("clipping:clipped","clipping:contained"):
        return covers(case["target"]) and ((not visible and bool(glyphs)) if statement=="clipping:clipped" else visible)
    if statement in ("appearance:changed","appearance:unchanged"):
        before=Image.open(directory/case["before"]).convert("RGB")
        after=Image.open(directory/case["after"]).convert("RGB")
        bounds=(math.floor(x),math.floor(y),math.ceil(x+w),math.ceil(y+h))
        changed=before.crop(bounds).tobytes()!=after.crop(bounds).tobytes()
        return changed if statement=="appearance:changed" else not changed
    return False

def task_evidence(observations, case, truth, directory):
    """Only independently witnessed statements establishing the requested task count."""
    def covers(observation, rect):
        g=observation.get("geometry",{})
        if g.get("type")!="box" or len(g.get("pixels",[]))!=4: return False
        x,y,w,h=g["pixels"];rx,ry,rw,rh=rect
        return x<=rx and y<=ry and x+w>=rx+rw and y+h>=ry+rh
    if case["task"]=="check_ui":
        return any(o.get("image_role") in ("after","single") and o.get("statement") in ("text:"+case["label"],"presence:absent","clipping:clipped") and assertion_correct(o,truth,case,directory) for o in observations)
    if case["task"]=="explain":
        return any(o.get("statement") in ("appearance:changed","appearance:unchanged") and covers(o,case["target"]) and assertion_correct(o,truth,case,directory) for o in observations)
    # Every exclusion needs an independently checked content statement and target coverage.
    width=case["dimensions"][0]
    for e in case["exclusions"]:
        pixels=[n for start,length in e["runs"] for n in (start,start+length-1)]
        if not pixels: return False
        xs=[n%width for n in pixels];ys=[n//width for n in pixels]
        rect=[min(xs),min(ys),max(xs)-min(xs)+1,max(ys)-min(ys)+1]
        if not any(e["id"] in o.get("evidence_refs",[]) and covers(o,rect) and o.get("statement") in ("appearance:changed","appearance:unchanged") and assertion_correct(o,truth,case,directory) for o in observations): return False
    return any(covers(o,case["target"]) and assertion_correct(o,truth,case,directory) for o in observations)

def validate_record(row, case, manifest):
    allowed={"case_id","arm","outcome","complete","observations","provenance","mechanical_valid",
             "order_disagreement","contradiction_withheld","used_vision","queue_delay_ms","retries",
             "counterfactual_checked","source_only","elapsed_ms","failure","counterfactual","route_decision"}
    if not isinstance(row,dict) or set(row)-allowed: raise ValueError("unknown result field (manual scores/labels are forbidden)")
    if row["arm"] not in ARMS or row["case_id"]!=case["case_id"]: raise ValueError("result identity")
    if row.get("outcome") not in ("observed","not_observed","unverifiable"): raise ValueError("unknown semantic outcome")
    for field in ("complete","mechanical_valid","order_disagreement","contradiction_withheld","used_vision","counterfactual_checked","source_only"):
        if field in row and not isinstance(row[field],bool): raise ValueError("invalid result flag")
    if row.get("route_decision") not in (None,"vision","insufficient"): raise ValueError("invalid routing choice")
    if row.get("route_decision")=="insufficient" and (row.get("outcome")!="unverifiable" or row.get("used_vision")): raise ValueError("router abstention acquired semantic authority")
    if len(row.get("observations",[]))>64 or len(row.get("provenance",[]))>4: raise ValueError("unbounded result")
    child=row.get("counterfactual")
    if child:
        if not case.get("counterfactual") or child.get("counterfactual"):
            raise ValueError("unexpected or recursive descendant")
        child_case=dict(case,after=case["counterfactual"]["path"],after_hash=case["counterfactual"]["hash"],counterfactual=None)
        validate_record(child,child_case,manifest)
        if row.get("counterfactual_checked")!=child.get("complete"):
            raise ValueError("counterfactual receipt flag mismatch")
    for receipt in row.get("provenance",[]):
        provider=receipt.get("provider")
        if provider not in ("gemini","jev","openrouter"): raise ValueError("unapproved provider")
        model=manifest["models"][provider];revision=manifest["models"][provider+"_revision"]
        if receipt.get("requested_model")!=model or receipt.get("returned_model")!=model or receipt.get("returned_revision")!=revision: raise ValueError("qualified revision drift")
        if receipt.get("cache_status") not in ("miss","bypass"): raise ValueError("cache replay is not an independent evaluation sample")
        if receipt.get("cost_usd") is not None and (not math.isfinite(receipt["cost_usd"]) or receipt["cost_usd"]<0): raise ValueError("invalid cost")
        if receipt.get("finished_ms",0)<receipt.get("started_ms",0): raise ValueError("invalid timestamps")
    if row.get("source_only") and not (source_fact(case) and row["arm"] in ("rules","cascade","cascade_jev_route")):
        raise ValueError("fabricated source-only execution")
    if row.get("complete") and len(row.get("provenance",[]))!=len(stages(case,row["arm"],row)):
        raise ValueError("completed provider arm lacks required execution receipts")

def execution_cost(receipts, complete):
    """Failed stages may have charged reservations even without usable provenance."""
    if not complete or not receipts or any(r.get("cost_usd") is None for r in receipts): return None
    return sum(r["cost_usd"] for r in receipts)

def summarize(items):
    count=len(items);eligible=sum(i["eligible"] for i in items)
    complete=sum(i["eligible"] and i["complete"] for i in items)
    committed=sum(i["committed"] for i in items)
    challenges=sum(i["important"] for i in items)
    detected=sum(i["important"] and i["detected"] for i in items)
    reassurance=sum(i["reassurance"] for i in items)
    assertions=sum(i["assertions"] for i in items);correct=sum(i["correct_assertions"] for i in items)
    disagree=sum(i["order_disagreement"] for i in items)
    both=sum(i["paired_order"] for i in items)
    necessary=sum(i["necessary"] for i in items);skips=sum(i["routing_error"] for i in items)
    known_cost=all(i["cost"] is not None for i in items)
    tokens=Counter()
    for item in items:
        for receipt in item["provenance"]:
            for name in ("input_tokens","candidate_tokens","thinking_tokens","cached_input_tokens","total_tokens"):
                value=receipt.get("usage",{}).get(name)
                if isinstance(value,int):tokens[name]+=value
    return dict(roots=count,eligible_requests=eligible,completed_requests=complete,
        availability=fraction(complete,eligible),committed_roots=committed,coverage=fraction(committed,count),
        abstentions=sum(i["abstention"] for i in items),provider_failures=sum(i["eligible"] and not i["complete"] for i in items),
        case_correct=sum(i["case_correct"] for i in items),case_precision=fraction(sum(i["case_correct"] for i in items),committed),
        committed_assertions=assertions,correct_assertions=correct,unsupported_assertions=assertions-correct,
        assertion_precision=fraction(correct,assertions),assertion_precision_lower95=lower(sum(i["assertions"]>0 and i["correct_assertions"]==i["assertions"] for i in items),sum(i["assertions"]>0 for i in items)),
        control_roots=sum(i.get("category")=="control" for i in items),
        control_false_positives=sum(i.get("category")=="control" and i["committed"] and not i["case_correct"] for i in items),
        unavailable_commitments=sum(i.get("category")=="unavailable" and i["committed"] for i in items),
        challenges=challenges,detected=detected,defect_recall=fraction(detected,challenges),defect_recall_lower95=lower(detected,challenges),
        false_reassurance=reassurance,false_reassurance_upper99=upper(reassurance,challenges),
        paired_order_roots=both,order_disagreement=disagree,order_disagreement_rate=fraction(disagree,both),
        unresolved_contradictions=sum(i["order_disagreement"] and not i["contradiction_withheld"] for i in items),
        geometry_citation_failures=sum(not i["mechanical_valid"] for i in items),
        necessary_vision_roots=necessary,routing_errors=skips,routing_error_upper99=upper(skips,necessary),
        tokens=dict(tokens),cost_usd=sum(i["cost"] for i in items) if known_cost else None,
        cold_call_p50_ms=percentile([r["elapsed_ms"] for i in items for r in i["provenance"] if "elapsed_ms" in r],.50),
        cold_call_p95_ms=percentile([r["elapsed_ms"] for i in items for r in i["provenance"] if "elapsed_ms" in r],.95),
        end_to_end_p50_ms=percentile([i["elapsed_ms"] for i in items if i["complete"]],.50),
        end_to_end_p95_ms=percentile([i["elapsed_ms"] for i in items if i["complete"]],.95),
        queue_delay_ms=sum(i["queue_delay_ms"] for i in items),retries=sum(i["retries"] for i in items),cache_replays=0)

def cluster(items):
    groups=defaultdict(list)
    for item in items:groups[item["family"]].append(item)
    rng=random.Random(POLICY["cluster_bootstrap_seed"])
    families=sorted(groups);precision=[];recall=[];coverage=[];reassurance=[]
    for _ in range(POLICY["cluster_bootstrap_repeats"]):
        sample=[i for _ in families for i in groups[rng.choice(families)]] if families else []
        precision.append(fraction(sum(i["correct_assertions"] for i in sample),sum(i["assertions"] for i in sample)))
        recall.append(fraction(sum(i["important"] and i["detected"] for i in sample),sum(i["important"] for i in sample)))
        coverage.append(fraction(sum(i["committed"] for i in sample),len(sample)))
        reassurance.append(fraction(sum(i["reassurance"] for i in sample),sum(i["important"] for i in sample)))
    return dict(unit="template family; all descendants remain grouped",families=len(families),
        repeats=POLICY["cluster_bootstrap_repeats"],precision_lower95=percentile(precision,.05),
        recall_lower95=percentile(recall,.05),coverage_lower95=percentile(coverage,.05),
        false_reassurance_upper99=percentile(reassurance,.99))

def gates(metric, clusters, family_metrics, corpus_complete):
    family_pass=all(m["coverage"]>=.60 and m["assertion_precision"]>=.95 and m["defect_recall"]>=.90 for m in family_metrics.values())
    return dict(nonvacuous=metric["roots"]>0 and metric["committed_roots"]>0,
        semantic_outcomes=metric["case_precision"]>=.95 and metric["control_roots"]>0 and metric["control_false_positives"]==0 and metric["unavailable_commitments"]==0,
        cost_accounting=metric["cost_usd"] is not None,
        corpus_and_independence=corpus_complete and clusters["families"]>=20,
        authority_and_input_safety=metric["geometry_citation_failures"]==0 and metric["unresolved_contradictions"]==0,
        availability=metric["availability"]>=.95,useful_coverage=metric["coverage"]>=.60,
        semantic_precision=metric["assertion_precision_lower95"]>=.95,
        defect_detection=metric["defect_recall_lower95"]>=.90,
        false_reassurance=metric["false_reassurance_upper99"]<=.01,
        order_handling=metric["paired_order_roots"]>=POLICY["usable_paired_roots_min"] and metric["order_disagreement_rate"]<=.05 and metric["unresolved_contradictions"]==0,
        family_point_gates=family_pass,
        clustered_uncertainty=(clusters["precision_lower95"] or 0)>=.95 and (clusters["recall_lower95"] or 0)>=.90 and (clusters["coverage_lower95"] or 0)>=.60 and clusters["false_reassurance_upper99"] is not None and clusters["false_reassurance_upper99"]<=.01)

def verify_gate_receipt(path):
    receipt=json.loads(path.read_bytes())
    if set(receipt)!={"schema","source_hash","gates","execution"} or receipt["schema"]!="saccade-assist-gates.v1" or receipt["source_hash"]!=gate_source_hash() or receipt["gates"]!={g:"PASS" for g in GATES}:
        raise ValueError("required exact-source heavy gate receipt is incomplete or stale")
    execution=receipt["execution"]
    counts=execution.get("test_counts",{})
    if execution.get("source_before")!=receipt["source_hash"] or set(counts)!={"core-tests","cli-tests","wave4-heavy-cli","wave4-batch-routing"} or any(type(n) is not int or n<=0 for n in counts.values()) or not execution.get("toolchain") or execution.get("features")!="assist,schema":
        raise ValueError("zero-test or incomplete execution receipt")
    if digest(Path(execution["binary"]).read_bytes())!=execution["binary_hash"]: raise ValueError("gate binary identity drift")
    return True

def evaluate(directory, result_file, gate_receipt=None):
    manifest,oracle=verify(directory)
    mechanical_fixtures=verify_gate_receipt(gate_receipt) if gate_receipt else False
    cases={c["case_id"]:c for c in manifest["cases"] if c["split"]=="heldout"}
    results={};executions={};ledger={};seen=set()
    if result_file and result_file.exists():
        for suffix,dest in ((".executions.jsonl",executions),(".ledger.jsonl",ledger)):
            path=result_file.with_suffix(suffix)
            if path.exists():
                for line in path.read_text().splitlines():
                    receipt=json.loads(line);identity=receipt["request_id"]
                    if identity in dest: raise ValueError("duplicate execution or money receipt")
                    dest[identity]=receipt
    if result_file and result_file.exists():
        if result_file.stat().st_size>128*1024*1024: raise ValueError("results too large")
        for line in result_file.read_text().splitlines():
            if len(line)>512*1024: raise ValueError("result row too large")
            row=json.loads(line)
            if row.get("case_id") not in cases: raise ValueError("held-out result bound to absent root")
            case=cases[row["case_id"]];validate_record(row,case,manifest)
            verify_execution(row,case,manifest,executions,ledger,seen)
            if row.get("counterfactual"):
                child_case=dict(case,after=case["counterfactual"]["path"],after_hash=case["counterfactual"]["hash"],counterfactual=None)
                verify_execution(row["counterfactual"],child_case,manifest,executions,ledger,seen)
            key=(row["case_id"],row["arm"])
            if key in results:raise ValueError("duplicate root/arm (retries/descendants are not independent samples)")
            results[key]=row
    if set(executions)!=set(ledger) or set(executions)!=seen: raise ValueError("unreconciled dispatches outside result rows")
    if any(r.get("outcome")!="completed" or r.get("actual_nano_usd") is None for r in ledger.values()): raise ValueError("unknown campaign accounting")
    if sum(r["actual_nano_usd"] for r in ledger.values())>30_000_000_000: raise ValueError("campaign monetary ceiling exceeded")
    grouped=defaultdict(list)
    for case_id,case in cases.items():
        truth=oracle[case_id]
        for arm in ARMS:
            row=results.get((case_id,arm),{})
            child=row.get("counterfactual")
            complete=row.get("complete",False) and row.get("mechanical_valid",False)
            if case.get("counterfactual") and arm not in ("rules","oracle_jev"):
                complete=complete and bool(child) and child.get("complete",False) and child.get("mechanical_valid",False)
            outcome=row.get("outcome","unverifiable")
            committed=complete and outcome!="unverifiable"
            observations=row.get("observations",[]) if committed else []
            correct=sum(assertion_correct(o,truth,case,directory) for o in observations)
            # Source-only exact facts may commit without model assertions; count one
            # assertion bound to the oracle's independent geometry outcome.
            if committed and row.get("source_only"):
                assertion_count=1;correct=int(outcome==truth["expected_outcome"])
            else:assertion_count=len(observations)
            if committed and not assertion_count:
                committed=False
            relevant=task_evidence(observations,case,truth,directory) or bool(row.get("source_only") and source_fact(case) and outcome=="observed")
            primary_correct=committed and relevant and outcome==truth["expected_outcome"] and correct==assertion_count
            child_correct=True
            if child:
                child_case=dict(case,after=case["counterfactual"]["path"])
                child_truth=dict(truth,rendered_witness=truth["counterfactual_witness"])
                child_committed=child.get("complete",False) and child.get("outcome")!="unverifiable"
                child_observations=child.get("observations",[]) if child_committed else []
                child_correct_assertions=sum(assertion_correct(o,child_truth,child_case,directory) for o in child_observations)
                child_correct=child_committed and child.get("outcome")=="observed" and bool(child_observations) and child_correct_assertions==len(child_observations) and task_evidence(child_observations,child_case,child_truth,directory)
                assertion_count+=len(child_observations);correct+=child_correct_assertions
            case_correct=primary_correct and child_correct
            important=truth["important"]
            reassurance=important and committed and outcome!=truth["expected_outcome"]
            # Invalid geometry that was withheld is not an authority violation;
            # report failures separately while keeping unavailable execution.
            receipts=row.get("provenance",[])+(child.get("provenance",[]) if child else [])
            cost=(execution_cost(receipts,complete) if receipts else 0 if row and complete and not stages(case,arm,row) else None) if row else None
            item=dict(root=case_id,category=truth["category"],family=case["family"],
                mutation=str(int(case["family"].split("-")[1])%6),label_strength="exact generated glyph witness",
                visibility="full" if truth["rendered_witness"]["label_complete"] else "partial" if truth["rendered_witness"]["label_pixels"] else "absent",
                condition_kind=(case.get("condition") or {}).get("kind",case["task"]),eligible=truth["category"]!="unavailable",important=important,
                necessary=truth["necessary_vision"],complete=complete,committed=committed,case_correct=case_correct,
                detected=case_correct,reassurance=reassurance,assertions=assertion_count,correct_assertions=correct,
                abstention=complete and outcome=="unverifiable",mechanical_valid=row.get("mechanical_valid",False) and (not child or child.get("mechanical_valid",False)),
                order_disagreement=row.get("order_disagreement",False) or bool(child and child.get("order_disagreement",False)),contradiction_withheld=row.get("contradiction_withheld",True) and (not child or child.get("contradiction_withheld",True)),
                paired_order=case["task"]!="check_ui" and arm in ("two_gemini","two_gemini_jev","cascade","cascade_jev_route","two_openrouter") and complete and committed and row.get("used_vision",False),
                routing_error=truth["necessary_vision"] and arm in ("cascade","cascade_jev_route") and (not row.get("used_vision",False) or not row.get("counterfactual_checked",False) or bool(child and not child.get("used_vision",False))),
                cost=cost,provenance=receipts,elapsed_ms=row.get("elapsed_ms",0),queue_delay_ms=row.get("queue_delay_ms",0)+(child.get("queue_delay_ms",0) if child else 0),retries=row.get("retries",0)+(child.get("retries",0) if child else 0))
            grouped[(case["workload"],arm)].append(item)
    report={"schema":SCHEMA,"epoch":manifest["epoch"],"manifest_hash":manifest["manifest_hash"],"oracle_hash":manifest["oracle_hash"],
        "policy":POLICY,"models":manifest["models"],"mechanical_fixture_receipt":str(gate_receipt) if gate_receipt else None,"synthetic_domain_only":True,"execution_dialect":"synthetic-offline/1","live_model_qualification":False,"revision_identity":"alias-bound and time-specific; no immutable revision claim","human_labels_used":False,
        "exclusions":manifest["exclusions"],"results_hash":digest(result_file.read_bytes()) if result_file and result_file.exists() else None,
        "workloads":{},"feature_decisions":{},"limitations":["Finite preregistered assertion vocabulary; unparsed natural descriptions count as unsupported.","Assertion confidence uses one all-facts-correct event per root; family block bootstrap and per-family gates are reported separately.","No claim of arbitrary real-world image accuracy."]}
    for workload in WORKLOADS:
        report["workloads"][workload]={}
        for arm in ARMS:
            items=grouped[(workload,arm)];metrics=summarize(items)
            families={family:summarize([i for i in items if i["family"]==family]) for family in sorted({i["family"] for i in items})}
            clusters=cluster(items)
            classes=Counter(oracle[i["root"]]["category"] for i in items)
            corpus_complete=metrics["roots"]>=1000 and classes==Counter(POLICY["classes"])
            checks=gates(metrics,clusters,families,corpus_complete)
            checks["authority_and_input_safety"]=checks["authority_and_input_safety"] and mechanical_fixtures
            if workload in ("check_ui","routing") or arm in ("rules","oracle_jev","single_gemini"):
                checks["order_handling"]=True # explicitly not a paired-image arm/workload
            report["workloads"][workload][arm]={"metrics":metrics,"family_metrics":families,"strata":{field:{value:summarize([i for i in items if i[field]==value]) for value in sorted({i[field] for i in items})} for field in ("mutation","label_strength","visibility","condition_kind")},"category_metrics":{k:summarize([i for i in items if i["category"]==k]) for k in ("challenge","control","unavailable")},"clustered_uncertainty":clusters,"gates":checks,"decision":"qualified" if all(checks.values()) else "unqualified"}
        report["feature_decisions"][workload]={"status":report["workloads"][workload]["cascade"]["decision"],"failed_gates":[k for k,v in report["workloads"][workload]["cascade"]["gates"].items() if not v]}
    jev_checks=[];routing_checks=[]
    for workload in WORKLOADS:
        base=report["workloads"][workload]["two_gemini"]["metrics"];jev=report["workloads"][workload]["two_gemini_jev"]["metrics"];routed=report["workloads"][workload]["cascade"]["metrics"]
        reduction=1-fraction(jev["unsupported_assertions"],base["unsupported_assertions"]) if base["unsupported_assertions"] else None
        jev_checks.append(mechanical_fixtures and all(report["workloads"][workload]["two_gemini_jev"]["gates"].values()) and reduction is not None and reduction>=.30 and base["coverage"]-jev["coverage"]<=.05)
        cost_reduction=1-routed["cost_usd"]/jev["cost_usd"] if routed["cost_usd"] is not None and jev["cost_usd"] else None
        a=grouped[(workload,"two_gemini_jev")];b=grouped[(workload,"cascade")]
        matched=min(sum(i["committed"] for i in a),sum(i["committed"] for i in b))
        def recall_at(items):
            kept=sorted([i for i in items if i["committed"]],key=lambda i:hashlib.sha256(i["root"].encode()).digest())[:matched]
            return fraction(sum(i["important"] and i["detected"] for i in kept),sum(i["important"] for i in items))
        loss=recall_at(a)-recall_at(b)
        routed["matched_committed_roots"]=matched;routed["matched_coverage_recall_loss"]=loss;routed["cost_reduction"]=cost_reduction
        necessary_gate=(routed["routing_error_upper99"]<=.01) if workload=="routing" else True
        routing_checks.append(mechanical_fixtures and all(report["workloads"][workload]["cascade"]["gates"].values()) and matched>=600 and cost_reduction is not None and cost_reduction>=.20 and loss<=.02 and necessary_gate)
        jev["unsupported_reduction"]=reduction
    report["feature_decisions"]["jev_support"]={"status":"qualified" if all(jev_checks) else "unqualified","value_gate_by_workload":dict(zip(WORKLOADS,jev_checks))}
    report["feature_decisions"]["routing"]={"status":"qualified" if all(routing_checks) else "unqualified","value_gate_by_workload":dict(zip(WORKLOADS,routing_checks))}
    optional_checks=[]
    for workload in WORKLOADS:
        baseline=report["workloads"][workload]["cascade"]["metrics"]
        routed=report["workloads"][workload]["cascade_jev_route"]["metrics"]
        base_items=grouped[(workload,"cascade")];route_items=grouped[(workload,"cascade_jev_route")]
        matched=min(sum(i["committed"] for i in base_items),sum(i["committed"] for i in route_items))
        def recall_matched(items):
            kept=sorted([i for i in items if i["committed"]],key=lambda i:hashlib.sha256(i["root"].encode()).digest())[:matched]
            return fraction(sum(i["important"] and i["detected"] for i in kept),sum(i["important"] for i in items))
        loss=recall_matched(base_items)-recall_matched(route_items)
        saving=1-routed["cost_usd"]/baseline["cost_usd"] if routed["cost_usd"] is not None and baseline["cost_usd"] else None
        routed["incremental_cost_reduction"]=saving;routed["matched_coverage_recall_loss"]=loss;routed["matched_committed_roots"]=matched
        optional_checks.append(mechanical_fixtures and all(report["workloads"][workload]["cascade_jev_route"]["gates"].values()) and matched>=600 and saving is not None and saving>=POLICY["routing_cost_reduction_min"] and loss<=POLICY["routing_matched_coverage_recall_loss_max"] and routed["routing_error_upper99"]<=POLICY["necessary_evidence_skip_upper99_max"])
    report["feature_decisions"]["jev_evidence_routing"]={"status":"qualified" if all(optional_checks) else "unqualified","default_enabled":False,"baseline_arm":"cascade","candidate_arm":"cascade_jev_route","value_gate_by_workload":dict(zip(WORKLOADS,optional_checks))}
    report["feature_decisions"]["blind_orders"]={"status":"qualified" if mechanical_fixtures and all(report["workloads"][w]["two_gemini"]["gates"]["order_handling"] and report["workloads"][w]["two_gemini"]["gates"]["availability"] for w in ("explain","audit_mask")) else "unqualified"}
    return report

if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("--corpus",type=Path,required=True);parser.add_argument("--results",type=Path);parser.add_argument("--gate-receipt",type=Path);parser.add_argument("--out",type=Path,required=True)
    args=parser.parse_args()
    try:
        report=evaluate(args.corpus,args.results,args.gate_receipt);put(args.out,report)
        print(json.dumps({"schema":SCHEMA,"feature_decisions":report["feature_decisions"]}))
    except (ValueError,KeyError,OSError) as error:
        print("constructed scoring refused: "+str(error),file=sys.stderr);sys.exit(4)
