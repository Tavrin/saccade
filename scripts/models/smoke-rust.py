#!/usr/bin/env python3
"""Execute an already compiled Rust test binary: one image/model, CPU, <=55 seconds."""
import hashlib, json, os, pathlib, subprocess, sys, time
CACHE=pathlib.Path(os.environ.get('WAVE7_MODEL_CACHE','/mnt/linux-extra/saccade-models'))
MODELS=['grounding-dino-tiny','owlv2-base','efficientsam-ti','yunet-2026may','ultraface-rfb','trustmark']
def main():
    binary=pathlib.Path(os.environ['WAVE7_SMOKE_BIN'])
    env=os.environ.copy()
    if not env.get('WAVE7_RUNTIME_LIBRARY'):raise RuntimeError('explicit provisioned runtime required')
    receipt=dict(binary_sha256=hashlib.file_digest(binary.open('rb'),'sha256').hexdigest(),
      source_head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
      source_dirty=bool(subprocess.check_output(['git','status','--porcelain'],text=True)),
      runtime_library=env['WAVE7_RUNTIME_LIBRARY'],
      runtime_sha256=hashlib.file_digest(open(env['WAVE7_RUNTIME_LIBRARY'],'rb'),'sha256').hexdigest(),
      source_export_parity=False,platform='linux-x64',models=[])
    failed=False
    for model in MODELS:
        env['WAVE7_SMOKE_MODEL']=model; started=time.monotonic()
        try:
            r=subprocess.run(['nice','-n','19',str(binary),
              'wave7::heavy_tests::pinned_single_image_cpu_smoke','--exact','--ignored','--nocapture'],
              env=env,capture_output=True,text=True,timeout=55)
            item=dict(model=model,exit=r.returncode,elapsed_seconds=time.monotonic()-started)
            for line in r.stdout.splitlines():
                if line.startswith('SMOKE '):item['measurement']=json.loads(line[6:])
            if 'measurement' not in item and r.returncode==0:item['exit']='no-smoke-receipt'
            (CACHE/'evidence'/f'{model}-rust-smoke.log').write_text(r.stdout+r.stderr)
        except subprocess.TimeoutExpired:
            item=dict(model=model,exit='timeout55',elapsed_seconds=time.monotonic()-started)
        receipt['models'].append(item)
        passed=item['exit']==0;failed|=not passed
        print(f'GATE rust-{model} {"PASS" if passed else "FAIL"} ({item["elapsed_seconds"]:.2f}s)',flush=True)
        (CACHE/'evidence/rust-smokes.json').write_text(json.dumps(receipt,indent=2)+'\n')
    return int(failed)
if __name__=='__main__':sys.exit(main())
