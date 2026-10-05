const {test,expect}=require('@playwright/test');const {runSweep}=require('../sweep.cjs');const fs=require('node:fs'),path=require('node:path'),http=require('node:http');
test('local HTTP sweep retains successful and failed receipts',async({},info)=>{
 const server=http.createServer((req,res)=>{if(req.url==='/missing'){res.writeHead(404);res.end('missing');}else{res.end('<body style="background:#405060">Generated fixture</body>');}});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const origin=`http://127.0.0.1:${server.address().port}`;
 try{const plan=info.outputPath('plan.json');fs.writeFileSync(plan,JSON.stringify({schema:'saccade-sweep.v1',seed:42,pages:[{id:'a'.repeat(64),group:'/',before:origin+'/',after:origin+'/missing',viewport:[320,240]}]}));
 const captures=await runSweep(plan,info.outputPath('captures'),{concurrency:1,perHostDelayMs:0,clock:'2026-01-01T00:00:00Z',randomSeed:42});
 expect(captures.receipts).toHaveLength(2);expect(captures.receipts.filter(r=>r.status==='captured')).toHaveLength(1);expect(captures.receipts.filter(r=>r.status==='error')).toHaveLength(1);
 }finally{await new Promise(r=>server.close(r));}
});
