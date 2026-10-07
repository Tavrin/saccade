#!/usr/bin/env python3
"""Prepare a complete, credential-free constructed-negative judge schedule."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from judge_calibration import digest
from video_scores import read


def prepare(binary, evidence, positives, rubric, models, revisions, repeats, cap, out, classes=None):
    if len(models)!=len(revisions) or len(set(models))!=len(models) or not 1<=repeats<=32:
        raise ValueError('independent arms and fresh repeats')
    manifest=read(evidence/'manifest.json')
    if len(positives)!=len(manifest['positive_sources']):raise ValueError('positive source denominator')
    for p,pin in zip(positives,manifest['positive_sources']):
        if hashlib.sha256(p.read_bytes()).hexdigest()!=pin['frame_map_sha256']:raise ValueError('positive map identity')
        frame_map=read(p)
        for frame,sha in zip(frame_map['frames'],pin['frame_sha256']):
            if hashlib.sha256((p.parent/frame['file']).read_bytes()).hexdigest()!=sha:raise ValueError('positive frame identity')
        if len(frame_map['frames'])!=len(pin['frame_sha256']):raise ValueError('positive frame denominator')
    entries=[e for e in manifest['entries'] if classes is None or e['class'] in classes]
    if not entries or (classes is not None and {e['class'] for e in entries}!=set(classes)):raise ValueError('degradation class denominator')
    if out.exists():raise ValueError('fresh plan directory required')
    requests=[];costs=[];items=[]
    with tempfile.TemporaryDirectory(prefix='judge-plan-',dir=out.parent) as temp:
        for i,entry in enumerate(entries):
            negative=evidence/entry['id']/'frames.json'
            if hashlib.sha256(negative.read_bytes()).hexdigest()!=entry['generated']['frame_map_sha256']:raise ValueError('negative map identity')
            command=[str(binary),'assist','video-judge','--experimental','--rubric',str(rubric),'--frame-map',str(positives[entry['positive_source']]),str(negative),
                     '--view-id',entry['id']+':positive','--view-id',entry['id']+':negative','--contact-sheet','--fps','120','--max-edge','128',
                     '--repeats',str(repeats),'--max-spend-usd','2','--out',str(Path(temp)/str(i))]
            for model in models:command+=['--model',model]
            for revision in revisions:command+=['--revision',revision]
            subprocess.run(command,check=True,capture_output=True)
            batch=read(Path(temp)/str(i)/'requests.json');plan=read(Path(temp)/str(i)/'plan.json')
            for row,cost in zip(batch,plan['rows']):
                row['root']=entry['id']+'-'+row['root'];cost['root']=row['root']
            requests+=batch;costs+=plan['rows'];items.append(dict(id=entry['id'],requests=batch))
    total=sum(c['reservation_nano_usd'] for c in costs)
    if total>cap:raise ValueError(f'complete schedule exceeds cap: {total} > {cap} nanodollars')
    # Expected cost is a declared scenario, never measured provider usage.
    expected=0
    for c in costs:
        output_price=4500 if c['model']=='openai/gpt-5.4-mini' else 3750
        expected+=(c['input_bound']+1)//2*750+256*output_price
    encoded=json.dumps(requests,indent=2)+'\n'
    if len(encoded.encode())>32*1024*1024:raise ValueError('runner file ceiling')
    out.mkdir()
    (out/'requests.json').write_text(encoded)
    summary=dict(schema='saccade-judge-priced-schedule.v1',calls=len(requests),fresh_repeats=repeats,
                 independent_sources=len(positives),classes=sorted({e['class'] for e in entries}),
                 worst_nano_usd=total,expected_nano_usd=expected,max_spend_nano_usd=cap,rows=costs,
                 expected_rule='scenario only: ceil(input bound / 2) prompt tokens + 256 aggregate completion tokens per call; no measured usage',
                 provider_calls=0,price_id=plan['price_id'],price_source=plan['price_source'],price_date=plan['price_date'])
    (out/'plan.json').write_text(json.dumps(summary,indent=2)+'\n')
    (out/'schedule.json').write_text(json.dumps(dict(schema='saccade-judge-calibration-schedule.v1',manifest_sha256=digest(manifest),items=items),indent=2)+'\n')
    return summary


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin',type=Path,required=True)
    parser.add_argument('--evidence',type=Path,required=True)
    parser.add_argument('--positive',type=Path,action='append',required=True)
    parser.add_argument('--rubric',type=Path,required=True)
    parser.add_argument('--model',action='append',required=True)
    parser.add_argument('--revision',action='append',required=True)
    parser.add_argument('--repeats',type=int,default=1)
    parser.add_argument('--cap-nano-usd',type=int,default=2_000_000_000)
    parser.add_argument('--classes',nargs='+')
    parser.add_argument('--out',type=Path,required=True)
    a=parser.parse_args()
    if not 0<a.cap_nano_usd<=2_000_000_000:raise ValueError('cap must be positive and at most 2 USD')
    result=prepare(a.bin,a.evidence,a.positive,a.rubric,a.model,a.revision,a.repeats,a.cap_nano_usd,a.out,a.classes)
    print(json.dumps({k:v for k,v in result.items() if k!='rows'}))


if __name__=='__main__':main()
