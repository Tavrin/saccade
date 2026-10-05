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
Tesseract runtime/models are operator-provisioned. The Rust adapter below supports
explicit pinned runtime downloads.
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
Apache-2.0. Wave 6b adds the optional pure Rust adapter below. Real model
recognition remains heavy-gated and was not run during this lane.

## Optional pure Rust OCR (Wave 6b)

`ocr` now enables ocrs 0.10.4 with RTen 0.21.0, both published as MIT OR
Apache-2.0. The published crates omit separate licence files; licence metadata
was reviewed from their registry sources and recorded in THIRD_PARTY.md.
Engine licensing does not establish model licensing.

`--ocr-contract` also accepts `saccade-ocrs.v1`: `cache` (relative to the
contract), `detection`, `recognition` (pinned ModelArtifact objects with roles
of the same names and `format: checkpoint` for RTen exports), an explicit CTC
`alphabet`, and `license_evidence`. Artifacts require HTTPS URL, version, byte
count, SHA-256 and MIT/Apache-2.0 licence declaration. Models are loaded only
from freshly hash-checked bytes, with no native executable/library. Ordinary
execution is offline. `text ... --ocr-contract rust-ocr.json --download-model`
explicitly enables the shared pinned runtime cache transport; no weights are
vendored. Model licences/evidence remain supplied operator declarations.

The upstream default alphabet lacks Latin accents. An accent-capable trained
model with its matching alphabet is required; changing the alphabet alone does
not qualify recognition. Neither reviewed model licence/pin nor accent model
artifact is present in the fetched crate, so canonical model selection and
actual accent recognition remain deferred to the heavy gate. It requires
`SACCADE_W6_RUST_OCR_CONTRACT` and generated CAFÉ glyphs. The existing pinned
Tesseract accent gate remains separately required by `SACCADE_W6_OCR_CONTRACT`.

The ocrs API exposes character/word boxes but no recognition confidence.
Observations use `kind: ocrs`, absent confidence, no semantic roles/source order
and incomplete coverage. CER/WER and content/position diff work; expected-text
readability fails when confidence is absent, preserving the established
contract. The heavy Rust OCR gate asserts recognized accents, unchanged CER
and this fail-closed readability behavior. No invented confidence is emitted.
MCP continues to accept imported observations only for text; it does not
execute or download arbitrary supplied OCR runtime contracts.
