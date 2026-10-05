#!/usr/bin/env node
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');
const { initialize, capture, sleep } = require('./stabilize.cjs');
const { boundedJson, compareFiles } = require('./matcher.cjs');
const MANIFEST_SCHEMA = 'saccade-sweep.v1', RECEIPT_SCHEMA = 'saccade-sweep-captures.v1';
function validate(manifest, options) {
  if (manifest.schema !== MANIFEST_SCHEMA || !Array.isArray(manifest.pages) || !manifest.pages.length || manifest.pages.length > 100000) throw new Error('invalid sweep manifest');
  if (!Number.isInteger(options.concurrency) || options.concurrency < 1 || options.concurrency > 16 || !Number.isFinite(options.perHostDelayMs) || options.perHostDelayMs < 0 || options.perHostDelayMs > 60000 || !Number.isFinite(options.timeout) || options.timeout <= 0 || options.timeout > 300000) throw new Error('invalid concurrency, timeout or per-host delay');
  const ids = new Set();
  for (const item of manifest.pages) {
    if (!/^[a-f0-9]{64}$/.test(item.id) || ids.has(item.id) || !Array.isArray(item.viewport) || item.viewport.length!==2 || item.viewport.some(n=>!Number.isInteger(n)||n<8||n>16384)) throw new Error('invalid sweep identity or viewport'); ids.add(item.id);
    for (const side of ['before','after']) { const url=new URL(item[side]); if (!['http:','https:'].includes(url.protocol) || url.username || url.password || url.hash) throw new Error('invalid capture URL'); }
  }
}
function limiter(delay) {
  const slots = new Map();
  return async host => { const now=Date.now(), start=Math.max(now,slots.get(host)||0);slots.set(host,start+delay);await sleep(Math.max(0,start-now)); };
}
async function collectCss(page, tokens) {
  const out=[];
  for (const token of tokens || []) {
    if (!token.selector || !token.name) throw new Error('CSS token requires name and selector');
    const values=await page.locator(token.selector).evaluateAll(nodes=>nodes.map(node=>{const s=getComputedStyle(node);return {color:s.color,font_family:s.fontFamily,font_size:s.fontSize};}));
    for(const value of values) out.push({ name:token.name, ...value });
  }
  return out;
}
async function runSweep(manifestPath, out, supplied = {}, chromiumOverride) {
  const options={concurrency:2,perHostDelayMs:250,timeout:30000,...supplied};
  const manifest=boundedJson(manifestPath);validate(manifest,options);
  // Reserve a new output directory so another run cannot overwrite captures.
  fs.mkdirSync(out,{recursive:false});
  fs.mkdirSync(path.join(out,'before'));fs.mkdirSync(path.join(out,'after'));
  const chromium=chromiumOverride || require('@playwright/test').chromium;
  const browser=await chromium.launch();const receipts=[], rate=limiter(options.perHostDelayMs);
  const jobs=manifest.pages.flatMap(item=>['before','after'].map(side=>({item,side}))); let next=0;
  const worker=async()=>{
    while(next<jobs.length){const {item,side}=jobs[next++];const start=Date.now(); let context;
      const receipt={id:item.id,side,status:'error',path:null,sha256:null,final_url:null,timing_ms:0,error:null,css_tokens:[]};
      try {
        await rate(new URL(item[side]).host);
        context=await browser.newContext({viewport:{width:item.viewport[0],height:item.viewport[1]},storageState:options.storageState});
        const page=await context.newPage(); page.setDefaultTimeout(options.timeout);page.setDefaultNavigationTimeout(options.timeout);
        await initialize(page,options);
        const response=await page.goto(item[side],{waitUntil:'domcontentloaded',timeout:options.timeout});
        if (!response || response.status()>=400) throw new Error('navigation returned an error response');
        const relative=`${side}/${item.id}.png`;await capture(page,path.join(out,relative),options);
        if (options.stabilityCheck) {
          await sleep(options.stabilityCheck.delayMs ?? 100);
          const check=path.join(out,`${side}-${item.id}-stability.png`);
          await capture(page,check,options);
          const comparison=await compareFiles(path.join(out,relative),check,path.join(out,'stability',side,item.id),{...options,config:undefined,threshold:0});
          if(!comparison.pass) throw new Error('unstable capture; proposed regions are in stability report');
        }
        receipt.css_tokens=await collectCss(page,options.cssTokens);
        receipt.path=relative;receipt.sha256=crypto.createHash('sha256').update(fs.readFileSync(path.join(out,relative))).digest('hex');
        const final=new URL(page.url());final.username='';final.password='';final.search='';final.hash='';receipt.final_url=final.toString();receipt.status='captured';
      } catch { receipt.error='capture_failed: navigation, timeout, stabilisation or screenshot failed (details redacted)'; }
      finally { if(context) await context.close().catch(()=>{}); receipt.timing_ms=Date.now()-start;receipts.push(receipt); }
    }
  };
  try { await Promise.all(Array.from({length:options.concurrency},worker)); } finally { await browser.close(); }
  receipts.sort((a,b)=>a.id.localeCompare(b.id)||a.side.localeCompare(b.side));
  const result={schema:RECEIPT_SCHEMA,manifest_sha256:crypto.createHash('sha256').update(fs.readFileSync(manifestPath)).digest('hex'),receipts};
  fs.writeFileSync(path.join(out,'captures.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});return result;
}
if(require.main===module){const [manifest,out,config]=process.argv.slice(2);if(!manifest||!out){process.stderr.write('usage: node sweep.cjs MANIFEST NEW_OUT [OPTIONS.json]\n');process.exitCode=2;}else runSweep(manifest,out,config?boundedJson(config):{}).then(r=>{process.stdout.write(JSON.stringify({schema:RECEIPT_SCHEMA,captured:r.receipts.filter(x=>x.status==='captured').length,failed:r.receipts.filter(x=>x.status!=='captured').length})+'\n');process.exitCode=r.receipts.some(x=>x.status!=='captured')?1:0;}).catch(()=>{process.stderr.write('sweep driver failed (details redacted)\n');process.exitCode=2;});}
module.exports={runSweep,validate,limiter,collectCss};
