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
  - It returns `Error::DimensionMismatch` if the dimensions differ,
    `Error::EmptyImage` for a 0×0 image and `Error::Config` unless
    `pixels_per_degree` is finite and > 0 (the CLI and `RunConfig::validate`
    reject it earlier, exit 2).
  - Percentiles use nearest-rank over the sorted error map. Map every NaN to
    +∞ first, then sort through `f32::total_cmp`.
  - **Alpha.** `compare` takes RGB. `compare::compare_rgba` is what `run` and
    `view` use: if both images are fully opaque it is `compare`; otherwise both
    are composited over black and over white, FLIP runs on each pair, and the
    error map is the per-pixel maximum. An alpha-only change is therefore
    visible, and RGB hidden under alpha 0 is not. Inputs are decoded to 8-bit
    RGBA, so 16-bit PNGs are accepted and down-converted.
  - Metric values that are not finite serialize as `null` in the report JSON
    and read back as NaN, so a report always round-trips.
- `properties::validate(img: &image::RgbImage) -> report::Properties` uses
  Rec. 709 luminance on sRGB-encoded bytes / 255. No linearisation; document
  this.
- `run::run(baseline_dir, capture_dir, report_dir, &RunConfig) -> Result<Report>`
  is the whole comparison.
  0. Remove `flipdiff-report.v1.json`, `index.html` and `images/` from the
     report directory, and nothing else, so a failed run cannot leave a stale
     report. A report directory that is the baseline or capture directory is
     refused (exit 2).
  1. Pair images by path relative to each root (`/`-separated). Recurse, and
     take extensions `png`, `jpg` and `jpeg`, case-insensitive. Apply `ignore`
     globs. Symbolic links are not followed: a link becomes an `error` entry
     ("symlinks are not followed"). A directory that cannot be read becomes an
     `error` entry named after it.
  2. For each pair, decode both. Copy them into
     `<report_dir>/images/<name>/{baseline,capture}.<ext>` and write
     `heatmap.png`. Every image gets its own directory, so nothing collides.
  3. Fill in an `Entry` (see `report.rs`). A decode failure or dimension
     mismatch gives `status = error`, with `error = Some(msg)`, and the copies
     that exist are still recorded.
  4. Write `<report_dir>/flipdiff-report.v1.json` (pretty, entries sorted by
     name), then call `render::render_html`.
  - Only directory, config and report-write failures return `Err`. Per-image
    problems (decode, copy, walk errors, symlinks) become `error` entries. A
    capture without a baseline that does not decode is `error`, not `new`.
