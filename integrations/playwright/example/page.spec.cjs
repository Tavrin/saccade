const { test, expect } = require('@playwright/test');
const { pathToFileURL } = require('node:url');
const path = require('node:path');

test('static card', async ({ page }) => {
  await page.goto(pathToFileURL(path.join(__dirname, 'page.html')).href);
  await expect(page).toHaveScreenshot('card.png');
});
