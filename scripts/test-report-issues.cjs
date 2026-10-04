const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../crates/saccade-core/assets/report.js'), 'utf8');
const definition = source.match(/function isIssue\(e\) \{[^}]+\}/);
assert.ok(definition, 'report issue filter exists');
const isIssue = vm.runInNewContext(`(${definition[0]})`);
const entries = [
  { name: 'ordinary.png', status: 'pass' },
  { name: 'local.png', status: 'pass', pass_with_local_change: true },
  { name: 'failure.png', status: 'fail' },
];
assert.deepEqual(entries.filter(isIssue).map(entry => entry.name), ['local.png', 'failure.png']);
assert.equal(entries.some(isIssue), true);