- `config::RunConfig` holds `default_threshold` (0.01), `default_metric`
  (`mean`), `pixels_per_degree`, `fail_on_new` (false), `ignore: Vec<glob>`
  and `overrides: Vec<Override { glob, threshold: Option<f64>, metric: Option<Metric> }>`.
  The first matching override wins, field by field over the defaults.
  `RunConfig::from_toml_file(path)`. Globs are case-insensitive, like the
  extensions. `pixels_per_degree` must be finite and > 0.
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
flipdiff approve <CAPTURE_DIR> <BASELINE_DIR> [NAMES...] [--all-failing <REPORT_JSON> [--include-errors]]
flipdiff summary <REPORT_JSON> [--format markdown|text] [--artifact-url URL] [--comment-key KEY]
flipdiff unblind <DECISIONS_JSON> <BLIND_KEY_JSON> [--out FILE]
```
- `--config` defaults to `./flipdiff.toml` if that file exists.
- `compare` prints a short aligned text table to stdout: status, name, metric,
  value and threshold, with non-pass rows first. With `--json` it prints the
  report JSON instead. It always writes the report directory.
- `approve` copies captures over baselines, creating directories as needed. It
  takes the named images, or every fail and new entry in a report. It prints
  each file it copies. `--include-errors` (with `--all-failing`) also takes
  `error` entries whose capture exists and decodes, so a size change can be
  approved in bulk. It validates every name first and refuses, copying
  nothing, when an existing component of a destination below `BASELINE_DIR` is
  a symbolic link.
- Names and error text printed in the text table are escaped (`\n`, `\x1b`,
  ...) so they cannot inject workflow commands. A closed stdout pipe is not an
  error.
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
Action can find its sticky comment and update it. With `--comment-key KEY`
(`MarkdownOptions::comment_key`; ASCII letters, digits, `.`, `_`, `-`, at most
64) the marker is `<!-- flipdiff-summary:KEY -->`, so matrix jobs keep one
comment each.

An error entry's text is shown in a code span under its name, escaped like
names.

## 7. GitHub Action (lane C)

`action.yml` is a composite action.

**Inputs:**
- `baseline-dir` and `capture-dir` (required);
- `threshold`, `metric`, `config` (optional);
- `report-dir` (default `flipdiff-report`; relative to the workspace, because
  an absolute path is not uploaded as an artifact);
- `artifact-name` (default `flipdiff-report`; matrix jobs need distinct names);
- `comment-key` (default empty; the comment marker becomes
  `<!-- flipdiff-summary:<key> -->`);
- `fail-on-new` (default `false`);
- `comment` (default `true` on `pull_request` events);
- `github-token` (default `${{ github.token }}`);
- `version`, a git ref of this repo (default: the action's own ref).

**Steps:**
1. Install the CLI. Use a prebuilt release binary if one exists for the
   version and its `.sha256` file matches; the binary must also run
   `flipdiff --version`. Otherwise (no asset, missing or mismatched checksum, a
   binary that does not run) `cargo install --git <repo> --rev <ref> flipdiff --locked`.
   Rust is expected on the runner. The release workflow publishes a `.sha256`
   next to every asset.
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

## 10. Review viewer: `flipdiff view` (added 2026-10-01, lane E)

The viewer is for people who have to decide between images: a reviewer
accepting a re-baseline, a look-dev judge, a lead choosing between candidate
renders. It needs no thresholds and no CI.

```
flipdiff view <DIR_A> <DIR_B> [<DIR_C> ...] [--labels parent,candidate,...] [--reference <DIR>] [--blind] [--out view/]
```
- **Pairing.** Images are paired by relative path across 2 to 6 directories.
  - `--reference` marks one directory as the FLIP reference. Without it, the
    first directory is the reference.
  - Each non-reference image gets a heatmap against the reference, and its
    metrics are shown.
- **Output.** It writes a self-contained `view/index.html` with the same rules
  as §5: offline, no external requests, the data embedded as JSON, images
  copied under `view/images/`. It reuses the report's CSS tokens and
  components where it can.
- **Viewer features, per image set:**
  - **Layouts:** side-by-side grid (N-up), swipe (any 2 chosen), flicker (cycles
    through the chosen ones, adjustable rate, pause), and heatmap overlay with
    an opacity slider.
  - **Synchronised zoom and pan** across all panes: wheel or pinch zoom,
    drag to pan, and 1×/2×/4×/8×/fit buttons. Pixelated past 1×.
  - **Pixel inspector:** under the cursor, the RGB values of every pane at the
    same pixel, plus the FLIP value.
  - **Exposure/contrast stretch slider** (display only, with a reset), for
    judging dark frames. Channel isolation: R, G, B and luminance.
  - **ROI:** drag a rectangle to get mean FLIP and mean RGB per pane inside
    it; the ROI shows in every pane.
- **Blind mode (`--blind`), for pairwise A/B judging:**
  - Pane order is shuffled per image set with a seed recorded in the data, and
    panes are shown as "A", "B", ….
  - The page embeds only neutral directory labels (`P1`, `P2`, …) and the seed.
    The true labels are written to `<out>/blind-key.json`
    (`flipdiff-blind-key.v1`: `seed`, `labels`), which the page does not
    reference, so view-source reveals nothing. Pane errors do not name paths.
  - The judge picks a preferred pane, or "no visible difference", and may add
    a free-text note.
  - "Reveal" (enabled after all sets are decided) opens a file picker for
    `blind-key.json` and shows the true labels. The exported decisions carry
    the neutral labels; `flipdiff unblind <decisions> <blind-key.json>` writes
    the same file with the true labels (and fails on a seed or label mismatch).
- **Decisions in any mode:** per image set, accept / reject / needs-work and
  a note. Use "Export decisions" to download a `flipdiff-decisions.v1.json`
  file containing:
  - per set: name, the decision or preference, the true label of the chosen
    pane, the note, the ROI if one was drawn, and a timestamp;
  - for the whole file: the seed and the labels.
  The download goes through a Blob, so it works from `file://`. Decisions
  persist in `localStorage` (wrapped in try/catch) so a reload keeps them.
- **Approving from decisions.** `flipdiff approve --decisions <file>` takes
  the "accept" entries as names, so a human verdict feeds straight into
  baseline promotion.

## 9. Non-goals (v0.1)

- HDR/EXR or float FLIP.
- GPU capture.
- Hosted storage, accounts, or image hosting for PR comments.
- Publishing to crates.io, the Marketplace or a public repo. That is a human
  decision.
- Masks and ignore-regions, beyond a single follow-up note in the README.

## 11. Scene-capture features (added 2026-10-01, lanes F/G/H)

Target users: renderer teams comparing captures of heavy scenes (Sponza, Bistro, San Miguel, jungle ruins) —
identity proofs for optimizations, HDR/linear captures, and region-focused judgement. The report-model
additions are FROZEN in `report.rs` (`Mode`, `Labels`, `RegionResult`, `HdrInfo`, and `Entry.{regions,
masked_fraction, bit_identical, hdr}`, all `serde(default)` so v1 reports still parse; schema stays v1).

