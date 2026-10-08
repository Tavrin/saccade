# saccade-a11y

First-party offline image accessibility policy for Saccade. MIT OR Apache-2.0.
These are **PRE-CHECKS**, never certification, DOM accessibility or complete coverage.

```rust,no_run
use saccade_a11y::{Options, run};
let report = run(std::path::Path::new("capture.png"),
                 std::path::Path::new("accessibility-report"), &Options::default())?;
# Ok::<(), saccade_a11y::Error>(())
```

`auto` exposes typed options/results, AA/AAA policy, automatic candidates, contrast,
legibility, glyph triage and JUnit. `a11y` retains the declared-region configuration
and optional explicit Gemini suggestions. Automatic mode never calls Gemini.
`ocr` enables core's pinned detection-only PaddleOCR adapter; provisioned artifacts
and runtime are required, with no implicit downloads. Without them, availability is
reported and a bounded deterministic edge/stroke fallback still runs.

Measurement primitives (text-quality, component-local contrast, colour simulation and OCR)
remain in `saccade-core`; success criteria, size assumptions and verdicts belong here.
Core owns the embedded report schemas, shared across CLI and standalone libraries.

The permanent gate in `tests/automatic.rs` renders bundled DejaVu anti-aliased
multiline text on light, dark and saturated backgrounds, with threshold neighbours
at 3:1, 4.5:1 and 7:1. It also checks mixed ink, local backings, mixed UI outlines,
texture and gradient regressions. It reports finite-corpus detection counts,
false PASS, measured rate and ratio error. The older bitmap test remains a
detection regression; its perfect counts do not qualify cross-domain accuracy. Detector misses,
short/rotated/connected text, gradients and UI semantics remain explicit limitations.

Without explicit `Options::scale_known` / CLI `--px-per-pt`, normal-text
thresholds apply. With explicit scale, the existing minimum glyph-body size proxy
is retained. Thin strokes without a supported core plateau yield lower bounds;
below-threshold lower bounds abstain rather than FAIL. Each component uses its
local ring background; declared-region values can change with this estimator.
UI boundaries are divided into 8x8 tiles, with at least two supported core
pixels per colour/segment; every supported segment contributes to the verdict.
