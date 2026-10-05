# Page sweep

Build with `--features products`. Plan a bounded UTF-8 URL list (one URL per
line, `#` comments allowed) or a sitemap URL/file, including sitemap indexes:

```sh
saccade sweep plan --urls urls.txt --before-origin https://example.org \
  --after-origin https://example.com --samples 3 --seed 42 \
  --viewport 1280x720 --viewport 390x844 --out sweep.json --json
node integrations/playwright/sweep.cjs sweep.json captures capture-options.json
saccade sweep compare sweep.json --captures captures/captures.json --out sweep-report --json
```

Numeric and long hex path components become `:id` groups; other path components
stay literal. Queries stay on the capture URLs. Sampling ranks each normalized URL
by SHA-256(seed, URL), independent of input order; viewports multiply the samples.
Limits: 100,000 URLs/pairs, 16 viewports, 128 sitemap documents, depth 4, 32 MiB
combined sitemap data. XML entities are decoded but DTDs are disabled. HTTP uses
bounded bodies and retries for rate limiting; redirects fail explicitly. Planning
never captures a page.

The driver consumes `saccade-sweep.v1`: `seed`, and `pages` with opaque hex `id`,
`group`, `before`, `after`, and `[width,height]` viewport. Any replacement driver
must emit `saccade-sweep-captures.v1`, binding the exact manifest SHA-256 and one
receipt per `(id,side)`. Receipts include `status` (`captured` or `error`), relative
PNG `path`, encoded `sha256`, sanitized `final_url`, `timing_ms`, `error`, and
optional `css_tokens`. Capture output must be a new directory. Comparison verifies
hashes, paths and identities; duplicate or extra receipts are contract errors.
Missing/failed receipts remain failures, including pages missing on both sides.

Driver options: `concurrency` (1–16, default 2), `perHostDelayMs` (default 250),
`storageState` (Playwright authentication file), `timeout` (default 30 seconds), and
[stabilisation options](playwright-matcher.md). Each task has an isolated browser
context; rate limits govern navigation starts per host across workers. Cookies and
storage files are never written into receipts. Error details and URL queries are
redacted. `cssTokens` is a list of `{name,selector}` declarations; collected computed
colour/font values feed design token checks. Static JSON and the normal comparison
HTML are grouped by URL pattern and viewport in the sweep summary.

Exit 0 means all planned pairs passed; 1 means comparison/capture failures; 2 means
an invalid plan, receipt or execution error. `--baseline last-good --history-store
history` selects a content-verified passing history run (see [last-good](last-good.md)).
