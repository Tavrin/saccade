const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const { createMatcher, resolveOptions, runCli } = require('./matcher.cjs');
test('profile merges per-project configuration and explicit options', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'saccade-'));
  try { const config = path.join(root, 'config.json'); fs.writeFileSync(config, JSON.stringify({ defaults: { metric: 'mean' }, projects: { desktop: { threshold: .02 } }, profiles: { strict: { threshold: .01, fullPage: true } } }));
    const options = resolveOptions({ project: { name: 'desktop' } }, { projectConfig: config, profile: 'strict', threshold: .03 });
    assert.equal(options.threshold, .03); assert.equal(options.fullPage, true); assert.equal(options.metric, 'mean');
  } finally { fs.rmSync(root, { recursive: true }); }
});
test('masks must have reasons and valid fractional geometry', () => {
  for (const masks of [[{ rect: [0,0,.1,.1] }], [{ reason: 'clock', rect: [.9,0,.2,.1] }], [{reason:'clock',selector:'div',rect:[0,0,.1,.1]}]]) assert.throws(() => resolveOptions({project:{}}, { masks }));
});
test('missing capture fails even with snapshot update', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'saccade-'));
  try { const info = { project: {}, config: {updateSnapshots:'all'}, snapshotPath: n => path.join(root,n), outputPath: n => path.join(root,'out',n), attach: async () => {} };
    const page = { waitForLoadState: async () => {}, evaluate: async () => {}, screenshot: async () => {} };
    const result = await createMatcher(() => info).call({}, page, 'page'); assert.equal(result.pass,false); assert.match(result.message(), /missing capture/); assert.equal(fs.existsSync(path.join(root,'page.png')), false);
  } finally { fs.rmSync(root,{recursive:true}); }
});
test('missing baseline is new and only explicit update writes it', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(),'saccade-'));
  try { let state; const info = { project: {}, config: {updateSnapshots:'none'}, snapshotPath:n=>path.join(root,n), outputPath:n=>path.join(root,'out',n), attach:async(name,data)=> { if(name==='saccade-matcher') state=JSON.parse(data.body); } };
    const page = { waitForLoadState:async()=>{}, evaluate:async()=>({x:0,y:0,width:100,height:100}), screenshot:async o=>fs.writeFileSync(o.path,'fixture') };
    assert.equal((await createMatcher(()=>info).call({},page,'page')).pass,false); assert.equal(state.status,'new');
    assert.equal((await createMatcher(()=>info).call({},page,'page',{updateSnapshots:true})).pass,true); assert.equal(state.updated,true);
  } finally { fs.rmSync(root,{recursive:true}); }
});
test('negated errors never become success', async () => {
  const info = { attach:async()=>{} }; const result = await createMatcher(()=>info).call({isNot:true},{},'page'); assert.equal(result.pass,true);
});
test('CLI errors and malformed output cannot pass', async () => {
  await assert.rejects(runCli(process.execPath,['-e','process.stdout.write("no")']), /invalid JSON/);
  const result = await runCli(process.execPath,['-e','process.stdout.write(JSON.stringify({verdict:"pass"}));process.exitCode=2']); assert.equal(result.code,2);
});
