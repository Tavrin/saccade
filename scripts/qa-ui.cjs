// Local visual acceptance: start the CLI fixture server and Chrome with --disable-gpu
// --remote-debugging-port first. No browser download or network access is needed.
const { chromium } = require(process.env.FLIPDIFF_PLAYWRIGHT || 'playwright');
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const staticBase = process.env.FLIPDIFF_STATIC_URL || 'http://127.0.0.1:18879';
const serveBase = process.env.FLIPDIFF_SERVE_URL || 'http://127.0.0.1:18878';
const out = process.env.FLIPDIFF_SHOTS || '/mnt/linux-extra/moss-cargo-targets/codex-flipdiff-u1-shots';
const results = { pages: [], checks: [], errors: [] };
const pause = ms => new Promise(r => setTimeout(r, ms));
(async () => {
  fs.mkdirSync(out, { recursive: true });
  const browser = await chromium.connectOverCDP(process.env.FLIPDIFF_CDP || 'http://127.0.0.1:19333');
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, colorScheme: 'light' });
  // Observe the real page registry, without changing registered actions or their behavior.
  await context.addInitScript(() => {
    let ui;
    Object.defineProperty(window, '__flipdiffUI', { get: () => ui, set: value => {
      ui = value;
      let use;
      Object.defineProperty(ui, 'use', { get: () => use, set: fn => {
        use = reg => { window.__qaActions = reg; return fn(reg); };
      } });
    } });
  });
  const page = await context.newPage();
  page.on('pageerror', e => results.errors.push({ url: page.url(), error: e.message }));
  page.on('response', r => { if (r.status() >= 400 && !r.url().endsWith('/favicon.ico') && !(r.status() === 404 && /\/api\/session\/[^/]+\/decisions$/.test(r.url()))) results.errors.push({ url: r.url(), status: r.status() }); });
  await page.goto(serveBase + '/compare?runs=reference,candidate,tone-fit&set=sphere_shadow.png&a=reference&b=candidate');
  await page.waitForURL(/\/session\//, { timeout: 60000 });
  await page.waitForFunction(() => !!window.flipdiff);
  const session = page.url();
  // Populate this isolated inbox solely to inspect a representative existing page.
  await page.evaluate(async () => {
    const r = await fetch('/api/inbox', { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Flipdiff-Token': window.FLIPDIFF_SERVE.token }, body: JSON.stringify({ question: 'Is the softer shadow an intended lighting change?', allowed_answers: ['accept', 'reject', 'needs-work'], context: 'Candidate reduces frame time from 12.0 ms to 10.4 ms. Review the center and bottom-center hotspots.', link: '/compare?runs=reference,candidate#entry=sphere_shadow.png', from: 'Visual review' }) });
    if (!r.ok) throw new Error(await r.text());
  });
  const pages = [
    ['report', staticBase + '/report/index.html', { entry: 'sphere_shadow.png' }],
    ['view', staticBase + '/view/index.html', { set: 'sphere_shadow.png', layout: 'side' }],
    ['blind', staticBase + '/blind/index.html', { set: 'sphere_shadow.png', layout: 'swipe' }],
    ['serve-landing', serveBase + '/'],
    ['serve-browse', serveBase + '/#reference'],
    ['serve-session', session],
    ['runs', staticBase + '/runs/index.html'],
    ['inbox', serveBase + '/inbox'],
    ['diagnostics', staticBase + '/diagnostics/index.html', { entry: 'lighting.exr', layout: 'diff', signed: 1, mask: true }],
    ['hdr-view', staticBase + '/hdr-view/index.html', { set: 'lighting.exr', layout: 'diff', signed: 1, mask: true }]
  ];
  for (const [width, scheme] of [[1440, 'light'], [1440, 'dark'], [2560, 'light'], [2560, 'dark'], [390, 'light'], [390, 'dark']]) {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 1000 });
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
    for (const [name, url, state] of pages) {
      await page.goto(url);
      if (state) await page.evaluate(s => window.flipdiff.set(s), state);
      if (name === 'serve-browse') {
        // The archive uses a path query inside its hash; follow a real run control.
        await page.waitForSelector('#runbox:not([hidden])');
      }
      await pause(200);
      await page.evaluate(async () => {
        await Promise.race([Promise.all([...document.images].filter(i => i.getClientRects().length).map(i => i.decode().catch(() => {}))), new Promise(resolve => setTimeout(resolve, 3000))]);
      });
      // Test 1: every actual page embeds tokens/components, fits its viewport and respects theme.
      const geometry = await page.evaluate(() => ({ width: document.documentElement.scrollWidth, viewport: innerWidth, bg: getComputedStyle(document.documentElement).getPropertyValue('--bg').trim(), shared: [...document.querySelectorAll('style')].some(s => s.textContent.includes('flipdiff design tokens') && s.textContent.includes('flipdiff shared components')), stage: document.querySelector('.stage')?.getBoundingClientRect().toJSON() }));
      assert(geometry.shared, name + ' does not embed the design system');
      assert(geometry.width <= width, name + ' overflows at ' + width);
      if (name === 'report') assert(await page.locator('tr.row').first().evaluate(n => n.getBoundingClientRect().height < 100), 'report name column collapsed');
      if (geometry.stage && geometry.stage.width) assert(geometry.stage.x + geometry.stage.width <= width, name + ' stage clipped at ' + width);
      assert.equal(geometry.bg, scheme === 'light' ? '#f3f4f7' : '#0e1117', name + ' theme');
      const filename = `${name}-${width}-${scheme}.png`;
      await page.screenshot({ path: path.join(out, filename), fullPage: true });
      results.pages.push({ name, width, scheme, screenshot: filename, bytes: Buffer.byteLength(await page.content()), geometry });
      if (width === 1440 && scheme === 'light' && name !== 'inbox') {
        // Test 2: the palette lists every registered action; keyboard search/run and focus work.
        const ids = await page.evaluate(() => window.__qaActions.items().map(a => a.id));
        await page.keyboard.press('Control+k');
        const rendered = await page.locator('.pal-i').evaluateAll(ns => ns.map(n => n.dataset.action));
        assert.deepEqual(rendered, ids, name + ' omitted palette action');
        await page.keyboard.press('ArrowDown');
        await page.keyboard.press('ArrowUp');
        await page.locator('.pal-in').fill('keyboard shortcuts');
        await page.keyboard.press('Enter');
        await page.waitForSelector('.help:not([hidden])');
        await page.keyboard.press('Escape');
        await page.keyboard.press('Meta+k');
        await page.screenshot({ path: path.join(out, `${name}-palette.png`) });
        await page.keyboard.press('Escape');
        assert.equal(await page.locator('.pal:not([hidden])').count(), 0);
        results.checks.push(name + ': palette ' + ids.length + ' actions, keyboard/help');
      }
      if (state && width === 1440 && scheme === 'light') {
        // Test 3: production diagnostic findings, layers, RGB/FLIP and state links render.
        if (name !== 'blind') assert(await page.locator('.diag').count(), name + ' diagnostics missing');
        else assert.equal(await page.locator('.diag').count(), 0, 'blind diagnostics leak');
        const stage = page.locator(name.includes('view') || name === 'blind' ? '.vp' : '.cmp .stage').first();
        await stage.scrollIntoViewIfNeeded();
        const b = await stage.boundingBox();
        await page.mouse.move(b.x + b.width * .45, b.y + b.height * .45);
        await page.waitForFunction(() => [...document.querySelectorAll('.sbar b')].some(b => /\d+ \d+ \d+/.test(b.textContent)));
        if (name !== 'blind') assert.match(await page.locator('.sbar').first().innerText(), /FLIP/);
        if (name === 'diagnostics' || name === 'hdr-view') {
          assert.match(await page.locator('.legend.signed').innerText(), /darker[\s\S]*brighter/);
          assert(await page.locator('.nfb').count(), 'non-finite clusters missing');
          assert.match(await page.locator('.legend.mask').innerText(), /NaN[\s\S]*Inf[\s\S]*negative/);
        }
        if (name === 'report' || name === 'view') {
          await page.evaluate(() => window.flipdiff.set({ layout: 'swipe', signed: .75 }));
          assert.match(await page.locator('.legend.signed').innerText(), /darker[\s\S]*brighter/);
          const display = name === 'view' ? page.locator('#display-btn') : page.getByRole('button', { name: 'Display', exact: true });
          await display.click();
          const pop = page.locator('.pop:not([hidden])');
          const pb = await pop.boundingBox(); assert(pb.x >= 0 && pb.y >= 0 && pb.x + pb.width <= width && pb.y + pb.height <= 1000, 'popover clipped');
          await page.screenshot({ path: path.join(out, name + '-display.png') });
          await page.keyboard.press('Escape');
          if (name === 'view') {
            await page.keyboard.press('f');
            await pause(150);
            assert(await page.locator('#compare-stage.fs-on .sbar').isVisible());
            await page.screenshot({ path: path.join(out, 'view-fullscreen.png') });
            await page.keyboard.press('Escape');
          }
        }
        results.checks.push(name + ': diagnostic/status/layer rendering');
      }
    }
    console.log('captured', width, scheme);
  }
  assert(!results.errors.some(e => e.url.includes('flipdiff-decisions.v1.js')), 'served session requested decision twin');
  assert.deepEqual(results.errors, [], 'browser/network errors');
  fs.writeFileSync(path.join(out, 'qa-results.json'), JSON.stringify(results, null, 2) + '\n');
  await context.close(); await browser.close();
  console.log('PASS: shared pages, all palette actions, diagnostics/status; ' + results.pages.length + ' screenshots');
})().catch(e => { fs.writeFileSync(path.join(out, 'qa-results.json'), JSON.stringify({ ...results, failure: e.stack }, null, 2)); console.error(e); process.exit(1); });
