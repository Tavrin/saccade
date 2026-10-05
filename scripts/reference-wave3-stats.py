#!/usr/bin/env python3
"""Regenerate independent mathematical reference values; no timing measurements."""
import numpy as np,json
mask=(1<<64)-1;state=42
before=np.array([10,20,30,40,50,60.]);after=before+np.array([-1,2,2,3,4,5.]);d=after-before;ratios=np.log(after)-np.log(before)
def hl(v):return float(np.median([(a+b)/2 for i,a in enumerate(v) for b in v[i:]]))
def index(n):
 global state
 threshold=((1<<64)-n)%n
 while True:
  state=(state+0x9e3779b97f4a7c15)&mask;z=state
  z=((z^(z>>30))*0xbf58476d1ce4e5b9)&mask;z=((z^(z>>27))*0x94d049bb133111eb)&mask;z^=z>>31
  if z>=threshold:return z%n
bs=[];ls=[]
for _ in range(2048):
 ids=[index(6) for _ in range(6)];bs.append(hl(d[ids]));ls.append(hl(ratios[ids]))
result={'source':'independent NumPy median over all Walsh pairs; linear np.quantile; SplitMix64 index stream; synthetic values','before_ms':before.tolist(),'after_ms':after.tolist(),'seed':42,'resamples':2048,'delta_ms':hl(d),'interval_ms':np.quantile(bs,[.025,.975]).tolist(),'delta_pct':float(np.expm1(hl(ratios))*100),'interval_pct':(np.expm1(np.quantile(ls,[.025,.975]))*100).tolist()}
open(__import__('pathlib').Path(__file__).resolve().parents[1] / 'crates/saccade-core/tests/fixtures/wave3/paired-reference.json','w').write(json.dumps(result,indent=2)+'\n')
print(result)
