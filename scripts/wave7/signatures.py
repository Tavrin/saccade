#!/usr/bin/env python3
"""Read ONNX protobuf declarations without loading/executing untrusted graph code."""
import os
import json, mmap, pathlib, hashlib
DTYPES={1:'float32',2:'uint8',6:'int32',7:'int64',9:'bool',10:'float16',11:'float64'}
def varint(b,i):
    n=s=0
    while True:
        v=b[i]; i+=1; n|=(v&127)<<s
        if v<128: return n,i
        s+=7
        if s>64: raise ValueError('bad varint')
def fields(b):
    i=0
    while i<len(b):
        k,i=varint(b,i); wire=k&7
        if wire==0: v,i=varint(b,i)
        elif wire==2:
            n,i=varint(b,i); v=b[i:i+n]; i+=n
        elif wire in [1,5]: n=8 if wire==1 else 4; v=b[i:i+n]; i+=n
        else: raise ValueError('unsupported protobuf wire')
        yield k>>3,v

def signature(path):
    with path.open('rb') as f, mmap.mmap(f.fileno(),0,access=mmap.ACCESS_READ) as b:
        # Only graph metadata is retained; initializer bytes are not decoded.
        graph=next(v for k,v in fields(b) if k==7)
        result={'inputs':[],'outputs':[],'external_data':[]}; initializers=set()
        for k,v in fields(graph):
            if k in [11,12]:
                info=dict(fields(v)); tensor=dict(fields(dict(fields(info[2]))[1])); shape=[]
                for sk,sv in fields(tensor.get(2,b'')):
                    if sk==1:
                        d=dict(fields(sv)); shape.append(d.get(1,d.get(2,b'?').decode() if isinstance(d.get(2,b'?'),bytes) else '?'))
                result['inputs' if k==11 else 'outputs'].append({'name':info[1].decode(),'dtype':DTYPES.get(tensor[1],str(tensor[1])),'dims':shape})
            elif k==5:
                t=dict(fields(v)); initializers.add(t.get(8,b'?').decode())
                if t.get(14)==1: result['external_data'].append(t.get(8,b'?').decode())
        result['overridable_initializers']=[x for x in result['inputs'] if x['name'] in initializers]
        result['inputs']=[x for x in result['inputs'] if x['name'] not in initializers]
        return result
if __name__=='__main__':
    cache=pathlib.Path(os.environ.get("SACCADE_MODEL_CACHE", pathlib.Path(os.environ.get("XDG_CACHE_HOME", pathlib.Path.home() / ".cache")) / "saccade/models")); out={}
    for e in json.loads((pathlib.Path(__file__).resolve().parent/'receipt.json').read_text()):
        if e['role'] in ['graph','encoder','decoder']:
            path=cache/e['sha256']
            with path.open('rb') as file:
                if path.stat().st_size!=e['bytes'] or hashlib.file_digest(file,'sha256').hexdigest()!=e['sha256']:
                    raise RuntimeError('HARD FAILURE: pin mismatch before graph read')
            out[e['model']+'/'+e['role']]=signature(path)
    print(json.dumps(out,indent=2))
