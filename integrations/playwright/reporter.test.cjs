const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const Reporter = require('./reporter.cjs');

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'saccade-playwright-reporter-'));
try {
  const outside = path.join(root, 'outside');
  const results = path.join(root, 'results');
  fs.mkdirSync(outside);
  fs.mkdirSync(results);
  for (const role of ['expected', 'actual', 'diff']) {
    fs.writeFileSync(path.join(outside, `${role}.png`), role);
  }
  const reporter = new Reporter({ outputFile: path.join(results, 'manifest.json') });
  reporter.onTestEnd({ id: 'case', parent: { project: () => ({ name: 'chromium', use: { browserName: 'chromium' } }) } }, {
    retry: 0,
    attachments: [...['expected', 'actual', 'diff'].map(role => ({ name: `card-${role}.png`, contentType: 'image/png', path: path.join(outside, `${role}.png`) })), {name: 'saccade-ui-sources-0', contentType: 'application/json', body: Buffer.from(JSON.stringify([{schema:'saccade-ui-source.v1',nodes:[{id:'price',text:'€19.99'}]},{schema:'saccade-ui-source.v1',nodes:[{id:'price',text:'€79.99'}]}]))}]
  });
  reporter.onEnd();
  const manifest = JSON.parse(fs.readFileSync(path.join(results, 'manifest.json')));
  assert.equal(manifest.entries.length, 1);
  assert.equal(manifest.entries[0].ui_sources[0].nodes[0].text, "€19.99");
  assert.equal(manifest.entries[0].ui_sources[1].nodes[0].text, "€79.99");
  for (const role of ['expected', 'actual', 'diff']) {
    const relative = manifest.entries[0][role];
    assert.equal(path.isAbsolute(relative), false);
    assert.equal(fs.readFileSync(path.join(results, relative), 'utf8'), role);
  }
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}
