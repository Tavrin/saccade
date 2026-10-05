# Perceptual Playwright assertions

Use the local package in `integrations/playwright` (no npm publication). Install
`@playwright/test` in the consuming project and use a Saccade binary on PATH:

```js
const { test, expect } = require('@playwright/test');
const { install } = require('/path/to/saccade/integrations/playwright');
install(expect, test);
test('page', async ({ page }) => {
  await page.goto('https://example.com/');
  await expect(page).toMatchSaccade('home', {
    threshold: 0.02, metric: 'p95', fullPage: true,
    masks: [{ rect: [0.8, 0, 0.2, 0.1], reason: 'declared rotating panel' }],
    stabilityCheck: { delayMs: 100 }
  });
});
```

The matcher supports Page and Locator. It uses `test.info().snapshotPath(name)`
(including the project's configured snapshot path template) and attaches capture,
report HTML, heatmap, full comparison JSON and `saccade-playwright-matcher.v1`
evidence. Missing baseline has status `new` and fails unless `updateSnapshots`
is true or Playwright snapshot updates allow new files. Existing baselines are
never rewritten. A missing capture, CLI error, empty comparison, malformed result,
performance-rejected verdict or negated assertion fails.

Options include `binary` (then `SACCADE_BIN`, then PATH), `config` (Saccade TOML),
`projectConfig` (JSON), `profile`, `metric`, `threshold`, `fullPage`, `timeout`,
`cliTimeout`, `clock` (ISO instant), `randomSeed` (integer), `scrollLazyLoad`,
`maxScrollSteps`, `scrollDelayMs` and `stabilityCheck.delayMs`. JSON config supports
`defaults`, `projects` keyed by Playwright project name, and `profiles` keyed by
profile name. Explicit options override defaults and profiles. Per-project options
can also be supplied through `project.use.saccade`.

Capture disables CSS animations/transitions and carets, waits for fonts and
network idle, and optionally scrolls to trigger lazy content with a finite step
limit. For clock/random effects during application initialization, call
`initialize(page, options)` from `stabilize.cjs` **before** navigation; matcher
setup cannot change values already consumed by application code. Network idle is
bounded by the capture timeout; a continuously busy page fails explicitly.

Masks require a nonempty reason and either a selector or a fractional
`[x,y,width,height]` rectangle. Selector masks apply to viewport Page captures and
Locator captures. Full-page captures require explicit rectangles. Combining a
TOML mask-image path and generated exclusions is refused to prevent path rebasing.
The stability comparison has no exclusions: its reported changing regions are
proposed masks with `applied:false`; an unstable capture fails and cannot initialize
a baseline. Review proposals manually before declaring an exclusion.

Browser-free tests: `node --test integrations/playwright/matcher.test.cjs`.
Generated-page browser tests are only run by `scripts/gates-wave5.sh`.

Optional `align` (none/translation/similarity/affine/homography/auto) and `resample` (reference/common) invoke registered comparison and attach geometry evidence. Registration refuses masks/config; stability checks always use raw captures.
