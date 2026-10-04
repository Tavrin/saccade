const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  testDir: __dirname,
  testMatch: 'page.spec.cjs',
  reporter: [['list'], [require.resolve('../reporter.cjs'), { outputFile: 'test-results/saccade-playwright.json' }]],
  projects: [{ name: 'chromium', use: { browserName: 'chromium', viewport: { width: 800, height: 600 } } }],
});
