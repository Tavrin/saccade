# flipdiff — spec v1 (frozen for the 2026-10-01 build)

flipdiff is a perceptual visual-regression tool for renderers, game engines and
graphics code. It compares a directory of baseline images with a directory of
fresh captures using NVIDIA FLIP. It writes a JSON report, a self-contained
HTML report and a Markdown summary. It ships as a CLI and a GitHub Action.

The seed is `crates/moss_visual_test/src/lib.rs` in the Moss repo
(`~/Documents/automation_game/assets_toolings/Moss`, MIT). **Do not copy its
defect.** It passes `to_rgba8()` (4 bytes per pixel) to
`nv_flip::FlipImageRgb8::with_data`, which expects tightly packed RGB (3 bytes
per pixel), and so compares misaligned channels. flipdiff converts with
`to_rgb8()`.

## 1. Layout and ownership

| Path | Owner |
|---|---|
| `Cargo.toml` (workspace), `SPEC.md`, `crates/flipdiff-core/src/{lib.rs,report.rs,error.rs}` | coordinator, frozen. A lane that needs a change reports it and does not edit these. Additive `Error` variants are the one exception: lane A may add them. |
| `crates/flipdiff-core/src/{compare.rs,properties.rs,run.rs,config.rs}`, `crates/flipdiff/**`, `crates/flipdiff-core/tests/{compare,run}_*.rs` | lane A |
| `crates/flipdiff-core/src/render/**`, `crates/flipdiff-core/assets/**`, `crates/flipdiff-core/tests/render_*.rs`, `testdata/sample-report/**` | lane B |
| `action.yml`, `.github/**`, `README.md`, `THIRD_PARTY.md`, `LICENSE-MIT`, `LICENSE-APACHE`, `examples/**`, `scripts/**` | lane C |

A lane may add a dependency to its own crate's `Cargo.toml` section, and it
must be MIT, Apache-2.0, BSD, Zlib or ISC. If it is a workspace dependency,
report it and the coordinator merges it. Lane A owns `crates/flipdiff/Cargo.toml`.
Lanes A and B both add to `crates/flipdiff-core/Cargo.toml` `[dependencies]`:
append only, and never reorder or remove existing lines.

## 2. Library: `flipdiff-core`

- `compare::compare(capture: &image::RgbImage, baseline: &image::RgbImage, opts: &CompareOptions) -> Result<Comparison>`
  - `CompareOptions { pixels_per_degree: f32 }`, where the default is
    `nv_flip::DEFAULT_PIXELS_PER_DEGREE`.
  - `Comparison { metrics: report::Metrics, error_map: Vec<f32> /* row-major, w*h */ }`, plus a method
    `heatmap_rgb(&self) -> image::RgbImage`, which applies the magma LUT.
  - It returns `Error::DimensionMismatch` if the dimensions differ and
    `Error::EmptyImage` for a 0×0 image.
  - Percentiles use nearest-rank over the sorted error map. Sort NaN as the
    largest value, through `f32::total_cmp`.
- `properties::validate(img: &image::RgbImage) -> report::Properties` uses
  Rec. 709 luminance on sRGB-encoded bytes / 255. No linearisation; document
  this.
- `run::run(baseline_dir, capture_dir, report_dir, &RunConfig) -> Result<Report>`
  is the whole comparison.
  1. Pair images by path relative to each root (`/`-separated). Recurse, and
     take extensions `png`, `jpg` and `jpeg`, case-insensitive. Apply `ignore`
     globs.
  2. For each pair, decode both. Copy them into
     `<report_dir>/images/<name>/{baseline,capture}.<ext>` and write
     `heatmap.png`. Every image gets its own directory, so nothing collides.
  3. Fill in an `Entry` (see `report.rs`). A decode failure or dimension
     mismatch gives `status = error`, with `error = Some(msg)`, and the copies
     that exist are still recorded.
  4. Write `<report_dir>/flipdiff-report.v1.json` (pretty, entries sorted by
     name), then call `render::render_html`.
  - Only directory, config and report-write failures return `Err`. Per-image
    problems become entries.
- `config::RunConfig` holds `default_threshold` (0.01), `default_metric`
  (`mean`), `pixels_per_degree`, `fail_on_new` (false), `ignore: Vec<glob>`
  and `overrides: Vec<Override { glob, threshold: Option<f64>, metric: Option<Metric> }>`.
  The first matching override wins, field by field over the defaults.
  `RunConfig::from_toml_file(path)`.
- **Never panic in non-test code.** No `unwrap`, `expect` or indexing that
  can go out of bounds.

### `flipdiff.toml`
```toml
threshold = 0.01          # default threshold
metric = "mean"           # mean | p95 | max
fail_on_new = false
ignore = ["**/debug_*.png"]

[[override]]
glob = "terrain/**"
threshold = 0.03
metric = "p95"
```
Precedence: CLI flags override the toml top-level defaults. Overrides always
apply on top.

## 3. Report: `flipdiff-report.v1.json`

The model is `crates/flipdiff-core/src/report.rs`, and it is frozen. Its
semantics:
- **pass** means `value <= threshold`, and **fail** means it is over.
  `value` is the entry's `metric_used` drawn from `metrics`.
- **new**: a capture without a baseline.
- **missing**: a baseline without a capture.
- **error**: the pair could not be compared.
- `Report::is_regression()` decides the exit code.
- All `paths.*` are relative to the report directory and `/`-separated, so a
  report directory can be zipped and opened anywhere.

## 4. CLI: `flipdiff`

