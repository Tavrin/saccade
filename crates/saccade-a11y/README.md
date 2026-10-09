# saccade-a11y

Offline accessibility checks for images, part of the Saccade project.
Licensed MIT OR Apache-2.0.

These are **pre-checks**. They are not certification, they do not inspect a DOM,
and they do not cover every accessibility requirement.

```rust,no_run
use saccade_a11y::{Options, run};
let report = run(std::path::Path::new("capture.png"),
                 std::path::Path::new("accessibility-report"), &Options::default())?;
# Ok::<(), saccade_a11y::Error>(())
```

The `auto` module provides typed options and results, AA and AAA policy, automatic
candidate regions, contrast, legibility and glyph checks, and JUnit output. The
`a11y` module keeps the declared-region configuration and the optional Gemini
suggestions, which run only when asked for. Automatic mode never calls Gemini.

The `ocr` feature turns on the pinned, detection-only PaddleOCR adapter from
`saccade-core`. It needs provisioned model files and runtime, and it never
downloads anything on its own. Without them, the report says OCR is unavailable
and a bounded, deterministic edge and stroke fallback still runs.

The measurements themselves (text quality, component-local contrast, colour
simulation and OCR) stay in `saccade-core`. Success criteria, size assumptions
and verdicts live in this crate. Core owns the embedded report schemas, which the
CLI and the standalone libraries share.

`tests/automatic.rs` renders anti-aliased multiline text in the bundled DejaVu
font on light, dark and saturated backgrounds, with colour pairs on either side
of 3:1, 4.5:1 and 7:1. It also covers mixed ink, local backings, mixed UI
outlines, and texture and gradient regressions. It reports detection counts on
this finite corpus, false PASS results, the measured rate and the ratio error.
The older bitmap test stays as a detection regression test; its perfect counts
say nothing about accuracy on other kinds of images. Known limitations: detector
misses, short, rotated or connected text, gradients, and UI semantics.

Without an explicit scale (`Options::scale_known`, or `--px-per-pt` on the CLI),
normal-text thresholds apply. With an explicit scale, text size comes from the
upper-quartile letter height, excluding dots, punctuation and accents, capped by
the detected line height. Thin strokes without a supported core plateau give a
lower bound on contrast. For the WCAG colour verdict, a lower bound below the
threshold abstains instead of failing; the separate displayed-contrast check
still fails text that renders below the threshold. Each component is measured against the ring of background around it, so
declared-region values can change with this estimator. UI boundaries are split
into 8x8 tiles. Each colour or segment needs at least two supported core pixels,
and every supported segment counts toward the verdict.
