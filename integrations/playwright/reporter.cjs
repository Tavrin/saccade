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
    this.sources = new Set();
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
    const snapshots = new Map();
    for (const attachment of result.attachments) {
      const match = /^(?:(.*?)[-_.])?(expected|actual|diff)(?:\.[^.]+)?$/i.exec(attachment.name);
      if (!match || !attachment.path || !attachment.contentType.startsWith('image/')) continue;
      const name = match[1] || 'snapshot-0', role = match[2].toLowerCase();
      if (!snapshots.has(name)) snapshots.set(name, { expected: [], actual: [], diff: [], duplicate: false });
      const roles = snapshots.get(name);
      const stat = fs.statSync(attachment.path);
      const identity = `${stat.dev}:${stat.ino}`;
      if (this.sources.has(identity)) roles.duplicate = true;
      this.sources.add(identity);
      roles[role].push(attachment.path);
    }
    const project = test.parent.project();
    const projectName = project?.name || '';
    const browser = project?.use?.browserName || projectName || 'unknown';
    const viewport = project?.use?.viewport;
    if (snapshots.size) this.expected.delete(this.id(test, projectName));
    const names = snapshots.size ? [...snapshots.keys()].sort() : ['snapshot-0'];
    for (const [i, name] of names.entries()) {
      const roles = snapshots.get(name) || { expected: [], actual: [], diff: [] };
      const case_id = `${projectName}/${test.id}/${encodeURIComponent(name)}`;
      if (!this.expected.has(case_id)) this.expected.set(case_id, { case_id, entry: `${case_id}.png`, required: true });
      const quarantined = (test.annotations || []).some(a => a.type === 'quarantine');
      const state = quarantined ? 'quarantined' : result.status === 'skipped' ? 'skipped' : roles.duplicate || roles.expected.length > 1 || roles.actual.length > 1 || roles.diff.length > 1 ? 'unusable' : roles.expected.length === 1 && roles.actual.length === 1 ? 'captured' : 'missing';
      const attempt = { case_id, entry: null, state, capture_sha256: null };
      this.attempts.push(attempt);
      if (state !== 'captured') continue;
      const geometryAttachment = result.attachments.find(a => a.name === `saccade-dom-regions-${i}`);
      const dom_regions = geometryAttachment ? JSON.parse(geometryAttachment.body ? geometryAttachment.body.toString('utf8') : fs.readFileSync(geometryAttachment.path, 'utf8')) : null;
      const uiAttachments = result.attachments.filter(a => a.name === `saccade-ui-sources-${i}`);
      if (uiAttachments.length > 1) throw new Error('duplicate UI source evidence');
      const uiAttachment = uiAttachments[0];
      const ui_sources = uiAttachment ? JSON.parse(uiAttachment.body ? uiAttachment.body.toString('utf8') : fs.readFileSync(uiAttachment.path, 'utf8')) : null;
      if (ui_sources && (!Array.isArray(ui_sources) || ui_sources.length !== 2)) throw new Error('UI source evidence requires reference and candidate');
      this.entries.push({ dom_regions, ui_sources,
        test_id: `${test.id}:${result.retry ?? 0}:${i}`, case_id, project: projectName, browser,
        viewport: viewport && Number.isInteger(viewport.width) && Number.isInteger(viewport.height) ? [viewport.width, viewport.height] : null,
        expected: path.resolve(roles.expected[0]), actual: path.resolve(roles.actual[0]), diff: roles.diff[0] ? path.resolve(roles.diff[0]) : null,
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
