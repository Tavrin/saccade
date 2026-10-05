// Heavy: generated local pages only; no external content.
const { test, expect } = require('@playwright/test');
const { install } = require('../matcher.cjs');
const { initialize } = require('../stabilize.cjs');
const fs = require('node:fs');
install(expect, test);
test('generated page and locator use perceptual assertions and masks', async ({ page }, info) => {
  await initialize(page, { clock: '2026-01-01T00:00:00Z', randomSeed: 42 });
  await page.setContent('<style>body{margin:0;background:#f0f0f0}div{width:64px;height:64px;background:#305080}</style><div></div>');
  const name = 'generated-page.png';
  try {
    await expect(page).toMatchSaccade(name, { updateSnapshots: true });
    await expect(page).toMatchSaccade(name, { threshold: 0 });
    await page.locator('div').evaluate(node => node.style.background = '#a02030');
    await expect(page).toMatchSaccade(name, { threshold: 0, masks: [{ rect:[0,0,.2,64/240], reason:'declared changing swatch' }] });
    await expect(page.locator('div')).toMatchSaccade('generated-locator.png', { updateSnapshots: true });
    await expect(page.locator('div')).toMatchSaccade('generated-locator.png', { threshold: 0, stabilityCheck: { delayMs: 10 } });
  } finally {
    for(const file of [name,'generated-locator.png']) fs.rmSync(info.snapshotPath(file),{force:true});
  }
});
test('unmasked change and dynamic capture fail', async ({page}, info) => {
  await page.setContent('<body style="margin:0;background:#fff"></body>');
  const name='generated-change.png';
  try {
    await expect(page).toMatchSaccade(name,{updateSnapshots:true});
    await page.evaluate(()=>document.body.style.background='#111');
    await expect(expect(page).toMatchSaccade(name,{threshold:0})).rejects.toThrow(/Saccade fail/);
    await page.evaluate(()=>setInterval(()=>document.body.style.background=document.body.style.background==='rgb(17, 17, 17)'?'#fff':'#111',100));
    await expect(expect(page).toMatchSaccade(name,{threshold:0,stabilityCheck:{delayMs:110}})).rejects.toThrow();
  } finally { fs.rmSync(info.snapshotPath(name),{force:true}); }
});
