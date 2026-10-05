#!/usr/bin/env python3
"""Pinned official SigLIP 2 CPU export; run in the shared model venv under admission.

Requirements: torch==2.5.1+cpu, transformers==4.46.3, tokenizers==0.20.3,
onnx==1.17.0, onnxruntime==1.22.0, numpy==1.26.4, Pillow==11.0.0.
No pretrained network loaders. Outputs are local, cache-only exports; bands
remain uncalibrated. Text graph uses float16 weights to keep the runtime artifact
below the existing 1 GiB per-artifact bound. Float checkpoint parity must pass.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import urllib.request

REV = '75de2d55ec2d0b4efc50b3e9ad70dba96a7b2fa2'
REPO = 'google/siglip2-base-patch16-224'
PINS = {
    'README.md': '39ac3705d62af9ffa1a14675b8ccb220a75f2d81acd530e564a3b1e3dfe418d8',
    'config.json': 'fe8b5fe6d5734360678fd71c11c21e1ea3364bd8598d34295d9206335973ffd7',
    'preprocessor_config.json': '9b36b57ebaf20f09bf4c22100ccc21877ea6bfe5aead0c00c59f8af8ccefacfc',
    'tokenizer_config.json': '14afe629fe4959b9e0d51e1852b8d9f7ad074f90a1a7125a4fcdd17f06e78fc8',
    'tokenizer.json': 'cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322',
    'model.safetensors': '612923381c76ec5a9bed335d1c48827e3f2e506ac31b044b63b2031fadee6a0b',
}
def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        while b := f.read(1024 * 1024): h.update(b)
    return h.hexdigest()
def save(path, data): path.write_text(json.dumps(data, indent=2) + '\n')
def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--download-only', action='store_true')
    args = parser.parse_args()
    root = Path('/mnt/linux-extra/saccade-models')
    out = root / 'siglip2-base'; src = out / REV; src.mkdir(parents=True, exist_ok=True)
    for name, expected in PINS.items():
        path = src / name
        if not path.exists():
            if shutil.disk_usage(root).free < 25 * 1024**3: raise RuntimeError('disk floor')
            url = f'https://huggingface.co/{REPO}/resolve/{REV}/{name}'
            with urllib.request.urlopen(url, timeout=90) as r, path.with_suffix('.partial').open('wb') as f:
                shutil.copyfileobj(r, f, 1024 * 1024)
            path.with_suffix('.partial').rename(path)
        if digest(path) != expected: raise RuntimeError(f'SHA-256 mismatch: {name}')
    assert 'license: apache-2.0' in (src / 'README.md').read_text()
    save(out / 'source-receipt.json', dict(repository=REPO, revision=REV, license='Apache-2.0',
         license_evidence='immutable official README.md frontmatter', sha256=PINS))
    if args.download_only: return
    os.environ['HF_HUB_OFFLINE'] = '1'
    import importlib.metadata
    versions = {'torch':'2.5.1+cpu','transformers':'4.46.3','tokenizers':'0.20.3','onnx':'1.17.0','onnxruntime':'1.22.0','numpy':'1.26.4','Pillow':'11.0.0'}
    for package, version in versions.items():
        if importlib.metadata.version(package) != version:
            raise RuntimeError(f'export requires {package}=={version}')
    import numpy as np
    import onnx
    import onnxruntime as ort
    import torch
    from transformers import SiglipModel
    from tokenizers import Tokenizer
    from onnxruntime.transformers.float16 import convert_float_to_float16
    torch.set_num_threads(1); torch.set_num_interop_threads(1); torch.manual_seed(0)
    model = SiglipModel.from_pretrained(src, local_files_only=True, attn_implementation='eager').eval()
    class ImageTower(torch.nn.Module):
        def __init__(self): super().__init__(); self.m = model.vision_model
        def forward(self, pixels): return self.m(pixel_values=pixels).pooler_output
    class TextTower(torch.nn.Module):
        def __init__(self): super().__init__(); self.m = model.text_model
        def forward(self, ids): return self.m(input_ids=ids).pooler_output
    image = ImageTower().eval(); text = TextTower().eval()
    pixels = torch.linspace(-1, 1, 3*224*224).reshape(1,3,224,224)
    tok = Tokenizer.from_file(str(src/'tokenizer.json'))
    tok.enable_truncation(max_length=64); tok.enable_padding(length=64, pad_id=0, pad_token='<pad>')
    prompts = ['a red square', 'a blue circle', 'a photograph of a mountain']
    ids = [torch.tensor([tok.encode(t).ids], dtype=torch.int64) for t in prompts]
    ig = out/'image.onnx'; tg = out/'text-f32.onnx'; qg = out/'text-f16.onnx'
    with torch.no_grad():
        if not ig.exists(): torch.onnx.export(image,pixels,str(ig),input_names=['pixel_values'],output_names=['embedding'],opset_version=17,dynamo=False)
        if not tg.exists(): torch.onnx.export(text,ids[0],str(tg),input_names=['input_ids'],output_names=['embedding'],opset_version=17,dynamo=False)
    if not qg.exists():
        converted = convert_float_to_float16(onnx.load(str(tg)), keep_io_types=True)
        onnx.save(converted, str(qg))
    options=ort.SessionOptions(); options.intra_op_num_threads=1; options.inter_op_num_threads=1
    def norm(v): v=v.astype(np.float64); return v/np.linalg.norm(v)
    errors=[]
    for graph, tower, inputs, name in [(ig,image,[pixels],'pixel_values'),(qg,text,ids,'input_ids')]:
        onnx.checker.check_model(str(graph)); sess=ort.InferenceSession(str(graph),sess_options=options,providers=['CPUExecutionProvider'])
        for tensor in inputs:
            with torch.no_grad(): expected=norm(tower(tensor).numpy())
            actual=norm(sess.run(['embedding'],{name:tensor.numpy()})[0])
            err=float(np.max(np.abs(expected-actual))); errors.append(dict(graph=graph.name,max_abs_normalized_error=err))
            print('PARITY',graph.name,err,flush=True)
            assert err <= .02, f'checkpoint/export parity failed: {graph.name}: {err}'
        del sess
    def artifact(p,role,fmt):
        h=digest(p); dest=root/h
        if not dest.exists(): shutil.copyfile(p,dest)
        assert digest(dest)==h
        return dict(role=role,url=f'https://example.invalid/local-exports/{h}',sha256=h,bytes=p.stat().st_size,license='Apache-2.0',version=REV+':local-export',format=fmt)
    contract=dict(schema='saccade-embedding-model.v1',family='siglip2-base',artifact=artifact(ig,'embedding','onnx'),input='pixel_values',output='embedding',size=[224,224],mean=[.5]*3,std=[.5]*3,dimensions=768,calibration=None,
        text=dict(artifact=artifact(qg,'text-embedding','onnx'),tokenizer=artifact(src/'tokenizer.json','tokenizer','tokenizer'),input='input_ids',output='embedding',length=64,pad_id=0))
    save(out/'model.json',contract)
    reg=json.loads((root/'r2/registry.json').read_text())
    reg['contracts']={key:value for key,value in reg['contracts'].items() if value.get('schema') != 'saccade-embedding-model.v1'}
    reg['contracts']['embedding']=contract
    save(out/'registry.json',reg)
    save(out/'export-receipt.json',dict(revision=REV,checkpoint_sha256=PINS['model.safetensors'],parity_tolerance=.02,parity=errors,contract_sha256=digest(out/'model.json'),tool_versions=versions,calibration='uncalibrated',scope='three text prompts and one deterministic image tensor; no retrieval accuracy calibration'))
    print('SigLIP 2 pinned CPU exports and checkpoint parity PASS',flush=True)
if __name__ == '__main__': main()
