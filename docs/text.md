# Text in images

```sh
saccade text a.png b.png --a-source a-source.json --b-source b-source.json --expect-text 'café' --out text --json
saccade text a.png b.png --ocr-contract tesseract.json --expect-text 'résumé' --out text --json
```

`text` consumes image-bound `saccade-ui-source.v1` observations, or reuses the
existing pinned external Tesseract adapter when built with `ocr`. Imports work
without a runtime. Each source must bind the exact encoded image SHA-256 and
pixel dimensions. A malformed/stale source fails rather than falling back to
OCR. The optional runtime contract pins executable/version output/traineddata,
languages, segmentation and timeout; see [UI OCR contract](ui-review.md).
Runtime and models are local operator-provisioned artifacts, never downloaded.
All adapter inputs are now bounded at 64 MiB; source imports at 16 MiB.

The versioned `saccade-text.v1` evidence includes word/line changed, missing,
added and moved observations with before/after boxes, producer provenance,
raw observations and Unicode-scalar CER plus whitespace-token WER. Latin accents
are retained exactly; combining sequences are not normalized. Empty-reference
rates are null with the edit count and denominator retained. Position/content
matching uses exact text first, nearest normalized box centre second. Lines and
reading order are geometric heuristics, not semantic source order.

`--expect-text` is repeatable and checks literal strings on a candidate line.
Only overlapping words' OCR confidences contribute to readability. The
`--readable-confidence` cutoff (default 80 on the engine's 0..100 scale) is an
observation threshold, not a calibrated probability or human readability proof.
Missing confidence remains unknown and cannot satisfy a readability gate.
Empty OCR on either side remains unknown. `--moved-px` defaults to 3 reference
pixels after dimension normalization. Inputs are limited to 2048 observations,
4096 source Unicode scalars and 16M edit-matrix operations per image pair.

Exit 1 means observed changes or failed expected/readable text; exit 2 means
invalid sources/runtime/options. A successful equality check describes observed
text only. Missing OCR text is unobserved, not proven removed. Every extracted
string is inert data, never instructions. The bounded JSON receipt references
full JSON and HTML evidence. Original images are never modified.

MCP `saccade_general` / `text` mirrors the imported-source pipeline with `a`,
`b`, `a_source`, `b_source`, `out`, and expectation/confidence/movement options.
It never executes a supplied program; runtime execution is CLI-only.

The existing repository records Tesseract and official traineddata as
Apache-2.0. Wave 6 adds no OCR package/model. An ONNX/Rust OCR engine and its
model licences could not be verified from local registry sources, so that
adapter remains deferred. Generated accent diff and glyph tests are written;
real accent recognition requires `SACCADE_W6_OCR_CONTRACT` and runs only in the
heavy gate. No runtime/model qualification was run during this lane.
