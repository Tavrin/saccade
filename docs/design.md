# flipdiff design and format reference

This document describes how flipdiff compares images and the formats it reads
and writes. Usage is in the [README](../README.md). The report schema is
`flipdiff-report.v1`; the decisions and blind-key files are
`flipdiff-decisions.v1` and `flipdiff-blind-key.v1`.

## 1. Pipeline

`flipdiff compare BASELINE_DIR CAPTURE_DIR` does the following.

1. **Clean the report directory.** It removes `flipdiff-report.v1.json`,
   `index.html` and `images/` from the report directory, and nothing else, so a
   failed run cannot leave a stale report. A report directory that is the
   baseline or the capture directory is refused (exit 2).
2. **Pair images** by path relative to each directory (section 2).
3. **Compare each pair** with FLIP (section 3) and evaluate it against its
   threshold, regions and masks.
4. **Write** the images, `flipdiff-report.v1.json` and `index.html`, then print
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

`flipdiff view` pairs 2 to 6 directories the same way.

## 3. Comparison

### 3.1 FLIP

FLIP (Andersson et al., "FLIP: A Difference Evaluator for Alternating Images",
HPG 2020) models how a human observer sees the difference between two images
shown alternately: colour, contrast sensitivity and spatial frequency, at a
given viewing distance. It produces an error map with one value per pixel in
`[0, 1]`: 0 is no visible difference, larger values are more visible.

flipdiff calls NVIDIA's C++ implementation through the `nv-flip` bindings.
Inputs are decoded to 8-bit sRGB. A 16-bit PNG is accepted and down-converted
to 8 bits.

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

The deciding statistic for an entry (`metric_used`) is `mean`, `p95` or `max`.
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
   `max(2, ceil(stop - start))`, at most 64.
3. For N evenly spaced exposures, tone-map both images (`aces`, `hable` or
   `reinhard`), encode to 8-bit sRGB and run FLIP.
4. The error map is the per-pixel maximum over the exposures.

**Approximation.** NVIDIA's reference keeps every exposure in floating point.
flipdiff quantises each tone-mapped exposure to 8 bits before FLIP, so its
values can differ slightly from the reference tool.

**Credit.** The exposure-range selection, the tone-mapping coefficients and the
overall procedure are ported from NVIDIA's FLIP reference code
(BSD-3-Clause). See [THIRD_PARTY.md](../THIRD_PARTY.md).

For HDR pairs, the report and the viewer show a display PNG tone-mapped at
exposure 0 (`images/<name>/baseline.png`, `capture.png`). The original file is
copied next to it as `baseline.orig.<ext>` and `capture.orig.<ext>`. Image
properties use linear Rec. 709 luminance. The chosen settings are recorded in
the entry's `hdr` field.

### 3.6 Identity mode

`flipdiff identity PARENT CANDIDATE` runs the same pipeline with
`mode = "identity"`, labels `parent` and `candidate`, metric `max` and
threshold 0. For every compared pair it sets `bit_identical`: whether the raw
decoded samples are equal at their native depth, including alpha, 16-bit
values and HDR NaN and negative values. A pair passes if it is bit-identical
or its value is within the threshold.

`compare` also fills `bit_identical` on every compared entry.

### 3.7 Image properties

Each compared image carries `properties`: `is_all_black`, `is_all_white`,
`mean_luminance`, `min_luminance`, `max_luminance`. Luminance is Rec. 709
weights applied to the sRGB-encoded values divided by 255, with no
linearisation. For HDR images it is linear Rec. 709 luminance, and "all white"
means every channel is at least 1.0.

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
| 1 | Regression: any `fail`, `error` or `missing` entry, or a `new` entry with `fail_on_new` |
| 2 | Usage, config or IO error |

Command-line parse errors are printed to stderr with exit code 2. Names and
error text printed in the table are escaped (`\n`, `\x1b`, ...) so a file name
cannot inject workflow commands. A closed stdout pipe is not an error.

## 5. Report directory

```
report/
  index.html                     self-contained HTML report
  flipdiff-report.v1.json        machine-readable report
  images/<name>/baseline.<ext>   copies of both inputs
  images/<name>/capture.<ext>
  images/<name>/heatmap.png      FLIP error map (magma colour map)
```

Every path in the JSON is relative to the report directory and `/`-separated,
so the directory can be zipped, moved and opened anywhere.

`index.html` is one file. CSS and JavaScript are inline, the report JSON is
embedded in a `<script type="application/json" id="flipdiff-data">` element,
and images are referenced by relative path. It makes no external request, so it
works from a `file://` URL and from an unzipped CI artifact. It follows
`prefers-color-scheme`, and each status carries a text label as well as a
colour.

## 6. Report JSON: `flipdiff-report.v1`

The Rust model is `crates/flipdiff-core/src/report.rs`. Fields added after the
first release have `serde` defaults, so older v1 reports still parse.

### 6.1 Top level

| Field | Type | Notes |
|---|---|---|
| `schema` | string | `"flipdiff-report.v1"` |
| `tool_version` | string | flipdiff version that wrote it |
| `generated_at_unix` | integer | Seconds since the Unix epoch |
| `config` | object | Effective settings, see below |
| `totals` | object | `total`, `pass`, `fail`, `new`, `missing`, `error` |
| `entries` | array | One entry per image name, sorted by name |

`config`:

