# saccade design and format reference

This document describes how saccade compares images and the formats it reads
and writes. Usage is in the [README](../README.md). The report schema is
`saccade-report.v1`; the decisions and blind-key files are
`saccade-decisions.v1` and `saccade-blind-key.v1`.

## 1. Pipeline

`saccade compare BASELINE_DIR CAPTURE_DIR` does the following.

1. **Check, then clean the report directory.** The output directory is refused
   (exit 2) when it is, or is inside, the baseline or the capture directory, and
   when it exists, is not empty and holds no `saccade-report.v1.json` (it is
   not a previous report). `view --out` applies the same rule with the marker
   `saccade-view.v1.json`, and `explain --out` with `explain.json`. A run then
   removes `saccade-report.v1.json`, `index.html` and `images/` from the
   report directory, and nothing else, so a failed run cannot leave a stale
   report. While a run is incomplete it keeps a `.saccade-run` file there (the
   ownership marker of a failed run); the file is removed when the report is
   complete.
2. **Pair images** by path relative to each directory (section 2).
3. **Compare each pair** with FLIP (section 3) and evaluate it against its
   threshold, regions and masks. Pairs are compared in parallel (`rayon`, one
   thread per core, `RAYON_NUM_THREADS` overrides); entries are collected in
   name order, so the output does not depend on scheduling. `view` builds its
   image sets the same way.
4. **Write** the images, `saccade-report.v1.json` and `index.html`, then print
   the summary table.

Only directory, config and report-write failures abort a run. A problem with
one image (it does not decode, the sizes differ, it is a symbolic link, a
directory cannot be read) becomes an `error` entry and the run continues.

## 2. Pairing rules

- Images are found recursively. The extensions are `png`, `jpg`, `jpeg`,
  `exr` and `hdr`, in any case.
- The key is the path relative to its directory, with `/` as the separator.
  `a/b.png` in the baseline pairs with `a/b.png` in the capture.
- A baseline image with no capture is `missing`. A capture with no baseline is
  `new`. A capture without a baseline that does not decode is `error`.
