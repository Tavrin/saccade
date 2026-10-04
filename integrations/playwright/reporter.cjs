// Playwright Reporter API: https://playwright.dev/docs/api/class-reporter
// Screenshot attachments: https://playwright.dev/docs/api/class-testresult#test-result-attachments
// Configure beside a normal reporter: [['list'], ['./integrations/playwright/reporter.cjs']].
const fs = require('node:fs');
const path = require('node:path');

class SaccadeReporter {
  constructor(options = {}) {
    this.outputFile = options.outputFile || process.env.SACCADE_PLAYWRIGHT_MANIFEST || 'test-results/saccade-playwright.json';
    this.entries = [];
  }

  printsToStdio() { return false; }

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
    const pairCount = Math.min(roles.expected.length, roles.actual.length);
    for (let i = 0; i < pairCount; i++) {
      this.entries.push({
        test_id: `${test.id}:${result.retry ?? 0}:${i}`,
        project: projectName,
        browser,
        viewport: viewport && Number.isInteger(viewport.width) && Number.isInteger(viewport.height)
          ? [viewport.width, viewport.height] : null,
        expected: path.resolve(roles.expected[i]),
        actual: path.resolve(roles.actual[i]),
        diff: roles.diff[i] ? path.resolve(roles.diff[i]) : null,
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
      for (const role of ['expected', 'actual', 'diff']) {
        if (!entry[role]) continue;
        const name = `${String(index).padStart(4, '0')}-${role}.png`;
        fs.copyFileSync(entry[role], path.join(assets, name));
        entry[role] = `saccade-playwright-assets/${name}`;
      }
    });
    fs.writeFileSync(target, JSON.stringify({ schema: 'saccade-playwright.v1', entries: this.entries }, null, 2) + '\n');
  }
}

module.exports = SaccadeReporter;
