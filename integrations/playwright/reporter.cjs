// Playwright Reporter API: https://playwright.dev/docs/api/class-reporter
// Configure beside a normal reporter. Every declared test is expected to capture.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

class SaccadeReporter {
  constructor(options = {}) {
    this.outputFile = options.outputFile || process.env.SACCADE_PLAYWRIGHT_MANIFEST || 'test-results/saccade-playwright.json';
    this.entries = [];
    this.expected = new Map();
    this.attempts = [];
  }
  printsToStdio() { return false; }
  id(test, project, index = 0) { return `${project}/${test.id}/snapshot-${index}`; }
  onBegin(_config, suite) {
    for (const test of suite.allTests()) {
      const project = test.parent.project()?.name || '';
      const case_id = this.id(test, project);
      this.expected.set(case_id, { case_id, entry: `${case_id}.png`, required: true });
    }
  }
  onTestEnd(test, result) {
    const roles = { expected: [], actual: [], diff: [] };
    for (const attachment of result.attachments) {
      const role = Object.keys(roles).find((candidate) => new RegExp(`(^|[-_.])${candidate}(?:\\.[^.]+)?$`, 'i').test(attachment.name));
      if (role && attachment.path && attachment.contentType.startsWith('image/')) roles[role].push(attachment.path);
    }
    const project = test.parent.project();
    const projectName = project?.name || '';
    const browser = project?.use?.browserName || projectName || 'unknown';
    const viewport = project?.use?.viewport;
    const count = Math.max(roles.expected.length, roles.actual.length, 1);
    for (let i = 0; i < count; i++) {
      const case_id = this.id(test, projectName, i);
      if (!this.expected.has(case_id)) this.expected.set(case_id, { case_id, entry: `${case_id}.png`, required: true });
      const quarantined = (test.annotations || []).some(a => a.type === 'quarantine');
      const state = quarantined ? 'quarantined' : result.status === 'skipped' ? 'skipped' : roles.expected[i] && roles.actual[i] ? 'captured' : 'missing';
      const attempt = { case_id, entry: null, state, capture_sha256: null };
      this.attempts.push(attempt);
      if (state !== 'captured') continue;
      this.entries.push({
        test_id: `${test.id}:${result.retry ?? 0}:${i}`, case_id, project: projectName, browser,
        viewport: viewport && Number.isInteger(viewport.width) && Number.isInteger(viewport.height) ? [viewport.width, viewport.height] : null,
        expected: path.resolve(roles.expected[i]), actual: path.resolve(roles.actual[i]), diff: roles.diff[i] ? path.resolve(roles.diff[i]) : null,
        attempt,
      });
    }
  }
  onEnd() {
    const target = path.resolve(this.outputFile);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    this.entries.sort((a, b) => a.project.localeCompare(b.project) || a.test_id.localeCompare(b.test_id));
    const assets = path.join(path.dirname(target), 'saccade-playwright-assets');
    fs.mkdirSync(assets, { recursive: true });
    this.entries.forEach((entry, index) => {
      const name = `${String(index).padStart(4, '0')}.png`;
      entry.attempt.entry = name;
      this.expected.get(entry.case_id).entry = name;
      entry.attempt.capture_sha256 = crypto.createHash('sha256').update(fs.readFileSync(entry.actual)).digest('hex');
      for (const role of ['expected', 'actual', 'diff']) {
        if (!entry[role]) continue;
        const filename = `${String(index).padStart(4, '0')}-${role}.png`;
        fs.copyFileSync(entry[role], path.join(assets, filename));
        entry[role] = `saccade-playwright-assets/${filename}`;
      }
      delete entry.attempt;
    });
    const inventory = { schema: 'saccade-inventory.v1', expected: [...this.expected.values()].sort((a, b) => a.case_id.localeCompare(b.case_id)), supplied: this.attempts };
    fs.writeFileSync(target, JSON.stringify({ schema: 'saccade-playwright.v1', entries: this.entries, inventory }, null, 2) + '\n');
  }
}
module.exports = SaccadeReporter;
