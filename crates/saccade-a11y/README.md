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

Measurement primitives (text-quality, ratio/cluster fit, colour simulation and OCR)
remain in `saccade-core`; success criteria, size assumptions and verdicts belong here.
Core owns the embedded report schemas, shared across CLI and standalone libraries.

The permanent generated gate in `tests/automatic.rs` builds bitmap-font text over
UI, document, web-like, HUD-like and photographic scenes without manual labels.
It scores matched boxes at IoU >=0.5, all-case contrast verdict accuracy and false
PASS counts around 3:1, 4.5:1 and 7:1. Test output gives exact finite-corpus counts
and a one-sided 95% binomial upper bound, conditional on independent cases; these
constructed correlated cases do not qualify real-world accuracy. Detector misses,
short/rotated/connected text, gradients and UI semantics remain explicit limitations.