| Field | Type | Notes |
|---|---|---|
| `default_threshold` | number | |
| `default_metric` | `"mean"`, `"p95"`, `"max"` | |
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
| `meta_diff` | array | Sidecar keys that differ: `{key, baseline, capture}`; a missing value is `"<absent>"` |

A region result has `name`, `rect_px` (`[x, y, w, h]` in pixels), `status`
(null when informational), `metric_used`, `threshold`, `value` (null when fully
masked) and `metrics`.

## 7. Markdown summary

`flipdiff summary REPORT --format markdown` prints:

- a first line `<!-- flipdiff-summary -->`, the marker the GitHub Action uses to
  find its pull-request comment. With `--comment-key KEY` the marker is
  `<!-- flipdiff-summary:KEY -->`. A key is ASCII letters, digits, `.`, `_` and
  `-`, at most 64 characters;
- a heading such as `### flipdiff: ❌ 2 failed · 1 new · 14 passed`, or
  `### flipdiff: ✅ 17 passed`. In identity mode the heading reads
  `identity: ✅ 12/12 bit-identical` or
  `identity: ❌ 2 differ (max FLIP 0.031 on a/b.png)`;
- a table of every non-pass entry (status, name, metric, value, threshold),
  with a row `name › region` for each failing region;
- the passing entries inside `<details>`;
- a footer that links the report artifact when `--artifact-url` is given,
  followed by `flipdiff vX.Y.Z`.

The output is capped at 60 000 bytes. If it is longer, pass rows are dropped
first, then non-pass rows, and a line says how many were left out. An error
entry's text is shown in a code span under its name.

## 8. Viewer: `flipdiff view`

`flipdiff view DIR_A DIR_B [DIR_C ...] --out view/` writes a self-contained
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
- **Region of interest.** Drag a rectangle to get the mean FLIP value and mean
  RGB per pane. Regions from `--config` appear as preset rectangles.

### 8.1 Decisions

For each image set the reviewer picks `accept`, `reject` or `needs-work`, and
may add a note. "Export decisions" downloads `flipdiff-decisions.v1.json`. The
download goes through a Blob, so it works from `file://`. Decisions are kept in
`localStorage` so a reload keeps them.

```json
{
  "schema": "flipdiff-decisions.v1",
  "seed": 7,
  "labels": ["baseline", "capture"],
  "blind": false,
  "sets": [
    {
      "name": "sphere_shadow.png",
      "decision": "accept",
      "chosen_label": null,
      "no_difference": false,
      "note": "shadow reads better",
      "roi": { "x": 60, "y": 140, "w": 120, "h": 60 },
      "timestamp_ms": 1790000000000
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

`flipdiff approve CAPTURE BASELINE --decisions decisions.json` copies the
capture of every set whose `decision` is `accept`.

### 8.2 Blind mode

With `--blind` the pane order is shuffled per image set, using `--seed` (or one
taken from the clock), and the panes are labelled "A", "B", and so on. The
page embeds only neutral directory labels (`P1`, `P2`, ...) and the seed, so
view-source reveals nothing. Pane errors do not name paths.

The true labels are written to `<out>/blind-key.json`, which the page does not
reference. Keep it away from the judge.

```json
{ "schema": "flipdiff-blind-key.v1", "seed": 7, "labels": ["baseline", "capture"] }
```

The judge picks a preferred pane or "no visible difference", and may decide
accept, reject or needs-work. "Reveal labels" is enabled once every set is
decided; it opens a file picker for `blind-key.json` and shows the true
labels. The exported decisions carry the neutral labels (`P1`, `P2`).
`flipdiff unblind decisions.json blind-key.json [--out FILE]` writes the same
file with the true labels, and fails if the seeds or label counts differ.

## 9. Metadata sidecars

Two captures can differ because the configuration differed, not because the
code did: for example, a capture preset silently injects a renderer setting the
caller never chose. A comparator that ignores configuration then reports a
"regression" or an "identity" verdict about the wrong question. Sidecars record
how each directory of captures was made, so flipdiff can show the difference
and, on request, refuse to give a verdict.

- **File.** `cost-card.json` by default (`--meta-name`, or `meta_name` in the
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
  case-insensitively, are `*time*`, `*timestamp*`, `run.id`, `*duration*`,
  `*_ms` and `*elapsed*`; `--meta-ignore` adds more. The differences are stored
  in the entry's `meta_diff`.
- **Enforcement.** With `--require-matching-meta`, an entry with a differing key
  that is not declared (`--declare KEY|GLOB,...`) becomes an `error` (its
  metrics are kept), so the run exits 1. Declared differences pass and are
  still shown. Without the flag, differences are shown and never change a
  status.
- **Display.** The HTML report shows a "config differs" badge and a key table
  per entry. The Markdown summary adds a line
  `⚠ config differs on N images: ...`. The text table marks the entry with
  `[config differs: ...]`. `view` shows a card per image set, and hides it in
  `--blind` mode.
- **Scope.** `compare` and `identity` accept all four flags. `view` accepts
  `--meta-name` and `--meta-ignore`.

## 10. Library

`flipdiff-core` is the library behind the CLI: `compare`, `run`, `report`,
`render` (HTML and Markdown), `view`, `hdr`, `regions` and `config`. It is
documented with rustdoc (`cargo doc -p flipdiff-core --open`). The CLI is a
thin layer over it. The code does not panic on bad input: `unwrap`, `expect`
and unchecked indexing are not used outside tests.
