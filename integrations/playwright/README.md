# Playwright screenshots

Playwright's [`toHaveScreenshot()`](https://playwright.dev/docs/test-snapshots)
stores expected snapshots and, on a visual failure, attaches expected, actual
and diff images to the test result. The [Reporter API](https://playwright.dev/docs/api/class-reporter)
exposes those attachments and the test's project. `reporter.cjs` writes a small
`saccade-playwright.v1` manifest with copies of those attachments beside it. It does not run
tests or alter snapshots.

Add this reporter alongside your normal reporter in `playwright.config.js`:

```js
reporter: [['list'], ['./integrations/playwright/reporter.cjs',
  { outputFile: 'test-results/saccade-playwright.json' }]],
```

Then run the test suite and ingest its manifest, even if Playwright exited 1:

```sh
npx playwright test || test "$?" -eq 1
saccade ingest playwright test-results/saccade-playwright.json --out saccade-playwright --json
```

The ingest command copies expected and actual images into paired directories,
copies Playwright's diff files when present, records exact test ID, project,
browser and viewport in sidecars and `playwright-mapping.json`, then runs the
normal saccade comparison under `saccade-playwright/report`. Input attachment
paths must be relative to the manifest and remain within its directory after
canonicalization. The output directory must
be new or empty. The CLI does not execute JavaScript.

Playwright may omit actual/diff attachments for passing assertions. The
reporter includes only complete expected/actual attachment pairs; an empty
manifest is rejected as no comparison evidence. Multiple failures in one
test receive separate IDs. A custom reporter or Playwright version with
different attachment names can write the same documented manifest directly.

## Tiny static page

From `integrations/playwright/example`, with Playwright installed in your
project, run `npx playwright test --update-snapshots` once to create the
expected image. Change the card colour in `page.html`, run `npx playwright
test`, then run `saccade ingest playwright test-results/saccade-playwright.json
--out comparison --json`. This example needs a locally installed Chromium.
The backlog review lane ran it headlessly with installed Playwright and
Chromium, then ingested its failure attachments without downloading anything.

## CI

The opt-in [GitHub Actions example](ci-example.yml) runs Playwright, ingests
complete screenshot failures and uploads the report. Baseline changes remain
a separate human review step.

The reporter inventories all enumerated tests, not just failing attachment pairs.
Every test declares one required screenshot case. Passed tests must attach their
expected and actual PNG paths explicitly; a pass without both is missing visual
assurance. Skips, `quarantine` annotations and retry attempts remain visible.
Ingest writes `inventory.json`; incomplete required coverage exits 1, independently
of image pass/fail results. Additional snapshots receive separate indexed IDs.
For a predeclared multi-state suite, use the generic `saccade inventory` manifest
in [the wave 2 guide](../../docs/wave2.md).

For source-backed text, disclosure, layout and order review, attach a
`saccade-ui-sources-N` reference/candidate source pair. Ingest checks capture
hashes and dimensions and preserves both in `ui-sources/` and the mapping.
See [UI review](../../docs/ui-review.md) for the producer contract and command.
