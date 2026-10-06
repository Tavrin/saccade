#!/usr/bin/env python3
"""Provision only reviewed immutable PP-OCRv5 pins; no mutable revisions or model guessing."""
import os
import argparse, hashlib, json, pathlib, urllib.request
p=argparse.ArgumentParser();p.add_argument('--cache',type=pathlib.Path,default=pathlib.Path(os.environ.get("SACCADE_MODEL_CACHE", pathlib.Path(os.environ.get("XDG_CACHE_HOME", pathlib.Path.home() / ".cache")) / "saccade/models")));a=p.parse_args()
root=pathlib.Path(__file__).resolve().parents[2];contract=json.loads((root/'crates/saccade-core/assets/paddle-ocr.json').read_text());sources=json.loads((root/'scripts/models/paddle-ocr-provenance.json').read_text())
a.cache.mkdir(parents=True,exist_ok=True)
for pin in [contract[k] for k in ['detection','recognition','dictionary']]+sources['models']+sources['source']:
 rev=pin.get('version',pin.get('revision'));url=pin['url'];assert len(rev)==40 and rev in url
 path=a.cache/pin['sha256']
 if path.exists():data=path.read_bytes()
 else:
  with urllib.request.urlopen(url,timeout=60) as r:data=r.read(pin['bytes']+1)
 if len(data)!=pin['bytes'] or hashlib.sha256(data).hexdigest()!=pin['sha256']:raise SystemExit('artifact SHA-256/size mismatch')
 if not path.exists():
  with path.open('xb') as f:f.write(data)
 print(pin.get('role','licence/source'),pin['bytes'],pin['sha256'])
