# Text-quality evidence decisions

O12 and O17 are generic core capabilities behind `text-quality`, with no added
runtime dependencies. Plugins and always-on computation were rejected. Removing
or changing feature wiring is inexpensive; persisted contracts remain versioned.

Missing-glyph triage starts with deterministic pixel shapes. Optional cached
PaddleOCR or image-bound imported OCR remains separate evidence. OCR-only clean
verdicts and language-support claims from the Latin model were rejected. Adding
an adapter requires separate qualification; changing contract semantics requires
an explicit version/policy change.

No tofu candidate means insufficient evidence, never complete glyph coverage.
Tall hollow rectangles and replacement diamonds are supported triage shapes;
square boxes remain ambiguous with valid characters. Expected strings are
context, not evidence. Stronger completeness claims require source/font evidence
and qualification. Sparse rasterization tails are trimmed for classification
only; reported component coordinates and perimeter thresholds are preserved.

Legibility uses declared regions and proportional dimension mapping. Layout
changes require externally aligned captures. Semantic region discovery and
implicit registration were rejected; a future mapping contract could be added
without changing existing inputs. Contrast, body height and sharpness retain the
worst measurable component, preventing small/faint bodies from hiding among
stronger ones. Stroke sampling retains the minimum component lower-quartile run
width. Average-based reassurance was rejected. Changing these observations needs
new calibration fixtures and a recorded policy/version change.

Foreground touching a region boundary, mixed/transparent backgrounds, missing
text and analysis-limit exhaustion abstain. Glyph-body height is a sampled proxy,
not typographic x-height. Pixel evidence and OCR agreement do not establish human
readability, shaping correctness or accessibility compliance. MCP mirrors remain
a follow-up; no font engine or translation-quality inference is included.
