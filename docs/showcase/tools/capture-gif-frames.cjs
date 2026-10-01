// Capture the frames of docs/images/showcase-swipe.gif from the built gallery.
// Usage: PLAYWRIGHT=<path to playwright-core> [CHROMIUM=<chrome binary>] node capture-gif-frames.cjs <frames-dir>
// Then: python3 docs/showcase/tools/assemble-gif.py <frames-dir> docs/images/showcase-swipe.gif
const path = require('path');
const fs = require('fs');
const { chromium } = require(process.env.PLAYWRIGHT || 'playwright-core');

const out = process.argv[2];
if (!out) { console.error('usage: capture-gif-frames.cjs <frames-dir>'); process.exit(2); }
fs.mkdirSync(out, { recursive: true });
const page_url = 'file://' + path.resolve(__dirname, '..', 'index.html');

(async () => {
  const browser = await chromium.launch(process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {});
  const ctx = await browser.newContext({ viewport: { width: 1180, height: 900 }, colorScheme: 'light', deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  await page.goto(page_url);
  await page.addStyleTag({ content: '.hero .wrap { grid-template-columns: 1fr !important; } .hero .wrap > div:first-child, .hero-out { display: none !important; } .hero .stg { --maxh: 520px !important; } .div:focus-visible .grip { box-shadow: 0 2px 10px rgba(0,0,0,.45) !important; }' });
  await page.evaluate(() => window.showcase['w-hero'].stopIntro());
  const el = page.locator('#w-hero');
  await el.scrollIntoViewIfNeeded();
  let n = 0;
  const shot = async () => { await el.screenshot({ path: path.join(out, `f${String(n++).padStart(3, '0')}.png`) }); };
  const set = (fn, ...a) => page.evaluate(([f, args]) => window.showcase['w-hero'][f](...args), [fn, a]);
  const ease = (t) => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2);
  const sweep = async (a, b, frames) => { for (let i = 1; i <= frames; i++) { await set('setSplit', a + (b - a) * ease(i / frames)); await shot(); } };
  const hold = async (frames) => { for (let i = 0; i < frames; i++) await shot(); };

  await set('setSplit', 50); await hold(6);
  await sweep(50, 88, 10); await sweep(88, 18, 14); await sweep(18, 60, 10); await hold(6);
  await set('setLayer', 'heat'); await hold(8);
  await sweep(60, 30, 8); await sweep(30, 78, 10); await hold(6);
  await set('setZoom', true);
  for (let i = 0; i < 6; i++) { await page.waitForTimeout(60); await shot(); }
  await hold(6);
  await sweep(78, 40, 10); await set('setLayer', 'cap'); await hold(4); await sweep(40, 68, 10); await hold(10);
  await set('setZoom', false); await set('setLayer', 'cap');
  for (let i = 0; i < 6; i++) { await page.waitForTimeout(60); await shot(); }
  await sweep(68, 50, 6); await hold(6);
  console.log(`${n} frames in ${out}`);
  await browser.close();
})();
