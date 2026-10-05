const fs = require('node:fs');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { initialize, capture, sleep } = require('./stabilize.cjs');
const SCHEMA = 'saccade-playwright-matcher.v1';
const MAX_JSON = 8 * 1024 * 1024;
function boundedJson(filename) {
  if (fs.statSync(filename).size > MAX_JSON) throw new Error('JSON exceeds 8 MiB');
  return JSON.parse(fs.readFileSync(filename, 'utf8'));
}
function resolveOptions(info, supplied) {
  const configured = info.project?.use?.saccade || {};
  let options = { ...configured, ...supplied };
  if (options.projectConfig) {
    const document = boundedJson(options.projectConfig);
    options = { ...document.defaults, ...document.projects?.[info.project.name], ...options };
    if (options.profile) {
      if (!document.profiles?.[options.profile]) throw new Error('unknown Saccade profile');
      options = { ...document.profiles[options.profile], ...options };
    }
  } else if (options.profile) throw new Error('named profile requires projectConfig');
  if (options.threshold !== undefined && (!Number.isFinite(options.threshold) || options.threshold < 0 || options.threshold > 1)) throw new Error('threshold must be in [0,1]');
  if (options.metric && !['mean', 'p95', 'p99', 'max'].includes(options.metric)) throw new Error('invalid metric');
  for (const mask of options.masks || []) {
    if (!mask.reason?.trim() || (!!mask.selector === !!mask.rect)) throw new Error('each mask needs a reason and exactly one selector or rectangle');
    if (mask.rect && (mask.rect.length !== 4 || mask.rect.some(v => !Number.isFinite(v) || v < 0 || v > 1) || mask.rect[2] <= 0 || mask.rect[3] <= 0 || mask.rect[0] + mask.rect[2] > 1 || mask.rect[1] + mask.rect[3] > 1)) throw new Error('mask rectangles are fractional [x,y,width,height] within the image');
  }
  return options;
}
function runCli(binary, args, timeout = 60000) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { shell: false, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', count = 0, overflow = false;
    const timer = setTimeout(() => { child.kill(); reject(new Error('Saccade timed out')); }, timeout);
    child.on('error', error => { clearTimeout(timer); reject(error); });
    child.stdout.on('data', bytes => { count += bytes.length; if (count > MAX_JSON) { overflow = true; child.kill(); } else stdout += bytes; });
    // Drain errors; CLI error output may contain input URLs, so do not echo it.
    child.stderr.resume();
    child.on('close', code => {
      clearTimeout(timer);
      if (overflow) return reject(new Error('Saccade output exceeds limit'));
      try { const result = JSON.parse(stdout); resolve({ code, result }); }
      catch { reject(new Error(`Saccade returned invalid JSON (exit ${code})`)); }
    });
  });
}
async function exclusions(target, masks) {
  const page = typeof target.page === 'function' ? target.page() : target;
  let frame;
  if (target !== page) frame = await target.boundingBox();
  else frame = await page.evaluate(() => ({ x: scrollX, y: scrollY, width: innerWidth, height: innerHeight }));
  if (!frame) throw new Error('capture target has no box');
  const out = [];
  for (const mask of masks || []) {
    if (mask.rect) { out.push({ ...mask }); continue; }
    const locator = page.locator(mask.selector);
    if (await locator.count() === 0) throw new Error('mask selector matched no elements');
    for (const element of await locator.all()) {
      const box = await element.boundingBox();
      if (!box) throw new Error('mask element is not visible');
      const offset = target === page ? await page.evaluate(() => ({ x: scrollX, y: scrollY })) : { x: 0, y: 0 };
      const x = Math.max(0, (box.x + offset.x - frame.x) / frame.width);
      const y = Math.max(0, (box.y + offset.y - frame.y) / frame.height);
      const right = Math.min(1, (box.x + offset.x + box.width - frame.x) / frame.width);
      const bottom = Math.min(1, (box.y + offset.y + box.height - frame.y) / frame.height);
      if (right <= x || bottom <= y) throw new Error('mask selector outside capture');
      out.push({ selector: mask.selector, reason: mask.reason, rect: [x, y, right - x, bottom - y] });
    }
  }
  return out;
}
async function compareFiles(expected, actual, out, options, declared = []) {
  const baseline = path.join(out, 'baseline'), candidate = path.join(out, 'candidate'), report = path.join(out, 'report');
  fs.mkdirSync(baseline, { recursive: true }); fs.mkdirSync(candidate, { recursive: true });
  fs.copyFileSync(expected, path.join(baseline, 'capture.png')); fs.copyFileSync(actual, path.join(candidate, 'capture.png'));
  const args = ['compare', baseline, candidate, '--out', report, '--json', '--fail-on-new'];
  if (options.metric) args.push('--metric', options.metric);
  if (options.threshold !== undefined) args.push('--threshold', String(options.threshold));
  if (declared.length) {
    let text = options.config ? fs.readFileSync(options.config, 'utf8') : '';
    if (Buffer.byteLength(text) > MAX_JSON) throw new Error('config exceeds limit');
    // Relative mask-image paths must retain their original base. Combining those
    // with generated masks would silently rebase them, so refuse explicitly.
    if (/^\s*image\s*=/m.test(text)) throw new Error('use rectangle masks when combining config and matcher exclusions');
    text += declared.map(mask => `\n[[mask]]\nrect = ${JSON.stringify(mask.rect)}\n`).join('');
    const config = path.join(out, 'saccade.toml'); fs.writeFileSync(config, text); args.push('--config', config);
  } else if (options.config) args.push('--config', options.config);
  const response = await runCli(options.binary || process.env.SACCADE_BIN || 'saccade', args, options.cliTimeout);
  const pass = response.code === 0 && response.result.schema === 'saccade-result.v2' && response.result.verdict === 'pass' && response.result.counts?.error === 0 && response.result.counts?.missing === 0 && response.result.counts?.new === 0 && response.result.counts?.total > 0;
  return { ...response, pass, report };
}
async function attachReport(info, report, prefix = 'saccade') {
  for (const [name, file, contentType] of [['report', 'index.html', 'text/html'], ['json', 'saccade-report.v1.json', 'application/json']]) {
    const filename = path.join(report, file);
    if (fs.existsSync(filename)) await info.attach(`${prefix}-${name}`, { path: filename, contentType });
  }
  const jsonFile = path.join(report, 'saccade-report.v1.json');
  if (fs.existsSync(jsonFile)) {
    const document = boundedJson(jsonFile);
    for (const [index, entry] of (document.entries || []).entries()) {
      const heatmap = entry.paths?.heatmap;
      if (heatmap) {
        const filename = path.resolve(report, heatmap);
        if (filename.startsWith(path.resolve(report) + path.sep) && fs.existsSync(filename)) await info.attach(`${prefix}-heatmap-${index}`, { path: filename, contentType: 'image/png' });
      }
    }
  }
}
function createMatcher(getInfo) {
  return async function toMatchSaccade(target, name, supplied = {}) {
    const info = getInfo();
    let state = { schema: SCHEMA, status: 'error', declared_exclusions: [], proposed_masks: [] };
    try {
      if (this.isNot) throw new Error('toMatchSaccade does not support negation: errors must always fail');
      if (typeof name !== 'string' || !name || /[\\/]/.test(name) || name === '..') throw new Error('snapshot name must be a filename');
      if (!name.endsWith('.png')) name += '.png';
      const options = resolveOptions(info, supplied);
      const baseline = info.snapshotPath(name), out = info.outputPath(`saccade-${name}`);
      fs.mkdirSync(out, { recursive: true });
      const actual = path.join(out, 'actual.png');
      const page = typeof target.page === 'function' ? target.page() : target;
      await initialize(page, options); await capture(target, actual, options);
      state.declared_exclusions = await exclusions(target, options.masks);
      if (options.fullPage && state.declared_exclusions.some(m => m.selector)) throw new Error('fullPage selector masks require explicit fractional rectangles');
      await info.attach(`${name}-actual`, { path: actual, contentType: 'image/png' });
      let unstable = false;
      if (options.stabilityCheck) {
        const delay = options.stabilityCheck.delayMs ?? 100;
        if (!Number.isFinite(delay) || delay < 0 || delay > 60000) throw new Error('invalid stability delay');
        await sleep(delay);
        const second = path.join(out, 'stability.png'); await capture(target, second, options);
        const stability = await compareFiles(actual, second, path.join(out, 'stability'), { ...options, config: undefined, threshold: 0 }, []);
        await attachReport(info, stability.report, 'saccade-stability');
        if (![0, 1].includes(stability.code)) throw new Error('stability comparison failed');
        const document = boundedJson(path.join(stability.report, 'saccade-report.v1.json'));
        if (document.entries?.some(e => !['pass', 'fail'].includes(e.status))) throw new Error('stability captures unmeasurable');
        state.proposed_masks = (document.entries || []).flatMap(e => (e.hotspots || []).map(h => ({ rect_px: h.rect_px, reason: 'capture remains dynamic; review manually', applied: false })));
        unstable = !stability.pass;
      }
      if (!fs.existsSync(baseline)) {
        state.status = 'new';
        const update = options.updateSnapshots ?? ['all', 'missing', 'changed'].includes(info.config.updateSnapshots);
        if (update && !unstable) { fs.mkdirSync(path.dirname(baseline), { recursive: true }); fs.copyFileSync(actual, baseline); state.updated = true; }
        state.pass = !!state.updated;
      } else {
        await info.attach(`${name}-expected`, { path: baseline, contentType: 'image/png' });
        const comparison = await compareFiles(baseline, actual, path.join(out, 'comparison'), options, state.declared_exclusions);
        await attachReport(info, comparison.report);
        state.result = comparison.result; state.status = comparison.pass ? 'pass' : 'fail';
        state.pass = comparison.pass && !unstable;
        if (comparison.code !== 0 && comparison.code !== 1) state.status = 'error';
      }
      state.stable = !unstable;
    } catch (error) { state.error = error.message; state.pass = false; }
    await info.attach('saccade-matcher', { body: Buffer.from(JSON.stringify(state)), contentType: 'application/json' });
    // Playwright inverts negated matchers; return true on .not to keep errors failures.
    return { pass: this.isNot ? true : !!state.pass, message: () => `Saccade ${state.status}${state.error ? `: ${state.error}` : ''}${state.stable === false ? ': capture is unstable; inspect proposed masks' : ''}` };
  };
}
function install(expect, test) { expect.extend({ toMatchSaccade: createMatcher(() => test.info()) }); }
module.exports = { install, createMatcher, resolveOptions, runCli, compareFiles, boundedJson, SCHEMA };
