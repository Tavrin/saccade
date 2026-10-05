const fs = require('node:fs');
const STYLE = '*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}';
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
function randomScript(seed) {
  let value = seed >>> 0;
  Math.random = () => { value = (value + 0x6D2B79F5) >>> 0; let t = Math.imul(value ^ value >>> 15, value | 1); t ^= t + Math.imul(t ^ t >>> 7, t | 61); return ((t ^ t >>> 14) >>> 0) / 4294967296; };
}
async function initialize(page, options = {}) {
  if (options.randomSeed !== undefined) {
    if (!Number.isInteger(options.randomSeed)) throw new Error('randomSeed must be an integer');
    await page.addInitScript(randomScript, options.randomSeed);
    await page.evaluate(randomScript, options.randomSeed);
  }
  if (options.clock !== undefined) {
    const instant = new Date(options.clock);
    if (!Number.isFinite(instant.getTime())) throw new Error('invalid clock instant');
    if (!page.clock) throw new Error('this Playwright version has no clock API');
    await page.clock.setFixedTime(instant);
  }
}
async function prepare(page, options = {}) {
  const timeout = options.timeout ?? 30000;
  await page.waitForLoadState('networkidle', { timeout });
  await page.evaluate(async () => { await document.fonts.ready; });
  if (options.scrollLazyLoad) {
    const max = options.maxScrollSteps ?? 100;
    for (let step = 0; step < max; step++) {
      const done = await page.evaluate(() => { window.scrollBy(0, Math.max(1, innerHeight)); return scrollY + innerHeight >= document.documentElement.scrollHeight; });
      await sleep(options.scrollDelayMs ?? 50);
      if (done) break;
      if (step === max - 1) throw new Error('lazy-load scroll exceeds maximum steps');
    }
    await page.evaluate(() => scrollTo(0, 0));
    await page.waitForLoadState('networkidle', { timeout });
    await page.evaluate(async () => { await document.fonts.ready; });
  }
}
async function capture(target, filename, options = {}) {
  const page = typeof target.page === 'function' ? target.page() : target;
  await prepare(page, options);
  const screenshotOptions = { path: filename, type: 'png', animations: 'disabled', caret: 'hide', style: STYLE, timeout: options.timeout ?? 30000, scale: 'css' };
  if (target === page) screenshotOptions.fullPage = options.fullPage ?? false;
  await target.screenshot(screenshotOptions);
  if (!fs.existsSync(filename) || fs.statSync(filename).size === 0) throw new Error('missing capture');
  return filename;
}
module.exports = { initialize, prepare, capture, sleep };