```
flipdiff compare <BASELINE_DIR> <CAPTURE_DIR> [--out report] [--threshold F] [--metric mean|p95|max]
                 [--config flipdiff.toml] [--fail-on-new] [--json] [--ppd F]
flipdiff approve <CAPTURE_DIR> <BASELINE_DIR> [NAMES...] [--all-failing <REPORT_JSON>]
flipdiff summary <REPORT_JSON> [--format markdown|text] [--artifact-url URL]
```
- `--config` defaults to `./flipdiff.toml` if that file exists.
- `compare` prints a short aligned text table to stdout: status, name, metric,
  value and threshold, with non-pass rows first. With `--json` it prints the
  report JSON instead. It always writes the report directory.
- `approve` copies captures over baselines, creating directories as needed. It
  takes the named images, or every fail and new entry in a report. It prints
  each file it copies.
- `summary` prints `render::render_markdown` output, or a plain-text form of
  it.
- **Exit codes:** `0` means no regression. `1` means a regression
  (`is_regression()`). `2` means a usage, config or IO error on the
  directories or the report.
- Use `clap` with derive. Its error output goes to stderr with exit 2.

## 5. HTML report (lane B)

`render::render_html(report, report_dir)` writes **one** `index.html`.
- All CSS and JS are inline. There are no external requests of any kind, no
  CDN and no web fonts, so it works when opened from an unzipped CI artifact
  via `file://`.
- The report JSON is embedded in a
  `<script type="application/json" id="flipdiff-data">`, escaping `</` as
  `<\/`. The images are referenced by their relative `paths.*`.
- **Header:** the run totals as status chips, the tool version, the generation
  time, and the default threshold and metric.
- **Table:** status, name, metric_used, value, threshold, mean, p95 and max.
  Columns sort on click. The default order puts non-pass first, then sorts by
  value descending. A filter toggles between "failures only" and "all".
- **Detail per entry**, which expands from its row: baseline, capture and
  heatmap side by side, wrapping on narrow screens. There is a **flicker**
  toggle (alternate baseline and capture every 500 ms, with a pause button)
  and a **swipe** slider (capture over baseline, clipped at the slider
  position). Missing images show a placeholder that names the status. The
  `properties` show as small badges, with all-black and all-white flagged.
- **Themes:** light and dark through `prefers-color-scheme`. Colours are CSS
  custom properties on `:root`. Status colours must not be the only signal, so
  each one carries a text label.
- **Responsive:** no horizontal page scroll at 390 px wide. The table can
  scroll inside its own container.
- Images render with `image-rendering: pixelated` once zoomed past 1×. Offer a
  1×/2×/fit zoom.

## 6. Markdown summary (lane B)

`render::render_markdown(report, opts)` produces:
- a heading line, `### flipdiff: ❌ 2 failed · 1 new · 14 passed`, or
  `### flipdiff: ✅ 17 passed`;
- a table of every non-pass entry (status, name, metric, value, threshold),
  with values to 4 significant digits;
- the passes inside `<details><summary>N passed</summary>…</details>`;
- a footer that links `opts.artifact_url` when it is set ("Full report with
  heatmaps"), followed by `flipdiff vX.Y.Z`.

The output must be at most `max_bytes` (default 60 000). If it is over, drop
pass rows first, then non-pass rows, and add a line saying "…and N more
(see full report)".

A hidden marker `<!-- flipdiff-summary -->` appears on the first line, so the
Action can find its sticky comment and update it.

## 7. GitHub Action (lane C)

`action.yml` is a composite action.

**Inputs:**
- `baseline-dir` and `capture-dir` (required);
- `threshold`, `metric`, `config` (optional);
- `report-dir` (default `flipdiff-report`);
- `fail-on-new` (default `false`);
- `comment` (default `true` on `pull_request` events);
- `github-token` (default `${{ github.token }}`);
- `version`, a git ref of this repo (default: the action's own ref).

**Steps:**
1. Install the CLI. Use a prebuilt release binary if one exists for the
   version; otherwise `cargo install --git <repo> --rev <ref> flipdiff --locked`.
   Rust is expected on the runner.
2. `flipdiff compare`. Keep its exit code and don't fail yet.
3. `actions/upload-artifact@v4` the report directory.
4. `flipdiff summary --format markdown --artifact-url <run url>` into
   `$GITHUB_STEP_SUMMARY`.
5. On a PR when `comment` is set, create or update the one comment that
   contains `<!-- flipdiff-summary -->`, using the `gh api` REST calls.
6. Exit with the saved code.

**Outputs:** `exit-code`, `failed`, `new`, `report-dir`.

Also ship:
- a CI workflow (`fmt --check`, `clippy -D warnings` and `test` on Linux,
  macOS and Windows);
- a release workflow that only triggers on `v*` tags and builds binaries for
  linux-x86_64, linux-aarch64 (if cross is easy, otherwise skip and note it),
  macos-arm64 and windows-x86_64;
- `examples/` holding a generated baseline/capture set with
  rendered-looking images, at least: one identical, one with a subtle
  sub-threshold change, one clear regression (for example a shifted shadow or
  a colour shift), one new and one missing;
- a generator script in `scripts/` that produces `examples/`
  deterministically.

## 8. Quality bar

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
  `cargo test` all pass.
- Rustdoc on every public item (`missing_docs` is on).
- **Tests:** about one focused test per stated behaviour. Generate fixtures in
  code; don't commit binary fixtures beyond `examples/`.

## 9. Non-goals (v0.1)

- HDR/EXR or float FLIP.
- GPU capture.
- Hosted storage, accounts, or image hosting for PR comments.
- Publishing to crates.io, the Marketplace or a public repo. That is a human
  decision.
- Masks and ignore-regions, beyond a single follow-up note in the README.
