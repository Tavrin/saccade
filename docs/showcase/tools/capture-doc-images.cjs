// Capture docs/images/report.png, report-detail.png and viewer.png from the bundled demo.
// Usage:
//   saccade demo --out DEMO
//   saccade view DEMO/baseline DEMO/capture --labels baseline,capture --out VIEW
//   PLAYWRIGHT=<path to playwright-core> [CHROMIUM=<chrome binary>] node capture-doc-images.cjs DEMO VIEW docs/images
// The demo images are rendered by saccade's own analytic renderer; no private captures are used.
const path = require('path');
const { chromium } = require(process.env.PLAYWRIGHT || 'playwright-core');

const [demo, view, out] = process.argv.slice(2);
if (!demo || !view || !out) { console.error('usage: capture-doc-images.cjs DEMO VIEW OUT_DIR'); process.exit(2); }
const url = (file, hash) => 'file://' + path.resolve(file) + '#' + hash;
const shots = [
  ['report.png', url(path.join(demo, 'report', 'index.html'), 'entry=sphere_shadow.png&layout=side')],
  ['report-detail.png', url(path.join(demo, 'report', 'index.html'), 'entry=sphere_shadow.png&layout=swipe&signed=0.6')],
  ['viewer.png', url(path.join(view, 'index.html'), 'set=sphere_shadow.png&layout=side')],
];

(async () => {
  const browser = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 1000 }, colorScheme: 'light', deviceScaleFactor: 1 });
  for (const [name, target] of shots) {
    const page = await ctx.newPage();
    await page.goto(target);
    await page.waitForLoadState('networkidle');
    await page.evaluate(() => document.fonts && document.fonts.ready);
    await page.waitForTimeout(400);
    await page.screenshot({ path: path.join(out, name), fullPage: true });
    console.log(name);
    await page.close();
  }
  await browser.close();
})().catch((e) => { console.error(e); process.exit(1); });