### 11.1 HDR / EXR (lane F)
- Decode `.exr` and `.hdr` (Radiance) via the `image` crate features `exr`/`hdr` (permissive licences; verify)
  into linear RGB f32. Pairing extensions extend to `exr`, `hdr`. Mixed LDR/HDR pair → `error` entry.
- **HDR-FLIP**: port the exposure procedure from NVIDIA FLIP's reference code
  (`~/.cargo/registry/src/*/nv-flip-sys-0.1.1/extern/cpp/CPP/image.h`, BSD-3 — keep attribution in
  THIRD_PARTY.md, port logic, don't copy text): compute start/stop exposure from the reference image
  luminance (auto) unless given; for each of N exposures tone-map both images (ACES default; Hable, Reinhard
  options), encode to sRGB 8-bit, run LDR-FLIP, take the **per-pixel max** over exposures. Document honestly
  that per-exposure images are quantised to 8 bits (reference HDR-FLIP stays in float) — an approximation.
- Config/CLI: `[hdr] tonemapper, start_exposure, stop_exposure, num_exposures` and
  `--hdr-tonemapper/--hdr-exposures START:STOP:N`. Fill `Entry.hdr`. Properties for HDR use linear luminance.
- Report/view images: copy the original EXR for download AND write a display PNG (tone-mapped at exposure 0
  with the chosen tonemapper) that `paths.baseline/capture` point to; add nothing to `EntryPaths` (the
  original is at `images/<name>/<side>.orig.<ext>` by convention; document it). `flipdiff view` handles EXR
  the same way (decode → display PNG; FLIP via HDR path).

### 11.2 Regions and masks (lane G)
```toml
[[region]]              # named ROI; fractions of the frame so resolution changes keep meaning
name = "atrium-floor"
glob = "sponza/**"      # optional; default all images
rect = [0.30, 0.55, 0.40, 0.35]   # x, y, w, h in [0,1]
threshold = 0.02        # optional; without it the region is informational
metric = "p95"          # optional; default = entry's metric

[[mask]]                # excluded from all statistics (whole-image and regions)
glob = "jungle/**"
rect = [0.0, 0.0, 1.0, 0.12]      # e.g. animated sky band
# or: image = "masks/jungle_foliage.png"  (white = EXCLUDE; resized nearest to frame; path relative to config)
```
- Region rect resolves to pixels by floor(x*W), floor(y*H), ceil to cover, clamped; empty → error entry.
- Masked pixels are excluded from every statistic; `masked_fraction` set when any mask applied. Heatmap shows
  masked pixels as a neutral hatch/grey. A region whose pixels are all masked → informational, value NaN→null.
- Entry `fail` if whole-image value > threshold OR any region with a threshold fails. Region results go in
  `Entry.regions` in config order.
- HTML report: region rows under the entry (name, value, threshold, status) and the region rectangles drawn
  over baseline/capture/heatmap (toggle). Markdown: a failing region adds `name › region` rows in the non-pass
  table. `flipdiff view`: config regions appear as preset ROIs (`--config`).

### 11.3 Identity mode (lane H)
`flipdiff identity <PARENT_DIR> <CANDIDATE_DIR> [--out] [--config] [--threshold F (default 0.0)] [--metric (default max)]`
- Same pipeline as `compare` with `Mode::Identity`, labels `parent`/`candidate` (overridable `--labels a,b`),
  defaults metric=max threshold=0.0, and `bit_identical` computed for every compared pair (decoded pixels equal;
  for HDR, f32 bitwise). Pass ⇔ bit-identical OR value ≤ threshold. Exit codes as compare.
- `compare` also gets `--labels` and always fills `bit_identical`.
- Text table and Markdown headline in identity mode: `identity: ✅ 12/12 bit-identical` / `❌ 2 differ (max
  FLIP 0.031 on bistro/cam3.png)`. HTML: labels from `config.labels` everywhere ("baseline"/"capture" strings
  must come from the report), a "bit-identical" badge per entry, mode shown in the header.

### 11.4 Shared rules
- Ownership: F → `hdr.rs` (new), decode helpers in `run.rs`/`view.rs` (decode functions only), `compare.rs`
  (add an HDR entry point; don't change the LDR one), `config.rs` `[hdr]` section, THIRD_PARTY.md.
  G → `regions.rs` (new), `config.rs` `[[region]]`/`[[mask]]`, stats plumbing in `compare.rs` (mask-aware
  statistics function), `run.rs` entry evaluation, `assets/report.*` + `render/html.rs` + `render/markdown.rs`
  region display, `view.rs`/`view.js` preset ROIs. H → `main.rs` (`identity`, `--labels`), `run.rs`
  `RunConfig` mode/labels plumbing + bit_identical, label/mode/badge display in report and view assets.
  Overlaps (run.rs, config.rs, compare.rs, report assets) — keep edits local, small, and re-read the file
  right before each edit; never reformat or reorder others' code.
- README gains a "Scene captures" section per feature (each lane writes its own subsection under a heading
  the first lane creates; append-only).
