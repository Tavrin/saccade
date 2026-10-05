# Design-source comparison

Build `--features products`. Figma is an adapter behind a generic design-source
interface; implementation mappings and capture receipts are vendor-independent.
Only `~/.config/saccade/figma.env` supplies the `FIGMA_TOKEN` credential. Requests
use `X-Figma-Token`; downloads receive no credential header. Retries are finite for
429/503, with bounded backoff. Redirects fail instead of forwarding credentials.

```sh
saccade design pull mapping.json --out design-pull --json
node integrations/playwright/design-capture.cjs mapping.json design-captures options.json
saccade design compare mapping.json --pull design-pull/pull.json \
  --captures design-captures/captures.json --align translation --out design-report --json
```

Mapping example:

```json
{"schema":"saccade-design-map.v1", "frames":[{
  "file_key":"design-file", "node_id":"1:2", "url":"https://example.com/",
  "test_name":null, "selector":"main", "viewport":[1280,720],
  "css_tokens":[{"name":"accent","selector":"button"}]
}]}
```

A mapping binds frame, implementation URL or Playwright test name, selector and
viewport. The standalone driver captures URL mappings. A test-name driver may
emit the same `saccade-design-captures.v1` receipt, binding the exact mapping hash
and retaining `file_key`, `node_id`, `url`, `test_name`, `selector`, `viewport`,
`status`, relative PNG `path`, encoded `sha256`, `error` and `css_tokens`.
Capture configuration includes concurrency, per-host delay, authentication
storageState, timeout, clock, random seed, lazy scrolling and stability check.

Pull calls document-tree, frame-image, published-style and local-variable endpoints.
A 403 for variables degrades to published styles and is recorded. Pulls cache by
source, file key/version, scale and node selection. Image hashes are checked on
cache hits. Failed exports are recorded and not cached as complete. `--scale` is
0.01–4 (default 1). `--cache` selects a local cache. `--fixture-dir` bypasses
credentials/network and reads recorded `FILE/{file,styles,variables,images}.json`
plus `images/SHA256(file_key:node_id).png`. Fixtures use `fixture_status:403` to
represent unavailable variables; fixture caches are separate from live pulls.

Compare verifies every mapped receipt and artifact hash, retains failures and
writes a normal FLIP report. Static-frame/responsive-page layout differences are
expected and remain visible. `--align translation` enables existing phase-correlation
and compensated-error diagnostics; **raw metrics decide**. `--align none` disables
translation diagnostics. No resampling policy is silently applied; different
image dimensions are explicit comparison errors. Frame export scale/selector and
viewport should be declared to match the intended implementation scope.

Token checks use design colour variables and document nodes referencing published
fill/text styles, matched by token name to driver computed CSS values. Opaque sRGB
`rgb()/rgba()` and six-digit hex are measurable; unsupported/transparent colours
remain unmatched. Variable aliases remain unmeasurable rather than guessed.
ΔE2000 buckets are `<1`, `1–3`, `3–10`, `≥10`, with no universal acceptance claim.
Font family compares the declared primary CSS family; font-size deltas use CSS px.
Missing design/implementation tokens and unavailable values are reported. Computed
CSS does not prove which fallback face rendered. Token output is diagnostic, not
an automatic layout-baseline approval.

JSON uses versioned pull/mapping/capture/report schemas. Exit 0 means complete
pull/all raw comparisons pass; 1 means export/capture/comparison failures; 2 is a
contract/transport/config error. Recorded provider and generated-image tests are
heavy-gated; no live Figma calls run during development or fixture qualification.
