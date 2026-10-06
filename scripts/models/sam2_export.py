#!/usr/bin/env python3
"""Official SAM2.1 checkpoint + tagged Apache/MIT sources; offline, CPU FP32 export.

The upstream Microsoft CLI defaults to SAM2.0 configs. We explicitly build SAM2.1
with its matching Tiny config and call the tagged encoder/decoder wrappers directly.
No community archive is silently substituted and no unexported model is registered.
"""
import os
import argparse, hashlib, json, pathlib, shutil, sys, urllib.request
CACHE = pathlib.Path(os.environ.get("SACCADE_MODEL_CACHE", pathlib.Path(os.environ.get("XDG_CACHE_HOME", pathlib.Path.home() / ".cache")) / "saccade/models"))
SOURCE = CACHE / 'sam2-source'
SAM_TAG = 'sam2.1'
ORT_TAG = 'v1.22.0'
CHECKPOINT = 'https://huggingface.co/facebook/sam2.1-hiera-tiny/resolve/36f406a75c9be63c7f429da63246273f028c6fd4/sam2.1_hiera_tiny.pt'
CHECKPOINT_HASH = '7402e0d864fa82708a20fbd15bc84245c2f26dff0eb43a4b5b93452deb34be69'
SAM_FILES = ['LICENSE','README.md','sam2/__init__.py','sam2/build_sam.py',
 'sam2/configs/sam2.1/sam2.1_hiera_t.yaml','sam2/modeling/sam2_base.py',
 'sam2/modeling/backbones/hieradet.py','sam2/modeling/backbones/image_encoder.py',
 'sam2/modeling/backbones/utils.py','sam2/modeling/memory_attention.py',
 'sam2/modeling/memory_encoder.py','sam2/modeling/position_encoding.py',
 'sam2/modeling/sam/prompt_encoder.py','sam2/modeling/sam/mask_decoder.py',
 'sam2/modeling/sam/transformer.py','sam2/modeling/sam2_utils.py','sam2/utils/misc.py']
ORT_FILES = ['image_encoder.py','image_decoder.py','mask_decoder.py','prompt_encoder.py','sam2_utils.py']

def digest(path):
    with path.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()

def fetch(url, dest, maximum, expected=None):
    dest.parent.mkdir(parents=True,exist_ok=True)
    if not dest.exists():
        if shutil.disk_usage(CACHE).free < 25 * 1024**3 + maximum:
            raise RuntimeError('DEFERRED: insufficient headroom')
        used=sum(p.stat().st_size for p in CACHE.rglob('*') if p.is_file())
        if used + maximum > 8_000_000_000: raise RuntimeError('DEFERRED: model cache limit')
        temporary=dest.with_suffix(dest.suffix+'.partial')
        try:
            with urllib.request.urlopen(url,timeout=45) as r, temporary.open('wb') as f:
                total=0
                while chunk:=r.read(1024*1024):
                    total+=len(chunk)
                    if total>maximum:raise RuntimeError('oversized authorized artifact')
                    f.write(chunk)
            if expected and digest(temporary)!=expected:raise RuntimeError('checkpoint integrity mismatch')
            temporary.rename(dest)
        finally:temporary.unlink(missing_ok=True)
    if expected and digest(dest)!=expected:raise RuntimeError('cached checkpoint integrity mismatch')
    return dict(url=url,path=str(dest.relative_to(CACHE)),bytes=dest.stat().st_size,sha256=digest(dest))

def prepare():
    receipt=[]
    frozen=json.loads((pathlib.Path(__file__).parent/'sam2-source-pins.json').read_text())
    pins={entry['url']:entry for entry in frozen['files']}
    # Official README explicitly includes checkpoints in the Apache-2.0 grant.
    for name in SAM_FILES:
        url=f'https://raw.githubusercontent.com/facebookresearch/sam2/{SAM_TAG}/{name}'
        receipt.append(fetch(url,SOURCE/name,pins[url]['bytes'],pins[url]['sha256']))
    for name in ORT_FILES:
        url=f'https://raw.githubusercontent.com/microsoft/onnxruntime/{ORT_TAG}/onnxruntime/python/tools/transformers/models/sam2/{name}'
        receipt.append(fetch(url,SOURCE/'exporter'/name,pins[url]['bytes'],pins[url]['sha256']))
    receipt.append(fetch(CHECKPOINT,CACHE/CHECKPOINT_HASH,200_000_000,CHECKPOINT_HASH))
    path=CACHE/'evidence/sam2-source-pins.json';path.parent.mkdir(exist_ok=True)
    path.write_text(json.dumps(dict(sam_tag=SAM_TAG,exporter_tag=ORT_TAG,files=receipt),indent=2)+'\n')
    print('Prepared official checkpoint and tagged SAM2/ONNX Runtime exporter sources',flush=True)

def export():
    import torch
    torch.manual_seed(0)
    torch.set_num_threads(1)
    torch.set_num_interop_threads(1)
    sys.path[:0]=[str(SOURCE),str(SOURCE/'exporter')]
    from sam2.build_sam import build_sam2
    from image_encoder import export_image_encoder_onnx
    from image_decoder import export_decoder_onnx
    model=build_sam2('configs/sam2.1/sam2.1_hiera_t.yaml',str(CACHE/CHECKPOINT_HASH),device='cpu',apply_postprocessing=False)
    output=CACHE/'sam2-export';output.mkdir(exist_ok=True)
    with torch.inference_mode():
        export_image_encoder_onnx(model,str(output/'encoder.onnx'),False,False,False)
        export_decoder_onnx(model,str(output/'decoder.onnx'),True)
    import onnx
    receipt=[]
    for role in ['encoder','decoder']:
        path=output/(role+'.onnx');graph=onnx.load(str(path),load_external_data=False)
        onnx.checker.check_model(graph)
        if any(t.data_location==onnx.TensorProto.EXTERNAL for t in graph.graph.initializer):
            raise RuntimeError('external data needs a complete manifest; export is not promoted')
        receipt.append(dict(role=role,bytes=path.stat().st_size,sha256=digest(path),
          hash_provenance='exported locally, reproducible by scripts/models/sam2.1-tiny.sh',license='Apache-2.0'))
    (CACHE/'evidence/sam2-export.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps(receipt,indent=2))

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--prepare',action='store_true');p.add_argument('--export',action='store_true');a=p.parse_args()
    if a.prepare:prepare()
    if a.export:export()