- Symbolic links are not followed; each becomes an `error` entry ("symlinks are
  not followed").
- `ignore` globs (config) remove names from both sides before pairing.
- Globs in the config match the whole `/`-separated name, case-insensitively.
  `*` does not cross `/`; `**` does.
- An HDR image (`exr`, `hdr`) and an LDR image cannot be compared: `error`.
- Two images with different dimensions cannot be compared: `error`.

`saccade view` pairs 2 to 6 directories the same way.

## 3. Comparison

### 3.1 FLIP

FLIP (Andersson et al., "FLIP: A Difference Evaluator for Alternating Images",
HPG 2020) models how a human observer sees the difference between two images
shown alternately: colour, contrast sensitivity and spatial frequency, at a
given viewing distance. It produces an error map with one value per pixel in
`[0, 1]`: 0 is no visible difference, larger values are more visible.

saccade calls the pure-Rust `flip-rs` port of NVIDIA FLIP v1.7. LDR inputs
are decoded to 8-bit sRGB and divided by 255 to supply sRGB `f32` values;
`ldr_flip` performs the sRGB-to-linear conversion. A 16-bit PNG is accepted
and down-converted to 8 bits. Statistics remain saccade's nearest-rank
percentiles, rather than flip-rs's error-weighted pooling.

`pixels_per_degree` (`--ppd`, `ppd` in the config) is the viewing condition.
The default is 67, FLIP's default. It must be finite and greater than 0. A
higher value models a farther viewer or a denser display, so fine detail
counts for less.

### 3.2 Statistics

The statistics are taken over the error map:

| Field | Definition |
|---|---|
| `mean` | Arithmetic mean |
| `max` | Largest value |
| `p50`, `p95`, `p99` | Nearest-rank percentiles of the sorted map |
| `frac_above_0_1`, `frac_above_0_5` | Fraction of pixels with an error above 0.1 and above 0.5 |
| `width`, `height` | Image size in pixels |

NaN values sort as +infinity. A metric that is not finite is written as `null`
in the JSON and read back as NaN, so every report round-trips.

The deciding statistic for an entry (`metric_used`) is `mean`, `p95`, `p99` or `max`.
The entry passes when `value <= threshold`, where `value` is that statistic.
The defaults are metric `mean` and threshold 0.01. Overrides in the config
change them per image.

### 3.3 Alpha

If both images are fully opaque, FLIP runs on the RGB values. Otherwise both
images are composited over black and over white, FLIP runs on each pair, and
the error map is the per-pixel maximum of the two. An alpha-only change is
therefore visible, and RGB values hidden under fully transparent pixels are
ignored.

### 3.4 Masks and regions

- A `[[mask]]` removes pixels from every statistic: the whole-image values and
  every region. `masked_fraction` records how much of the frame was removed.
  The heatmap shows masked pixels as a grey hatch.
- FLIP's spatial filters run on the whole image before masking. A change just
  inside a mask therefore raises error values in neighbouring unmasked pixels.
  Leave a margin around the area you want to ignore.
- A `[[region]]` is a named rectangle, given as fractions of the frame so that
  a resolution change keeps its meaning. It gets its own statistics and, if it
  has a `threshold`, its own pass or fail status. A region without a threshold
  is informational.
- A rectangle becomes pixels by flooring the origin and taking the ceiling of
  the far edge, clamped to the frame. A rectangle that resolves to no pixel
  makes the entry an `error`.
- A region whose pixels are all masked is informational and its value is
  `null`. A mask that covers the whole image makes the entry an `error`.
- An image mask is white = exclude (luminance of 128 or more) and is resized
  to the frame with nearest-neighbour sampling. Its path is relative to the
  config file and must not be absolute or contain `..`.
- A region or mask `glob` that matches no image in the run prints a warning.
- An entry fails if its whole-image value exceeds its threshold or any region
  with a threshold fails.

### 3.5 HDR-FLIP

`.exr` and `.hdr` (Radiance) files are decoded to linear `f32` RGB. Alpha is
dropped, and NaN and negative values become 0. A pair is compared with
HDR-FLIP, which follows NVIDIA's reference procedure:

1. Choose an exposure range. By default it is computed from the baseline
   image: the brightest pixel reaches 0.85 after tone mapping at the first
   exposure, and the median luminance does at the last. `start_exposure` and
   `stop_exposure` (in stops) override it.
2. Choose the number of exposures N: `num_exposures`, or
   `max(2, ceil(stop - start))`. The count must be in `2..=i32::MAX`, even
   when the endpoints are equal.
3. For N evenly spaced exposures, tone-map both images (`aces`, `hable` or
   `reinhard`) in linear `f32` and run FLIP without 8-bit quantisation.
4. The error map is the per-pixel maximum over the exposures.

`flip-rs::hdr_flip` implements the reference float HDR-FLIP algorithm.
Its resolved `HdrParameters` populate the entry's `hdr` record; each omitted
endpoint and count is resolved from the baseline. A zero median luminance is
floored to `f32::EPSILON`, as in the reference. An all-black baseline has no
automatic start exposure and returns an error; give explicit endpoints to
compare it. Invalid, reversed or nonfinite exposure ranges return errors.

**Credit.** flip-rs is a BSD-3-Clause port of NVIDIA FLIP v1.7 and retains
NVIDIA's notice. The display-only tone-mapping coefficients are also derived
from NVIDIA's reference. See [THIRD_PARTY.md](../THIRD_PARTY.md).

For HDR pairs, the report and the viewer show a display PNG tone-mapped at
exposure 0 (`images/<name>.d/baseline.png`, `capture.png`). The original file is
copied next to it as `baseline.orig.<ext>` and `capture.orig.<ext>`. Image
properties use linear Rec. 709 luminance. The chosen settings are recorded in
the entry's `hdr` field.

### 3.6 Identity mode

`saccade identity PARENT CANDIDATE` runs the same pipeline with
`mode = "identity"`, labels `parent` and `candidate`, metric `max` and
threshold 0. For every compared pair it sets `bit_identical`: whether the raw
decoded samples are equal at their native depth, including alpha, 16-bit
values and HDR NaN and negative values. A pair passes if it is bit-identical
or its value is within the threshold.

`compare` also fills `bit_identical` on every compared entry.

### 3.7 Image properties

Each compared image carries `properties`: `is_all_black`, `is_all_white`,
`mean_luminance`, `min_luminance`, `max_luminance`, `nan_count`, `inf_count`
and `negative_count`. Luminance is Rec. 709
weights applied to the sRGB-encoded values divided by 255, with no
linearisation. For HDR images it is linear Rec. 709 luminance over the finite
pixels, and "all white" means every channel is at least 1.0. The three counts
are the NaN, infinite and negative samples found in an HDR file before decoding
replaced them (always 0 for 8-bit images). `baseline_properties` holds the same
checks for the baseline.

Entries carry `warnings`, plain text lines: a baseline or capture that is
all black or all white, and non-finite samples. A warning never changes a
status. The exception is `fail_on_nonfinite` (config, default `true`): a
capture with NaN or infinite samples makes the entry an `error` (its metrics are
kept), because FLIP over replaced samples would compare something other than the
capture. Set it to `false` to keep only the warning. The text table, the
Markdown summary and the report show the warnings.

### 3.8 Hotspots

Hotspots say where on the frame the FLIP error is concentrated. For every
compared pair, the pixels whose error is strictly above `hotspot_threshold`
(default 0.1) are grouped into 8-connected components; components whose
bounding boxes lie within about 1% of the long side of each other are merged;
the groups are ranked by summed error and the first `hotspots` (default 5, 0
disables the search) are kept. Masked pixels never count. The component search
is a single raster pass with union-find, so memory does not grow with the frame.

| Field | Type | Notes |
|---|---|---|
| `rect_px` | `[x, y, w, h]` | Bounding box in pixels |
| `rect_frac` | `[x, y, w, h]` | The same box as fractions of the frame |
| `area_px`, `area_frac` | number | Pixels above the threshold, absolute and as a fraction of the frame |
| `mean_flip`, `max_flip` | number | Over the unmasked pixels inside the box |
| `share_of_total_error` | number | Summed error of the hotspot over the summed error of every unmasked pixel, in 0 to 1 |
| `position` | string | 3x3 cell of the box centre: `top-left`, `top-center`, `top-right`, `middle-left`, `center`, `middle-right`, `bottom-left`, `bottom-center`, `bottom-right` |

A box whose area (`rect_frac[2] * rect_frac[3]`) is at least 0.5 is shown as a
"frame-wide change" in the HTML report and in `view`.

**Local defects behind a passing mean.** A small, severe defect barely moves
the `mean`. Two things keep it from passing silently:

- A passing entry whose worst hotspot has `max_flip >= 0.5` gets a line
  `↳ pass, but local hotspot: 40x40 @ top-left (max 0.75)` in the text table and
  in the Markdown summary. It does not change the status.
- `hotspot_fail` (config, off by default; a value in `(0, 1]` that is not below
  `hotspot_threshold`) fails an entry when the largest unmasked FLIP value of
  the frame reaches it. Because that pixel is above `hotspot_threshold`, it
  belongs to a hotspot. The reason is added to the entry's `warnings`.
  Alternatively choose a metric that sees tails: `--metric p99`.

### 3.9 Diagnostics

Every compared pair gets `diagnostics` (module `saccade_core::diagnostics`):
what kind of change it is, in numbers. It never changes a status.

- **Tone fit.** Per channel, in linear light, `capture = gain * baseline + bias`
  by four rounds of trimmed least squares (20% of the largest residuals dropped
  each round) over the lower-gradient half of a grid of at most about 1M
  pixels, skipping clipped samples. The bias is fitted only when the baseline's
  standard deviation is at least 0.02; otherwise gain only. Reported:
  `exposure_stops` (log2 of the luminance-weighted gain, negative is darker),
  `gain`, `bias`, `white_balance` (red and blue gain relative to green) and a
  `summary` line. The inverse fit is applied to the capture and FLIP re-run;
  `tone_explained_fraction` is the share of the mean error removed.
  `residual` holds the mean, max and top-3 hotspots of that re-run. A fit with
  every gain within 0.4% of 1 and every bias below 0.002 is negligible: no
  re-run, fraction 0.
- **Sub-pixel shift.** Phase correlation of a compressed luminance
  (`sqrt(y / (1 + y))`), mean-removed, Hann-windowed, zero-padded to a power of
  two. A first estimate runs on a plane of at most 1024 px per side (box
  downsample); the offset is rounded to whole pixels and a second pass on a
  full-resolution window of at most 1024 x 1024 refines it. The sub-pixel part
  is a weighted least-squares fit of the residual cross-power phase slope over
  frequencies up to a quarter of the sampling rate. `confidence` is the
  weighted phase coherence of the fit residuals (1 for a pure translation,
  near 0 for unrelated content). A detected shift is only named in the
  description when the re-run confirms it (`shift_explained_fraction`).
  `dx`, `dy`: positive is right and down, content of the capture against the
  baseline. When the offset reaches `shift_min_px` (0.25) with
  `shift_min_confidence` (0.5), the capture is resampled back (bicubic) and
  FLIP re-run: `shift_explained_fraction`.
- **Signed difference.** `signed`: mean of capture minus baseline display
  luminance, fractions brighter and darker than half a code value, and the
  colour `scale`. `paths.signed_diff` is `images/<name>.d/signed_diff.png`: dark
  grey is no change, orange brighter, blue darker, full colour at `scale` (the
  99.5th percentile of the absolute difference, at least 0.004).
- **Non-finite map.** For an HDR capture with NaN, infinite or negative
  samples: `nonfinite` (counts, cluster count, boxes of the first 16
  8-connected clusters in raster order) and `paths.nonfinite_mask` (magenta
  NaN, yellow infinite, cyan negative).
- **Perf pairing.** `perf`: every sidecar key matching `perf_keys` (default
  `*_ms`, `gpu_ms`, `frame_ms`, `*.ms`, `timing.*`) with a numeric value on
  both sides: `{key, baseline, capture, delta, delta_pct}`, largest relative
  change first. These keys are also ignored by `--require-matching-meta`.
- **Class**, first match wins: `identical` (bit-identical, or maximum FLIP 0);
  `broken_frame` (capture entirely black or white while the baseline is not,
  NaN or infinite samples, or a flat colour while the baseline has contrast);
  `misaligned` (shift fraction at least `explained_min` 0.8 and not below the
  tone fraction); `global_tone` (tone fraction at least 0.8); `noise` (both
  fractions below `partial_min` 0.2 and maximum FLIP at most `noise_max_flip`
  0.05); `local_structure` (both fractions below 0.2, above noise); otherwise
  `mixed`. Below mean FLIP 0.0001 no cause search runs.
- **Description.** A fixed template per class. A tone or shift cause is named
  only when its fraction reaches `partial_min`; "rest of frame unchanged" only
  when the mean FLIP outside the named regions is at most 0.001 (within noise
  up to 0.01). Examples: `Local structural change at bottom-center (149×39,
  20% of error); rest of frame unchanged.`, `Whole frame 0.4 stops darker (tone
  shift explains 91% of the difference); no structural change.`, `Capture
  shifted 0.5 px right (explains 87%).` A blind explain pack omits class and
  description because they name the sides.

Config `[diagnostics]`: `enabled` (true), `shift_detection` (true),
`shift_min_px`, `shift_min_confidence`, `noise_max_flip`, `explained_min`,
`partial_min`, `perf_keys`. Masks and regions do not enter the analysis; the
FLIP means it compares are whole-frame.

## 4. Statuses and exit codes

| Status | Meaning |
|---|---|
| `pass` | `value <= threshold` (identity mode: or bit-identical) |
| `fail` | `value > threshold`, or a region with a threshold failed |
| `new` | Capture without a baseline |
| `missing` | Baseline without a capture |
| `error` | The pair could not be compared |

| Exit code | Meaning |
|---|---|
| 0 | No regression |
| 1 | Regression: any `fail`, `error` or `missing` entry, a `new` entry with `fail_on_new`, or no pair compared at all (`nothing compared`, for example an empty baseline directory) unless `--allow-empty` or `allow_empty = true` |
| 2 | Usage, config or IO error, including a refused output directory (section 1) |

A pair counts as compared when it has FLIP metrics. A run where every entry is
`new` (empty or missing baseline directory) therefore exits 1 with a warning on
stderr; `--allow-empty` accepts it, for the first run that creates the
baselines.

Command-line parse errors are printed to stderr with exit code 2. Names and
error text printed in the table are escaped (`\n`, `\x1b`, ...) so a file name
cannot inject workflow commands. A closed stdout pipe is not an error.

## 5. Report directory

```
report/
  index.html                     self-contained HTML report
  saccade-report.v1.json         machine-readable report
  images/<name>.d/baseline.<ext>   copies of both inputs
  images/<name>.d/capture.<ext>
  images/<name>.d/heatmap.png      FLIP error map (magma colour map)
```

Every path in the JSON is relative to the report directory and `/`-separated,
so the directory can be zipped, moved and opened anywhere.

`<name>` is the relative input image name, including its extension. The `.d`
suffix marks a directory: `terrain/lit.png` goes under
`images/terrain/lit.png.d/`. Viewer and serve-session images follow this
convention too, including per-pane diagnostics. Explain hotspot strips use
`hotspots/<name>.d/`; run overviews store flat thumbnails and heatmaps.

At the CLI boundary, every command with `--out` checks its parent for the
configured metadata sidecar (default `saccade-meta.json`) or `capture.json`.
Finding either prints a stderr warning because generated images may be
indexed as captures. `--allow-out-near-captures` suppresses it. MCP skips this
warning and continues to confine paths to its root.

`index.html` is one file. CSS and JavaScript are inline, the report JSON is
embedded in a `<script type="application/json" id="saccade-data">` element,
and images are referenced by relative path. It makes no external request, so it
works from a `file://` URL and from an unzipped CI artifact. It follows
`prefers-color-scheme`, and each status carries a text label as well as a
colour.

## 6. Report JSON: `saccade-report.v1`

The Rust model is `crates/saccade-core/src/report.rs`. Fields added after the
first release have `serde` defaults, so older v1 reports still parse.

### 6.1 Top level

| Field | Type | Notes |
|---|---|---|
| `schema` | string | `"saccade-report.v1"` |
| `tool_version` | string | saccade version that wrote it |
| `generated_at_unix` | integer | Seconds since the Unix epoch |
| `baseline_dir`, `capture_dir` | string or null | Input paths relative to the report directory by default; `--record-absolute-paths` opts in to absolute paths. `approve` resolves them from the report location |
| `config` | object | Effective settings, see below |
| `totals` | object | `total`, `pass`, `fail`, `new`, `missing`, `error` |
| `entries` | array | One entry per image name, sorted by name |

`config`:

| Field | Type | Notes |
|---|---|---|
| `default_threshold` | number | |
| `default_metric` | `"mean"`, `"p95"`, `"p99"`, `"max"` | |
| `allow_empty` | boolean | A run that compared no pair is accepted |
| `fail_on_nonfinite` | boolean | Default true, see section 3.7 |
| `hotspot_fail` | number or null | See section 3.8 |
| `pixels_per_degree` | number | |
| `fail_on_new` | boolean | |
| `mode` | `"regression"`, `"identity"` | |
| `labels` | `{baseline, capture}` | Display names of the two sides |
| `meta` | object | Sidecar settings: `name` (file name), `required` (`--require-matching-meta`), `declared` (keys allowed to differ), `ignored` (effective ignore globs) |

### 6.2 Entry

| Field | Type | Notes |
|---|---|---|
| `name` | string | Path relative to the input directories |
| `status` | string | See section 4 |
| `metric_used` | string | The deciding statistic |
| `threshold` | number | Effective threshold after overrides |
| `value` | number or null | The deciding statistic's value |
| `metrics` | object or null | `mean`, `max`, `p50`, `p95`, `p99`, `frac_above_0_1`, `frac_above_0_5`, `width`, `height` |
| `properties` | object or null | See section 3.7 |
| `paths` | object | `baseline`, `capture`, `heatmap`: relative paths or null |
| `error` | string or null | Message for `error` entries |
| `regions` | array | One result per matching `[[region]]`, in config order |
| `masked_fraction` | number or null | Fraction of pixels masked, when a mask applied |
| `bit_identical` | boolean or null | Set for compared pairs |
| `hdr` | object or null | `tonemapper`, `start_exposure`, `stop_exposure`, `num_exposures`, `auto_range` |
| `hotspots` | array | Ranked hotspots (section 3.8); empty when none exceed the threshold, when the pair was not compared or when they are disabled |
| `diagnostics` | object or null | Section 3.9: `class`, `description`, `tone`, `residual`, `shift`, `signed`, `nonfinite`, `perf`, `elapsed_ms`; paths in `paths.signed_diff` and `paths.nonfinite_mask` |
| `meta_diff` | array | Sidecar keys that differ: `{key, baseline, capture}`; a missing value is `"<absent>"` |
| `meta_ignored_diff` | array | Keys that differ but matched an ignore glob, same shape |
| `baseline_properties` | object or null | Section 3.7, for the baseline |
| `warnings` | array of strings | Section 3.7 and 3.8; never change a status on their own |
| `baseline_sha256`, `capture_sha256` | string or null | SHA-256 of the files as compared; `approve` verifies them |

A region result has `name`, `rect_px` (`[x, y, w, h]` in pixels), `status`
(null when informational), `metric_used`, `threshold`, `value` (null when fully
masked) and `metrics`.

## 7. Markdown summary

`saccade summary REPORT --format markdown` prints:

- a first line `<!-- saccade-summary -->`, the marker the GitHub Action uses to
  find its pull-request comment. With `--comment-key KEY` the marker is
  `<!-- saccade-summary:KEY -->`. A key is ASCII letters, digits, `.`, `_` and
  `-`, at most 64 characters;
- a heading such as `### saccade: ❌ 2 failed · 1 new · 14 passed`, or
  `### saccade: ✅ 17 passed`. In identity mode the heading reads
  `identity: ✅ 12/12 bit-identical` or
  `identity: ❌ 2 differ (max FLIP 0.031 on a/b.png)`;
- a table of every non-pass entry (status, name, metric, value, threshold),
  with a row `name › region` for each failing region. Numbers use 4
  significant digits with no trimming (`0.01000`, `0.5000`, `123.5`), and zero
  is `0.0000`;
- bullet lines for the entries with notes, over every entry (at most 20):
  `↳ pass, but local hotspot: ...` for a passing entry with a severe local
  hotspot (section 3.8) and `⚠ <warning>` for each warning (section 3.7). A
  run that compared nothing says `nothing compared` in the heading;
- the passing entries inside `<details>`;
- a footer that links the report artifact when `--artifact-url` is given,
  followed by `saccade vX.Y.Z`.

The output is capped at 60 000 bytes. If it is longer, pass rows are dropped
first, then non-pass rows, and a line says how many were left out. An error
entry's text is shown in a code span under its name.

## 8. Viewer: `saccade view`

`saccade view DIR_A DIR_B [DIR_C ...] --out view/` writes a self-contained
`view/index.html` with the same rules as the report (offline, data embedded,
images copied to `view/images/`).

- **Reference.** `--reference` names the FLIP reference directory. Without it
  the first directory is the reference. Every other image gets a heatmap and
  metrics against the reference.
- **Layouts.** Side by side, swipe (any two chosen images), flicker (cycles
  through the chosen images at an adjustable rate) and heatmap overlay with an
  opacity slider.
- **Navigation.** Synchronised zoom and pan across panes (wheel or pinch to
  zoom, drag to pan, 1x, 2x, 4x, 8x and fit buttons; pixelated past 1x).
- **Pixel inspector.** The RGB value of every pane at the cursor, and the FLIP
  value.
- **Display controls.** Exposure and contrast sliders (display only), and
  channel isolation (R, G, B, luminance).
- **Hotspots.** Each non-reference pane carries its own `hotspots` against the
  reference (same fields as section 3.8, default settings) in the embedded
  model. The viewer draws them as numbered boxes, lists them in a card, and
  zooms to a box on click. Blind mode hides them with the other measurements.
- **Region of interest.** Drag a rectangle to get the mean FLIP value and mean
  RGB per pane. Regions from `--config` appear as preset rectangles.

### 8.1 Decisions

For each image set the reviewer picks `accept`, `reject` or `needs-work`, and
may add a note. "Export decisions" downloads `saccade-decisions.v1.json`. The
download goes through a Blob, so it works from `file://`. Decisions are kept in
`localStorage` so a reload keeps them.

```json
{
  "schema": "saccade-decisions.v1",
  "seed": 7,
  "labels": ["baseline", "capture"],
  "blind": false,
  "dirs": ["../baseline", "../capture"],
  "sets": [
    {
      "name": "sphere_shadow.png",
      "decision": "accept",
      "chosen_label": null,
      "no_difference": false,
      "note": "shadow reads better",
      "roi": { "x": 60, "y": 140, "w": 120, "h": 60 },
      "timestamp_ms": 1790000000000,
      "chosen_dir": null,
      "sha256": ["3b1f...", "9c07..."]
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `seed`, `labels` | The view's shuffle seed and directory labels |
| `blind` | Whether the view was blind |
| `sets[].name` | Image path of the set |
| `sets[].decision` | `"accept"`, `"reject"`, `"needs-work"` or null |
| `sets[].chosen_label` | Blind pairwise judging: the preferred pane's label, or null |
| `sets[].no_difference` | Blind pairwise judging: "no visible difference" |
| `sets[].note` | Free text |
| `sets[].roi` | The rectangle that was drawn, in pixels, or null |
| `sets[].timestamp_ms` | Last edit, Unix milliseconds |
| `dirs` | Path of each directory relative to the decisions document, in `labels` order. Absolute only by opt-in. Empty in a blind view until `unblind` fills it from the key |
| `sets[].chosen_dir` | Pairwise judging: the directory of the preferred pane (set by the viewer, or by `unblind`) |
| `sets[].sha256` | SHA-256 of the set's image in each directory when it was judged, in directory order; null where the directory had none |

`saccade approve --decisions view/decisions.json` (two recorded directories), or `saccade approve CAPTURE BASELINE --decisions decisions.json` copies the
capture of every set whose `decision` is `accept`, after checking that the file
is what was judged. It refuses (exit 2, error code `approve_mismatch`) when:

- `blind` is true: the file is still blind; run `saccade unblind` first;
- `CAPTURE` or `BASELINE` is not one of `dirs`;
- an accepted set's `chosen_dir` is not `CAPTURE`;
- a capture file's SHA-256 differs from the recorded one.

`saccade approve --report report.json --all-failing` makes the same checks against the
report's `baseline_dir`, `capture_dir` and per-entry `capture_sha256` (and
`baseline_sha256`, which also guards `--prune-missing`). `--force` overrides
the directory and hash checks and prints each overridden problem as a warning.
A file that records no directories or hashes (written by an older version)
cannot be checked: `approve` warns and proceeds.

### 8.2 Blind mode

With `--blind` every image set lays out its panes in its own random directory
order (seeded by `--seed`, or by the operating system's randomness), and the
panes are labelled "A", "B", and so on. What the page embeds is chosen so that
view-source reveals nothing:

- neutral labels `P1`, `P2`, ... by position within the set, and image files
  named `images/<name>.d/p_<random hex>.<ext>` (never `pane<i>`);
- `order` is always `0..n` (the panes themselves are shuffled) and `reference`
  is `n`, no pane;
- no FLIP data at all: no heatmaps, metrics, hotspots, error maps, triage status
  or meta diffs, because any of them singles out the reference directory (the
  alternative, comparable data on every pane, is not possible with a single FLIP
  reference);
- `seed` is a random token that only pairs the page with its key and decisions,
  not the shuffle seed, so the shuffle cannot be undone from the page;
- pane errors do not name paths.

The true labels are written to the key file: `--key-out PATH`, by default
`<out>/blind-key.json` (then keep `<out>` away from the judge). The page does
not reference it.

```json
{ "schema": "saccade-blind-key.v1", "seed": 2555678565829461, "shuffle_seed": 7,
  "labels": ["baseline", "capture"], "dirs": ["/abs/baseline", "/abs/capture"],
  "sets": { "scene.png": ["capture", "baseline"] } }
```

`sets[name][n - 1]` is the true label behind `P<n>` in that set. A test greps
the blind page and the blind explain pack for the directory names and labels.

The judge picks a preferred pane or "no visible difference", and may decide
accept, reject or needs-work. "Reveal labels" is enabled once every set is
decided; it opens a file picker for `blind-key.json` and shows the true
labels. The exported decisions carry the neutral labels (`P1`, `P2`).
`saccade unblind decisions.json blind-key.json [--out FILE]` writes the same
file with the true labels, and fails if the seeds or label counts differ.

## 9. Metadata sidecars

Two captures can differ because the configuration differed, not because the
code did: for example, a capture preset silently injects a renderer setting the
caller never chose. A comparator that ignores configuration then reports a
"regression" or an "identity" verdict about the wrong question. Sidecars record
how each directory of captures was made, so saccade can show the difference
and, on request, refuse to give a verdict.

- **File.** `saccade-meta.json` by default (`--meta-name`, or `meta_name` in the
  config). A per-image sidecar is `<image_stem>.<meta-name>`.
- **Lookup.** For an image, the sidecars from the root of the directory down to
  the image's folder are merged, the nearer one winning on a conflicting key,
  and the per-image sidecar overrides the result. This is done for each side.
- **Format.** A flat JSON object. Values are strings, numbers, booleans or null.
  A nested value makes the entry an `error` that names the key. A sidecar that
  is a symbolic link, or larger than 1 MiB, is refused. Dot-namespaced keys are
  a convention (`renderer.mode`, `resolution.internal`, `binary.sha`).
- **Comparison.** The two merged key sets are compared. A key present on one
  side only differs, with the value `<absent>` on the other. Keys matching an
  ignore glob are skipped. The built-in ignore globs, matched
  case-insensitively, are `*timestamp*`, `*_ms`, `*duration*`, `*elapsed*`,
  `run.id`, `*.started_at`, `*.finished_at` and `generated_at*`. A bare
  `*time*` is deliberately absent: it also matches `timezone`, `timeout` or
  `lifetime`. `--meta-ignore` adds more. The differences are stored in the
  entry's `meta_diff`; the ones that were skipped are stored in
  `meta_ignored_diff`, so what an ignore glob hides stays visible.
- **Enforcement.** With `--require-matching-meta`, an entry with a differing key
  that is not declared (`--declare KEY|GLOB,...`) becomes an `error` (its
  metrics are kept), so the run exits 1. Declared differences pass and are
  still shown. Without the flag, differences are shown and never change a
  status.
- **Display.** The HTML report shows a "config differs" badge and a key table
  per entry. The Markdown summary adds a line
  `⚠ config differs on N images: ...`. The text table puts a
  `↳ config differs: ...` line under the entry (omitted when the entry's error
  already names the keys). `view` shows a card per image set, and hides it in
  `--blind` mode.
- **Scope.** `compare` and `identity` accept all four flags. `view` accepts
  `--meta-name` and `--meta-ignore`.

## 10. Judge packs: `saccade explain`

`saccade explain REPORT_JSON [--out DIR]` reads a report and writes, for each
failing entry, crops that show what changed at each hotspot. Only the pack's own
files in the output directory are replaced.

```
explain.json                 schema saccade-explain.v1 (schemas/saccade-explain.v1.schema.json)
explain.md                   the same, as text
thumbs/<name>.png            whole-frame strip with the hotspot boxes drawn
hotspots/<name>.d/hN.png       [baseline | capture | heatmap] crop strip of hotspot N
```

The thumbnail of `a.png` is `thumbs/a.png` (a name that does not end in `.png`
gets `.png` appended). A strip is at most 1536 px wide: panels of a wider strip
are scaled down together, and `scale` in `explain.json` includes that factor.
Hotspots carrying less than `--hotspot-min-share` (default 0.01) of the total
error are left out; `hotspot_min_share` in `saccade.toml` applies the same
filter when the report is made.

`explain.json` has `schema`, `report` (relative to the pack; absent when blind), `dir`
(pack path relative to the working directory; absolute paths require opt-in), `blind`, `labels` (absent when blind), `settings` (`top`,
`pad`, `stretch`) and `entries`. An entry has `name`, `status`, `metric_used`,
`threshold`, `value`, `thumbnail`, an optional `note` (for example "no hotspot
above the threshold: the difference is diffuse or below it") and `hotspots`.
Each hotspot has `index` (1-based), the report's `hotspot` object, `strip`,
`crop_rect_px` (the box plus `--pad`, clamped to the frame), `scale`, `gain` and
`panels`. With `--stretch`, dark crops get one gain applied to both images.

With `--blind` each strip is `[A | B]`, A and B assigned at random per hotspot
(`--seed`, recorded in the key) and the heatmap omitted. The pack names neither
the report nor the sides. `--key-out PATH` is required and must be outside
`--out`; the key (`saccade-explain-blind-key.v1`) lists, for each strip (the
whole-frame strip has `hotspot: null`), which side was A and which was B. It is
never written into the pack. Per-hotspot hot-pixel and box areas are printed
separately in `explain.md` (`hot px N (x% of frame), box y% of frame`).

## 11. MCP server: `saccade mcp`

JSON-RPC 2.0 over stdio, one JSON message per line, protocol version
`2025-06-18`. It implements `initialize`, `notifications/initialized`, `ping`,
`tools/list` and `tools/call`. Tools: `saccade_compare`, `saccade_identity`
(both write a report and a judge pack into `out_dir`), `saccade_explain` and
`saccade_summary`. `tools/list` gives each tool an `inputSchema`, an
`outputSchema` (a compact form of the shipped schema) and annotations
(`destructiveHint: false`, `idempotentHint: true`, `readOnlyHint` only for the
summary). The run tools accept `threshold`, `metric`, `ppd`, `labels`,
`meta_name`, `fail_on_new`, `allow_empty`, `require_matching_meta`, `declare`
and `include_images` (compare also `config`).

**Path policy.** `saccade mcp [--root DIR]` (default: the working directory).
CLI next-step commands use the working directory; MCP result paths are relative
to the server root so they can be reused directly as tool inputs. Entry image
paths remain relative to the report directory.

Every path argument (inputs, `out_dir`, `report_json`, `config`, `key_out`) is
joined to the root when relative, folded (`..`), canonicalised through its
longest existing prefix (symlinks resolved) and must then lie under the root,
otherwise the call fails with `unsafe_path`.

A successful call returns `content` (a short text, then up to three image
blocks) and `structuredContent`. The run tools return the lean
`saccade-result.v1` (`verdict`, `totals`, the failing entries with up to three
hotspots, `paths`, `next_step`), the same object as `compare --json`. The image
blocks are the top explain strips as PNG, each at most 1024 px wide, unless
`include_images` is `false`. A regression is a successful call. A failed call has
`isError: true` and `structuredContent` of schema `saccade-error.v1` with
`code` `usage` (bad or missing argument), `unsafe_path` (outside the root), `io`
(unreadable or unwritable path, undecodable image or report), `config` (invalid
config) or `not_empty_out_dir`. Protocol errors use the JSON-RPC codes -32700,
-32600, -32601 and -32602. `saccade summary --format json` prints the summary
object (`saccade-summary.v1`).

## 11.1 JSON output of the CLI

`compare --json` and `identity --json` print `saccade-result.v1`; `--json=full`
prints the whole `saccade-report.v1`. Floats of the lean result have 4
significant digits. CLI output paths (`result`, `summary`, `approve`,
`view-summary`) are relative to the working directory by default; report input
provenance is relative to the report, and the explain `report` is relative to
the pack. `--record-absolute-paths` opts back in. `next_step` hints always use
working-directory-relative paths. With
`--json` or `--format json`, a failing command prints `saccade-error.v1` on
stdout (`code` one of `usage`, `io`, `config`, `unsafe_path`,
`not_empty_out_dir`, `nothing_compared`, `approve_mismatch`), with a `hint`
naming the offending path/argument and a repair, and keeps its exit
code `2`; clap's own argument errors included. A run that compared nothing is a
regression (exit `1`) reported in the result, so `nothing_compared` is reserved.
Every schema id has a file `schemas/saccade-<name>.v1.schema.json` with
`$id` `https://github.com/Tavrin/saccade/schemas/<file>`.

## 12. Local web app: `saccade serve`

`saccade serve ROOT` serves an archive of capture directories on `127.0.0.1`.
It reuses the `view` pipeline: a session is a `view` model built in the cache
directory for 2 to 6 runs, opened at `/session/<id>/`, with a progress page
while it builds.

- **Routes.** `/` (landing page), `/api/ls`, `/api/search` (path substring,
  sidecar `key=value` filter, `recent=1`), `/thumb` and `/img` (image files
  under the root only), `/compare?runs=a,b[,...][&labels=x,y][&blind=1]` (2 to
  6 runs relative to the root; `labels` has one entry per run), `/pair?a=&b=`
  (two image files), `/api/decisions` (recently saved decisions).
- **Security.** Binds `127.0.0.1` only. `Host` must be `127.0.0.1:<port>` or
  `localhost:<port>`; every POST needs a matching `Origin` and the per-process
  token in `X-Saccade-Token`. Client paths are relative to the root,
  canonicalised and must stay below it; external symlinks are followed only through explicit `symlink_targets`
  (see section 12.1). The root is read-only.
- **State.** Cache (`--cache-dir`, default `$XDG_CACHE_HOME/saccade`): sessions,
  thumbnails, staged pairs, uploads (64 MB per file). Decisions
  (`--decisions-dir`, default `$XDG_DATA_HOME/saccade/decisions`): one
  `<session-id>.saccade-decisions.v1.json` per session, written as the reviewer decides, in
  the format of section 8.1.
- **Recent runs.** Each run lists up to three sidecar chips: the keys whose
  value differs among the listed runs (timing keys are hidden), values over 20
  characters shortened with the full value in the tooltip, the rest as "+N".

### 12.1 Dashboard deep links and remote captures

Canonical routes are `/run?path=<rel>` (one capture),
`/compare?run=<rel>&run=<rel>` (2–6 inputs), and
`/runs?ref=<rel>&run=<rel>[&run=...]` (1–6 candidates). Query parsing retains
repeated values without splitting them; legacy `runs=a,b` is used only if no
`run` key is present. Generated links always use repeated `run`. A one-input
compare redirects to `/run`, or `/image` for an image. `/run` renders cached
thumbnail tiles, the configured metadata sidecar and actions using the existing
compare tray. `/image` reuses the view frontend with exactly one pane.

`/api/roots` returns `{name, path}` entries in command-line order. One root has
an empty name and no prefix. Multiple roots use directory basenames, with
`-2`, `-3`, etc. for collisions, skipping previously assigned names. `/open`
accepts repeated absolute `abs` and an optional absolute `ref`, checks lexical
containment before filesystem access, resolves each input and emits canonical
relative links. One/two image inputs redirect to `/image`/`/pair`. Optional
`labels` and `blind` survive redirects. Outside and missing paths share a
styled 404 that names the supplied path without filesystem error details.

Containment requires an unresolved path under a served root without `..`.
The canonical target must be under the original root, another served root when
`--follow-symlinks-within-roots` is enabled, or the explicit repeatable
`--symlink-target` allowlist (`symlink_targets` in TOML). Relative TOML targets
are relative to the config file. The lexical alias remains the identity used
for links, including metadata and overview thumbnail URLs. Allowlisted targets
cannot be addressed directly through `/open` unless also served as roots.

All GET storage requests, including file reads, run on helper threads with
`sync_channel(1)` and `recv_timeout`. `--fs-timeout-ms` overrides TOML
`fs_timeout_ms` (default 3000; positive values only). A shared atomic limit
allows eight active probes. A timeout does not free its slot until the operation
actually returns; saturation refuses immediately. A styled 503 says
`storage not reachable: <root-relative path>`. The landing page and root map
remain available while storage is saturated. Each comparison input is copied
through containment checks into an immutable local cache snapshot before
background view/overview jobs start, so those jobs never touch a hung mount.
Nothing is written under the archive roots; Host/Origin/token checks remain.

## 13. Library

`saccade-core` is the library behind the CLI: `compare`, `run`, `report`,
`render` (HTML and Markdown), `view`, `hdr`, `regions` and `config`. It is
documented with rustdoc (`cargo doc -p saccade-core --open`). The CLI is a
thin layer over it. The code does not panic on bad input: `unwrap`, `expect`
and unchecked indexing are not used outside tests.

## 14. Sequence format: `saccade-sequence.v1`

`sequence BASE CAP --pattern GLOB --out OUT` sorts each sequence by the trailing
integer in each matching image stem and pairs by zero-based sorted index.
Numbering origins/gaps may differ; duplicate numbers or unnumbered matching
images are rejected before writes. Config ignores apply before pairing.
Synthetic safe entry names (`frame_00000000.png`, etc.) key the per-frame report
images; `frames[]` records actual `baseline_name`, `capture_name` and their
numbers. Per-path overrides/regions/masks match the actual baseline name (or
capture name for a new frame), and metadata loads from each actual source name.

`saccade-sequence.v1.json` contains `schema`, `verdict`, `pattern`, source frame
counts, `totals`, `mean_flip_curve` (null for unmeasured pairs),
`baseline_temporal_mean`, `capture_temporal_mean`, `temporal_instability`,
`temporal_errors`, `worst_frame`, `frames_over_threshold`, working-directory-relative report/HTML
paths by default (absolute by opt-in) and `frames[]` with normal `Entry` payloads. Temporal means cover all
adjacent frames on each side using whole-image colour FLIP (HDR-FLIP for HDR).
Instability is the signed capture mean minus baseline mean. It is null when
either side has fewer than two frames or a temporal pair cannot be measured;
temporal errors are regressions. Worst frame uses mean FLIP; threshold count
uses each entry's deciding metric. Temporal instability has no separate gate.
The normal `saccade-report.v1.json` supports the HTML table and explain packs;
its synthetic entry names are not baseline-update source names.

`--json`/MCP omit the curve and `frames`; `--json=full` returns the complete
sequence object. HTML adds an SVG curve generated in Rust, with gaps for null
values. No temporal browser/serve interface or video decoding is involved.

## 15. Numerical buffer entries

`[[buffer]]` has `glob`, `kind`, `encoding`, optional `threshold`, `metric`
(default mean) and `scale` (default 1). First match wins independently of colour
overrides. Depth (`linear01`, `reverse_z`, `r32f`), normal (`rgb_snorm`, `oct`),
motion (`rg_snorm`) and discrete mask/id (`exact`) bypass FLIP. Numerical
comparisons retain native precision; EXR depth reads R directly. Size mismatch,
invalid normals or non-finite numerical samples become error entries.

The additive optional `Entry.buffer` carries `kind`, effective `encoding`,
`unit`, `scale`, deciding `metric`, `value`, `threshold`, `stats` and
`heatmap_max`. `stats` contains mean/max/p95/p99; depth adds mean/max relative
error (`abs(c-b)/max(abs(b),1e-12)` after decoding), mask/id add
`exact_match_fraction` and `changed_pixels`. Mask/id compare complete native
pixel samples and decide on changed fraction, with thresholds in [0,1]. Other
kinds decide on their configured absolute/angular/end-point statistic. FLIP
`metrics`, `hdr`, hotspots, regions and colour diagnostics remain absent.
Mask/id formats must match. A buffer's numerical value counts as a compared
pair. Metadata enforcement continues to apply.

Heatmaps use magma with zero at the bottom and `heatmap_max` at the top: 1 for
normalised depth/discrete error, 180 for normals, `2*sqrt(2)*scale` for motion,
and the larger of the observed maximum/threshold for r32f depth. Default limits
are 0.01 depth units, 1 degree, 0.5 pixels and 0 changed fraction. Reverse Z is
normalised `1-R`, without projection linearisation or camera near/far parameters.

## 16. Ranking format: `saccade-rank.v1`

`rank REFERENCE CANDIDATE...` runs normal comparisons under `<out>/<label>/`.
Candidate labels are unique safe directory components. All destinations are
checked against every input before writes; child output symlinks are refused.
The chosen metric overrides metric overrides so every ranking uses one FLIP
unit. Numerical buffer rules are not accepted in rank.

`saccade-rank.v1.json` contains `schema`, `verdict`, `metric`, `reference_dir`,
`reference_images`, `common_images`, `overall[]`, `images[]` and JSON,
HTML and Markdown paths (relative to the working directory by default). Each `images[]` has `name` and `candidates[]` with
`label`, `status`, nullable `value` and nullable `rank`. Valid comparisons rank
in increasing metric order using exact ties and competition ranks (1,1,3).
Missing/new/error values have no rank and sort last. `overall[]` holds labels,
rank, mean rank, mean metric, number of ranked images, `complete`,
`bit_identical`, normal report totals, working-directory-relative report JSON and relative HTML
link. Both overall means use the intersection of valid comparisons across all
candidates. No overall rank is published unless every reference image belongs
to that intersection. Overall order is mean rank, then mean metric, then label;
identical rank/metric tuples tie. Candidate-specific new images are listed but
do not enter overall means. Verdicts use the normal compare rules.

`ranking.md` and root HTML list overall/per-image rankings and link to child
reports. `--json` and `saccade_rank` MCP omit `images[]`; full output includes
them. Both sequence/rank MCP tools follow the root path policy, accept CLI run
settings, declare output schemas and safe-write annotations, and return no
image content blocks. Their lean floats use 4 significant digits.

## 17. Baseline-update Action

`update-baselines` defaults false. Only a trusted `workflow_dispatch` on a branch
can enter the update step; both the step condition and the shell write boundary
guard the event and fork status. Comparison must finish with exit 0/1 and have
an uploaded report/Markdown summary. The baseline path must resolve below the
workspace, and tracked/staged changes must be absent before approval.

The Action runs approve with `--all-failing` and optional `--prune-missing`
(`update-prune-missing`, default false). It uses approve's JSON list to stage
only copied/pruned paths, preserving literal filenames. With changes, it creates
`<update-branch-prefix>-<run_id>` (default prefix `saccade/update-baselines`),
commits, pushes to the event repository and opens a PR against the dispatch
branch. The existing Markdown summary (including the report artifact link) is
passed through `gh pr create --body-file`. No changed files means no branch/PR.
Requires contents write for the checkout credentials and pull-requests write
for `github-token`; errors propagate. All expressions go through step env,
with none embedded in shell scripts. The compare exit verdict remains the
Action's final verdict. No PR-comment command handler is required.

## 18. Bisect format: `saccade-bisect.v1`

`schema`, `status` (`found`, `pass`, `inconclusive`, `non_monotonic`), nullable
`first_bad`/`last_good`, `candidates[]`, `probes[]`, `total_probes`, and
`non_monotonic[]` (observed bad-before-good target pairs). Each probe has
zero-based `index`, `target` (absolute run path or immutable revision),
`verdict` (`good`, `bad`, `skip`), nullable absolute `report_dir` and `reason`.
Only an unambiguous monotonic boundary populates `first_bad`. If no probed
target passed, `last_good` uses the supplied `--good` path or good Git revision.
Skips between
last good and observed bad produce candidates, including the observed bad.
This is a search under a monotonic assumption, not exhaustive verification.
Native sample identity is the default; explicit threshold uses FLIP. Selected
entry statuses/totals are reflected in per-probe normal reports. Missing/new
images fail; decode errors/no selected images skip. CLI mode B
uses read-only Git revision queries and user `sh -c` commands, with fresh
capture directories and logs, never an implicit checkout. MCP excludes it.

Watch reuses `saccade-result.v1` as JSONL, or `saccade-error.v1` on failures.
Native recursive `notify` monitoring falls back to content-comparing polling;
initial capture and debounced events run the normal compare pipeline. MCP
watch status and best-effort log notifications share that result format.

## 19. Human inbox and ask formats

`saccade-inbox-item.v1` stores `id` (32 lowercase hex), `question`,
`allowed_answers[]`, nullable `context`, `link`, `from`, `status`
(`open`/`answered`), `created_unix`, nullable `answer`, `note`, `answered_unix`.
Question/answer body caps are bounded. Allowed answers are unique, nonempty,
1 to 32 values, at most 256 bytes each. Links are local absolute paths; the
HTTP route may normalize this server's absolute URL to a path. UI uses text
nodes and local links. Writes are atomic and concurrent answers serialized.
Items live only under decisions/inbox, never archive roots; files/symlink ids
are checked before access. GET list orders open first, newest within groups.

`saccade-ask-result.v1`: `schema`, `id`, `status`, nullable `answer`/`note`,
`timed_out`, and `url` (human inbox deep link). Timeout leaves the question
open. Ask uses private mode-0600 `serve.json` (`port`, `token`) in serve's
cache, validates the requested port, and connects directly to literal IPv4
127.0.0.1 over HTTP with no DNS, proxy or redirects. Inbox POSTs reuse serve's
Host/Origin/token gate. No answer implies a baseline approval. The schemas
for all three new formats are derived from Rust and checked for drift; focused
CLI/HTTP/MCP tests validate actual payloads against them.

## Judge panels, evidence and trust

`judge` accepts a report or a rank document and a panel TOML. Its six fixed
questions declare `checkable` (`triage`, `cause`), `rubric` (`accept`, `ask_human`,
`mask_suggest`) or `preference`; every answer set includes abstention. The
`saccade-judge.v1` document includes the kind and its limits, requests on a dry
run, and otherwise the per-call audit, merged per-judge votes, weighted panel
result, agreement, canaries, position bias and a trust explanation.

The deterministic text encoder augments decision-request evidence with an 8×8
mean FLIP grid rounded to two decimals, CIELAB nearest-neighbour English colour
names over changed pixels, diagnostic tone/shift values without runtime timings,
metadata differences, intent and optional external OCR line differences. Raw
pixels never enter text requests. Vision inputs are PNG strips of padded,
contrast-stretched hotspot crops with neutral labels, no heatmap and no full
frame fallback. Vision and human questions, and all pairwise preferences, use
both orders. Preference answers are mapped from presentation slots back to
stable candidate ids before merging. An order flip escalates; incomplete human
orders abstain. A probability omitted by a model is not treated as certainty.

Known-answer generated canaries are interleaved at a configurable rate. Judges
that fail are flagged and down-weighted. Every call, including canaries, retains
its evidence SHA-256, provider, requested/answering model and reported version,
probability/confidence, rubric version, timestamp, latency, errors and retry/
fallback attempts. Text probabilities are explicitly uncalibrated until checked
against labels. HTTP keys come from the allowed files only, never ambient API
key variables; redirects are disabled. OpenCode free-model calls use isolated
config and working directories, no inherited credentials and denied tools.

Individual answers use `propose_report`, sharing `decide` validation and storage
while keeping human panel votes as proposals. Only a settled `panel` aggregate
uses the configured gate. Deterministic failures remain refused. A calibration
threshold is schema/range checked and applies only to its source/question;
relative config paths resolve from the config directory. Baseline approval still
requires a final decision.

`saccade-calibration.v1` reports accuracy against human consensus, pairwise
agreement with humans, ECE, reliability bins, optional position bias and nominal
Krippendorff's alpha over multiple human ratings. Automatically promoted model
predictions are not human labels. Thresholds require minimum labelled support.
`saccade-judge-selftest.v1` reports repeat, order, name and crop-shift flip rates.
Rank judging fits Bradley–Terry over individual votes after order merging, with
seeded bootstrap strength/rank intervals and warnings for sparse, disconnected
or indistinguishable candidates. Preference results describe the asked
population, never objective truth.

Human runs persist at `decisions/judge/<evidence-and-panel-hash>/run.json` with
schema `saccade-judge-votes.v1`, anonymous strip files and append-only
`votes.jsonl`; latest vote per named voter/item wins. `/vote/<id>` and
`/api/vote/<id>/{items,vote}` reuse the shared server security gate, refuse unsafe
ids and symlink escapes, and expose only the current voter's responses. Each
voter's items are deterministically shuffled, with both orders; localStorage
access is guarded. Repeating the judge command includes those votes without
reusing them for changed evidence or rubrics. MCP exposes `saccade_judge` and
`saccade_judge_calibrate`, with nested input paths confined to its root.

## 20. Release ergonomics

`init` writes one of four commented templates with exclusive creation by default.
`config` uses the same explicit-file / working-directory-file / built-in selection
as compare, showing defaults, effective values and field sources. Its image
explanation uses the first matching override, regions, masks and buffer rule.
Identity still imposes its own strict command defaults as described above.

File pairs key their one entry by the capture filename, preserve both hashes
and read sidecars from the files' parent directories. Mixed file/directory inputs
are rejected. `--entries` selects a union of case-insensitive name globs before
comparison on compare, identity, view and runs; config ignores take precedence.

`saccade-entries.v1` contains full entry payloads, filtered total, offset, limit
and nullable next_cursor. The cursor is the next filtered offset; filters and
page size must remain unchanged and the underlying report must not be replaced
while paginating. MCP list/get are read-only and retain root confinement.

`saccade-noise.v1` records the metric, margin, relative run paths, entries and
warnings. Each entry records pair count, the maximum pairwise mean/p95/max and
threshold = largest deciding metric × margin. All distinct pairs are measured.
Missing/error pairs prevent calibration; suggestions are exact image-name
TOML overrides. High noise warns, and noise alone cannot establish separation
from a known changed build. Schemas are derived from Rust and validated on
real generated-fixture command output.

JUnit emits one testcase per entry with XML-escaped names and messages; fail,
error and missing fail, new skips unless fail_on_new, and pass has no child.
Rank prefixes entry names with the candidate label. Demo embeds less than 116 KiB of
example PNGs and uses the normal comparison/report pipeline.

Viewer decisions must be saved beside the viewer to retain their relative
provenance. Server saves and CLI unblind rebase paths when saving elsewhere.
Blind keys include view_dir relative to the key file, so CLI unblind can
resolve the view-relative input dirs even when --key-out is elsewhere.
Approval keeps directory and SHA-256 checks, including baseline pruning.


## 21. Run performance and ablation contracts

`saccade-perf.v1` is an optional capture-directory sidecar in milliseconds.
See [Performance evidence](../README.md#performance-evidence) for the complete
input example and commands. Frame values are finite and nonnegative, samples
are positive, and statistics are `p50`, `p95`, `p99`, `mean`, `min` or `max`.
Pass and gap values are additive; scopes require existing, acyclic non-gap
parents and never contribute to frame totals. Optional spread bounds contain
the term value. IDs are unique. Counter maps reference existing IDs and carry
finite numeric values. Unknown fields and invalid evidence fail validation as
`PerfError { path, message }`. Compare/identity record `perf_errors` and one
error entry per malformed sidecar; runs and viewer sessions carry run-level
error entries. Optional sidecars never change image pairing or thresholds.

`perf_diff` is attached once per pair at the report/run level. Its schema is
`saccade-perf-diff.v1`: `unit`, `noise_k`, `frame`, signed
`unattributed_before/after`, `terms` and `warnings`. Every delta has nullable
`before`, `after`, `delta`, `delta_pct`, `noise_floor`, `noise_threshold`, `beyond_noise` and a
`status` (`paired`, `appeared`, `disappeared`, `not_comparable`). Before zero
makes percentage null. Relative calculations that overflow remain unknown.
Exact ID, kind and parent must agree for a paired term; frame statistics must
agree for a paired frame. Term rows retain both kinds/parents, frame shares and
counter deltas; counters sort by absolute relative change, then name. Inputs
are never matched by labels or similarity. Remainders warn above 1% in either
direction. Tables nest scopes under parents and sort sibling groups; stacked
bars contain only each side's additive terms and positive remainder on a shared
millisecond scale, without pretending a negative remainder is an extra term.

Repeat calibration uses `max(value) - min(value)` across all unchanged-build
repeats. Noise JSON extends `saccade-noise.v1` with `perf_noise`; generated TOML
contains `[perf_noise] frame = ...` and `[perf_noise.terms]`. Terms absent or
structurally inconsistent across repeats have no floor and generate warnings.
Sidecars missing from only some repeats or inconsistent frame statistics abort
calibration. The raw floor is independent of the image threshold margin and of
per-capture spread bounds. Configuration accepts inline `[perf_noise]`, or a
`perf_noise`/`perf_noise_file` file path resolved relative to its config file.
CLI `--perf-noise` overrides it. `perf_noise_k`/`--perf-noise-k` defaults to 3
and is finite and positive. The timer quantum is estimated across all term
values in all repeats: sort values, take adjacent distinct differences, then
reduce with approximate Euclid (GCD), treating residuals below `1e-6` ms as
floating noise. Adjacent differences have the same GCD as all pairwise
differences. Frame durations and descriptive sidecar spreads do not enter the
estimate. No distinct values means unknown quantum. An estimate describes the
observed lattice; sparse measurements can overestimate the actual timer tick,
and mixed timing sources need an explicit override.

`perf_resolution_ms`/`--perf-resolution` overrides the estimate with a finite
positive quantum in ms. `perf_resolution_ticks`/`--perf-resolution-ticks` is a
positive integer, default 2. `perf_min_delta_ms`/`--perf-min-delta-ms` defaults
to 0.05 ms; `perf_min_delta_pct`/`--perf-min-delta-pct` defaults to 0.5 (percent).
Both minima are finite and nonnegative. The shared meaningful minimum is
`max(perf_min_delta_ms, baseline.frame.value * perf_min_delta_pct / 100)`;
it uses the baseline frame even for term and scope deltas. The effective frame
or term threshold is `max(k * repeat_range, ticks * quantum, meaningful_minimum)`.
Only `abs(delta) > threshold` counts; equality is within noise. This prevents
single-tick changes from being evidence even when repeat spread is zero.

Noise output records raw frame/term ranges and `resolution_ms` (nullable in JSON,
omitted from TOML when unknown), `resolution_ticks`, `min_delta_ms` and
`min_delta_pct` inside `perf_noise`. Older calibration files use the new defaults
and an unknown quantum. Explicit config and CLI options override loaded calibration
settings, which override defaults. `noise --config` supports the same keys and
CLI options. Generated TOML also records `perf_noise_k` at the root. Diffs retain
raw `noise_floor`, add `noise_threshold`, and record the applied settings and
`minimum_delta_ms` (the effective meaningful minimum). An unknown repeat floor
does not become zero: meaningful deltas retain null `beyond_noise`; deltas
at or below the known timer/meaningful minimum receive false. Counter deltas
do not use timing thresholds. Structurally different terms retain null deltas
and null `beyond_noise` regardless of their durations.

`saccade-ablate.v1` contains `base` and `arms`. Each arm records its path,
label, image verdict, nullable frame delta, top-N beyond-noise term rows,
config keys from the existing metadata diff, flags, combined verdict, full
perf evidence/errors and its relative report link. Arm output directories are
stable `arm-1`, `arm-2`, etc.; arbitrary labels never become output paths.
The output ownership and input-containment guards apply before writes.
The command exits 0 for completed evidence (including image changes), 2 for
malformed/incomplete comparisons or command errors. `PERF-ONLY` requires
identical images and a comparable frame OR at least one comparable term beyond
its effective threshold. `NO-EFFECT` requires identical images, a calibrated
comparable frame within noise, and all paired terms within their thresholds or
below the timer/meaningful minimum. Appeared, disappeared and structurally
changed terms alone never establish `PERF-ONLY` or block `NO-EFFECT`: they are
reported separately with a `terms differ` note, sorted by largest recorded
duration, with durations above the meaningful minimum highlighted. Unknown
meaningful paired deltas or an uncalibrated/non-comparable frame remain
`INCONCLUSIVE` when no change is established. Counters remain descriptive.
HTML, JSON and
text are written even for a completed arm with sidecar error entries.

The run overview exposes an ablation table and run-level performance tables.
Changed overview pairs use the existing diagnostics engine for FLIP classes
without writing diagnostic image artifacts; normal arm reports use the same engine.
Performance inputs and noise-file content participate in server cache identity;
position/manual staging preserves the directory sidecar. Blind viewers contain
no performance evidence. The shared report/view/runs design system supplies
performance tables and composition bars. `saccade_ablate` and pair/overview
MCP tools enforce confinement on explicit and config-provided noise paths.
Combined verdicts reach full/lean pair JSON, text, Markdown, explain summaries
(except blind packs) and MCP text/structured results. All recorded paths use
the canonical path helper and `/` separators.
