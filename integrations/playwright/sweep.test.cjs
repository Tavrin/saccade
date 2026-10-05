const {test}=require('node:test');const assert=require('node:assert/strict');const fs=require('node:fs'),path=require('node:path'),os=require('node:os');
const {runSweep,validate}=require('./sweep.cjs');
test('invalid capture identity, credentials and concurrency are refused',()=>{
 const m={schema:'saccade-sweep.v1',pages:[{id:'a'.repeat(64),before:'https://example.com/',after:'https://example.org/',viewport:[320,240]}]};const opts={concurrency:2,perHostDelayMs:0,timeout:1000};validate(m,opts);assert.throws(()=>validate(m,{...opts,concurrency:17}));m.pages[0].before='https://secret@example.com/';assert.throws(()=>validate(m,opts));
});
test('failed pages retain receipts and captures have hashes',async()=>{
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'saccade-'));try{
 const file=path.join(root,'plan.json');fs.writeFileSync(file,JSON.stringify({schema:'saccade-sweep.v1',seed:42,pages:[{id:'a'.repeat(64),group:'/',before:'https://example.com/',after:'https://example.org/',viewport:[320,240]}]}));
 let running=0,max=0;
 const chromium={launch:async()=>({newContext:async()=>{running++;max=Math.max(max,running);return {newPage:async()=>({setDefaultTimeout(){},setDefaultNavigationTimeout(){},goto:async(url)=>{if(url.includes('example.org'))throw Error('secret');return {status:()=>200};},waitForLoadState:async()=>{},evaluate:async()=>{},screenshot:async(o)=>fs.writeFileSync(o.path,'fixture'),url:()=> 'https://example.com/?token=hidden'}),close:async()=>running--};},close:async()=>{}})};
 const result=await runSweep(file,path.join(root,'out'),{concurrency:1,perHostDelayMs:0},chromium);assert.equal(max,1);assert.equal(result.receipts.length,2);assert.equal(result.receipts[0].status,'error');assert.equal(result.receipts[1].status,'captured');assert.equal(result.receipts[1].final_url,'https://example.com/');assert.equal(result.receipts[1].sha256.length,64);assert.ok(!JSON.stringify(result).includes('secret'));
 }finally{fs.rmSync(root,{recursive:true});}
});
