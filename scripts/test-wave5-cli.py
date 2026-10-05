#!/usr/bin/env python3
"""Heavy coordinator fixture gate: generated PNG/pages and recorded API JSON only."""
import hashlib,json,pathlib,struct,subprocess,sys,tempfile,zlib
BIN=str(pathlib.Path(sys.argv[1]).resolve())
def png():
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
    pixels=b''.join(b'\0'+bytes([40,60,80])*32 for _ in range(32))
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',32,32,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(pixels))+chunk(b'IEND',b'')
def put(path,value):path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value))
def run(*argv,codes=(0,)):
    result=subprocess.run([BIN,*map(str,argv)],capture_output=True,text=True,timeout=120)
    assert result.returncode in codes,(argv,result.returncode,result.stderr)
    if '--json' in argv:return json.loads(result.stdout)
    return result
with tempfile.TemporaryDirectory(prefix='saccade-wave5-')as temp:
    root=pathlib.Path(temp);image=png();sha=hashlib.sha256(image).hexdigest()
    urls=root/'urls.txt';urls.write_text('https://example.com/items/1\nhttps://example.com/items/2\n')
    plan=root/'sweep.json';manifest=run('sweep','plan','--urls',urls,'--before-origin','https://example.org','--after-origin','https://example.com','--samples','1','--viewport','32x32','--out',plan,'--json')
    identifier=manifest['pages'][0]['id'];receipts=[]
    for side in ['before','after']:
        path=root/f'{side}/{identifier}.png';path.parent.mkdir();path.write_bytes(image)
        receipts.append(dict(id=identifier,side=side,status='captured',path=f'{side}/{identifier}.png',sha256=sha,final_url=f'https://example.com/items/1',timing_ms=1,error=None,css_tokens=[]))
    receipt=root/'captures.json';put(receipt,dict(schema='saccade-sweep-captures.v1',manifest_sha256=hashlib.sha256(plan.read_bytes()).hexdigest(),receipts=receipts))
    report=root/'sweep-report';value=run('sweep','compare',plan,'--captures',receipt,'--out',report,'--json');assert value['verdict']=='pass'
    empty=root/'failed-captures.json';put(empty,dict(schema='saccade-sweep-captures.v1',manifest_sha256=hashlib.sha256(plan.read_bytes()).hexdigest(),receipts=[]))
    value=run('sweep','compare',plan,'--captures',empty,'--out',root/'failed-report','--json',codes=(1,));assert len(value['capture_failures'])==2
    history=root/'history';run('history','record',report/'comparison/saccade-report.v1.json','--store',history,'--json')
    value=run('sweep','compare',plan,'--captures',receipt,'--out',root/'last-report','--baseline','last-good','--history-store',history,'--json');assert value['verdict']=='pass'
    value=run('compare',root/'after','--baseline','last-good','--history-store',history,'--out',root/'last-compare','--json');assert value['verdict']=='pass'
    key,node='fixture-file','1:2';frame_id=hashlib.sha256(f'{key}:{node}'.encode()).hexdigest();fixtures=root/'fixtures'
    for kind,data in [('file',{'version':'v1','document':{'children':[{'styles':{'fill':'s1'},'fills':[{'type':'SOLID','color':{'r':1,'g':0,'b':0}}]}]},'styles':{'s1':{'name':'accent'}}}),('styles',{'meta':{'styles':[]}}),('variables',{'fixture_status':403}),('images',{'images':{node:'https://example.com/frame.png'}})]:put(fixtures/key/f'{kind}.json',data)
    (fixtures/'images').mkdir();(fixtures/'images'/f'{frame_id}.png').write_bytes(image)
    mapping=root/'mapping.json';put(mapping,dict(schema='saccade-design-map.v1',frames=[dict(file_key=key,node_id=node,url='https://example.com/',test_name=None,selector=None,viewport=[32,32],css_tokens=[])]))
    pull=root/'pull';value=run('design','pull',mapping,'--out',pull,'--cache',root/'cache','--fixture-dir',fixtures,'--json');assert len(value['degradations'])==1
    put(pull/'captures.json',dict(schema='saccade-design-captures.v1',mapping_sha256=hashlib.sha256(mapping.read_bytes()).hexdigest(),entries=[dict(file_key=key,node_id=node,url='https://example.com/',test_name=None,selector=None,viewport=[32,32],status='captured',path=f'frames/{frame_id}.png',sha256=sha,error=None,css_tokens=[dict(name='accent',color='rgb(255, 0, 0)')])]))
    value=run('design','compare',mapping,'--pull',pull/'pull.json','--captures',pull/'captures.json','--out',root/'design-report','--json');assert value['verdict']=='pass';assert value['token_check']['findings'][0]['colour_bucket']=='under_1'
    (root/'source.png').write_bytes(image);put(root/'tune.json',dict(schema='saccade-imgtune.v1',images=[dict(id='image-1',source='source.png',current='source.png')],widths=[32],formats=['jpeg','webp'],qualities=[50,80,95],target_score=80,butteraugli_ceiling=None,adapter=dict(kind='local')))
    value=run('imgtune','search',root/'tune.json','--out',root/'tune-report.json','--json');assert value['verdict']=='complete';assert (root/'source.png').read_bytes()==image
    mcp_inputs=root/'mcp-inputs';mcp_inputs.mkdir();mcp_urls=mcp_inputs/'urls.txt';mcp_urls.write_bytes(urls.read_bytes())
    rpc=[{'jsonrpc':'2.0','id':1,'method':'tools/list'},{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'saccade_products','arguments':{'operation':'imgtune_audit','artifact':str(mcp_urls),'accept':['image/*'],'out':str(root/'mcp-out/audit.json')}}}]
    (root/'mcp-out').mkdir();server=subprocess.run([BIN,'mcp','--root',str(mcp_inputs),'--out-root',str(root/'mcp-out')],input=''.join(json.dumps(x)+'\n'for x in rpc),capture_output=True,text=True,timeout=30)
    replies=[json.loads(line)for line in server.stdout.splitlines()];assert any(tool['name']=='saccade_products'for tool in replies[0]['result']['tools']);assert replies[1]['result']['isError'] is True;assert 'network_not_authorized' in json.dumps(replies[1])
print('wave5 CLI fixtures: PASS')
