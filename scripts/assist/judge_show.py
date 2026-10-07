#!/usr/bin/env python3
"""Write a local hand-check sheet of the exact scheduled images, timestamps and rubric."""
import argparse
import html
import json
from pathlib import Path
from video_scores import read, export


def render(rows, scores=None):
    parts=['<!doctype html><meta charset="utf-8"><title>Judge smoke hand check</title>',
           '<style>body{font:16px sans-serif;max-width:1000px;margin:auto}article{border-bottom:1px solid;padding:24px}img{width:384px;image-rendering:auto}pre{white-space:pre-wrap}label{display:block;margin:12px}</style>',
           '<h1>Judge smoke hand check</h1><p>Inspect every order and provider. Record whether the positive, defect, temporal sampling and anchors support the intended comparison. This local form neither sends nor approves anything.</p>']
    for row in rows:
        data=json.loads(row['payload']['messages'][1]['content'][0]['text']);packet=data['packet']
        parts.append('<article><h2>'+html.escape(row['root'])+'</h2><p>'+html.escape(row['model']+' / '+row['revision'])+'</p>')
        parts.append('<pre>'+html.escape(json.dumps(packet,indent=2))+'</pre>')
        if scores is not None:
            parts.append('<h3>Settled scores and identity</h3><pre>'+html.escape(json.dumps([s for s in scores if s['root']==row['root']],indent=2))+'</pre>')
        for part in row['payload']['messages'][1]['content'][1:]:
            url=part['image_url']['url']
            if not url.startswith('data:image/png;base64,') or any(c not in 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=' for c in url.split(',',1)[1]):raise ValueError('inline PNG required')
            parts.append('<img alt="Scheduled sampled evidence" src="'+url+'">')
        parts.append('<label>Hand-check notes <textarea rows="3" cols="80"></textarea></label></article>')
    return '\n'.join(parts)+'\n'


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--requests',type=Path,required=True);parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--results',type=Path,help='optional settled run; export validates provenance')
    a=parser.parse_args()
    with a.out.open('x') as output:output.write(render(read(a.requests),list(export(a.requests,a.results)) if a.results else None))
