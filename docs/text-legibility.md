# Text legibility across variants

Build with `--features text-quality`. Supply a baseline, one or more variants
and declared baseline text rectangles in capture pixels. No font metadata,
domain vocabulary or semantic judgement is required.

```sh
saccade text-legibility baseline.png light.png dark.png scaled.png \
  --region 12,12,396,62 --json
```

`saccade-text-legibility.v1` includes encoded input hashes, baseline results,
and every region for every variant. Rectangles map proportionally between
capture dimensions, with outward rounding. Captures with layout changes need
external alignment or separate runs with the corresponding regions. Regions
must be nonempty and entirely inside the baseline. At most 64 regions and 64
variants are accepted; missing/unreadable inputs are errors, never discarded.

The default policy requires luminance contrast ≥ 4.5, component body height
≥ 8 sampled pixels, normalized sharpness ≥ 0.35, and stroke runs ≥ 1 pixel.
Override with `--minimum-contrast`, `--minimum-x-height-px`,
`--minimum-sharpness` and `--minimum-stroke-px`. All effective values are recorded.
Contrast uses linearized sRGB relative luminance from actual foreground and
background pixels, `(lighter + 0.05) / (darker + 0.05)`. Glyph-body height is
a minimum connected-component height proxy, not a font's typographic x-height.
Sharpness measures adjacent-pixel edge steps relative to foreground/background
range. Stroke sampling measures horizontal/vertical foreground run lengths.
Sharpness and contrast retain the worst measurable component; stroke sampling
retains the minimum of per-component lower-quartile run widths. These are deterministic
sampling observations, not human readability or WCAG compliance certification.

An opaque dominant background and separable glyph bodies are required.
Missing text, transparent pixels, mixed backgrounds, clipped foreground and
analysis-limit exhaustion yield `insufficient_evidence`. A variant cannot be
qualified against a baseline that did not establish legible pixel evidence.
Measured failures yield `illegible` and reasons `low_contrast`, `too_small`,
`blurred`, `undersampled_strokes` or `missing_glyph_candidate`. Thresholds
satisfied in all supplied regions yield `legible`; glyph completeness and human
readability remain unverified. Failures dominate abstentions in the overall state.

`--ocr` uses the cached PaddleOCR adapter when available; it never downloads.
Alternatively supply `--baseline-source FILE` and one `--variant-source FILE`
per variant with image-bound OCR observations. Sufficiently confident text
contained in each region is compared by exact Unicode scalars, in geometric
order. Observed disagreement makes the variant illegible. Missing text or
confidence below 80 is explicitly unavailable, never agreement. Text is inert
data; OCR success does not prove readability or script support.

`--json` emits the full report; `--out DIR` optionally saves it. Exit 0 means
all declared pixel thresholds passed, 1 means a measured failure, 4 means
insufficient evidence, and 2 means usage/input/feature error. OCR availability
is reported independently. Run `scripts/text-quality/fixtures.py --out DIR
--binary PATH` for generated truth fixtures and explicit cached-OCR skip reasons.
