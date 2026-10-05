#!/usr/bin/env python3
"""Reproduce official DINOv2-small export and frozen generated, disjoint corpus.
CPU only; downloads are confined to an immutable official HF revision.
"""
import hashlib,json,os,urllib.request
from pathlib import Path
ROOT=Path('/mnt/linux-extra/saccade-models/r2')
REV='ed25f3a31f01632728cabb09d1542f84ab7b0056'
def sha(b): return hashlib.sha256(b).hexdigest()
def save(p,v): p.write_text(json.dumps(v,indent=2,ensure_ascii=False)+'\n')
def main():
 import numpy as np,torch,onnx
 from PIL import Image,ImageDraw
 from transformers import Dinov2Model
 torch.set_num_threads(1);torch.set_num_interop_threads(1);torch.manual_seed(0)
 src=ROOT/'dinov2-source';src.mkdir(parents=True,exist_ok=True)
 meta=json.load(urllib.request.urlopen(f'https://huggingface.co/api/models/facebook/dinov2-small/revision/{REV}?blobs=true'))
 pins={f['rfilename']:f for f in meta['siblings']}
 downloads=[]
 for name in ['README.md','config.json','model.safetensors','preprocessor_config.json']:
  url=f'https://huggingface.co/facebook/dinov2-small/resolve/{REV}/{name}'
  p=src/name
  if not p.exists():
   with urllib.request.urlopen(url) as r,p.open('wb') as out:
    while b:=r.read(1024*1024):out.write(b)
  b=p.read_bytes();h=sha(b)
  expected=pins[name].get('lfs',{}).get('sha256')
  if expected and h!=expected:raise ValueError('upstream LFS SHA mismatch')
  downloads.append(dict(file=name,url=url,sha256=h,bytes=len(b),upstream_lfs_sha256=expected))
 assert 'license: apache-2.0' in (src/'README.md').read_text()
 save(ROOT/'upstream-pins.json',dict(revision=REV,license='Apache-2.0',license_evidence='official immutable README model-card frontmatter',downloads=downloads))
 model=Dinov2Model.from_pretrained(src,local_files_only=True).eval()
 class Pooled(torch.nn.Module):
  def __init__(self,m):super().__init__();self.model=m
  def forward(self,x):return self.model(x).pooler_output
 pooled=Pooled(model).eval()
 size=56;mean=np.array([.485,.456,.406],dtype=np.float32);std=np.array([.229,.224,.225],dtype=np.float32)
 samples=[];images=ROOT/'images';images.mkdir(exist_ok=True)
 # Fit and holdout populations are frozen before inference, no threshold retuning.
 for i in range(8):
  kind=(i//2)%2;variation=i%2;split=i//4
  im=Image.new('RGB',(size,size),'white');d=ImageDraw.Draw(im)
  if kind==0:d.rectangle((8+split,7,46,48),fill=(40+variation*2,95+split,200))
  else:d.ellipse((6,8+split,49,47),fill=(210,65+variation*2,30+split))
  # unique encoded identity without adding a duplicate exact sample
  im.putpixel((0,0),(250-i,255,255));p=images/f'{i:04}.png';im.save(p)
  a=np.asarray(im,dtype=np.float32)/np.float32(255)
  x=np.ascontiguousarray(((a-mean)/std).transpose(2,0,1)[None])
  tensor=torch.from_numpy(x)
  with torch.no_grad():v=pooled(tensor).numpy().reshape(-1)
  v=(v.astype(np.float64)/np.linalg.norm(v.astype(np.float64))).astype(np.float32)
  samples.append(dict(image=f'images/{i:04}.png',sha256=sha(p.read_bytes()),tensor_sha256=sha(x.astype('<f4').tobytes()),embedding=v.tolist()))
 graph=ROOT/'dinov2-small.onnx'
 with torch.no_grad():torch.onnx.export(pooled,tensor,str(graph),input_names=['pixel_values'],output_names=['embedding'],opset_version=17,dynamo=False)
 onnx.checker.check_model(onnx.load(graph))
 gb=graph.read_bytes();gh=sha(gb);(ROOT.parent/gh).write_bytes(gb)
 artifact=dict(role='embedding',url=f'https://example.invalid/local-exports/{gh}.onnx',sha256=gh,bytes=len(gb),license='Apache-2.0',version=REV+':local-onnx-export',format='onnx')
 # Runtime URL remains checkpoint provenance; export is supplied cache-only, never downloaded.
 save(ROOT/'model.json',dict(schema='saccade-embedding-model.v1',family='dinov2-small',artifact=artifact,input='pixel_values',output='embedding',size=[size,size],mean=mean.tolist(),std=std.tolist(),dimensions=384,calibration=None))
 pairs=[]
 for split,start in [('fit',0),('holdout',4)]:
  for a,b,same in [(0,1,True),(2,3,True),(0,2,False),(1,3,False)]:pairs.append(dict(a=start+a,b=start+b,split=split,same_content=same))
 corpus=dict(schema='saccade-embedding-corpus.v1',export_sha256=gh,checkpoint_sha256=downloads[2]['sha256'],source_revision=f'transformers 4.46.3; official HF {REV}; prepare-r2.py',scope='Generated colored rectangle versus ellipse, tiny perturbations; MIT OR Apache-2.0 generated pixels; disjoint fit/holdout; no natural-image semantic or domain accuracy claim',parity_tolerance=.0001,samples=samples,pairs=pairs)
 save(ROOT/'corpus.json',corpus)
 save(ROOT/'export-receipt.json',dict(export_sha256=gh,corpus_sha256=sha((ROOT/'corpus.json').read_bytes()),checkpoint_sha256=downloads[2]['sha256'],script_sha256=sha(Path(__file__).read_bytes()),torch=torch.__version__,numpy=np.__version__,onnx=onnx.__version__,processor='56x56 RGB /255 ImageNet normalization, matches explicit Rust seam; no hidden resize',status='export produced; independent Rust parity and holdout gate pending'))
 print(json.dumps(dict(export_sha256=gh,corpus_sha256=sha((ROOT/'corpus.json').read_bytes()))))
if __name__=='__main__':main()
