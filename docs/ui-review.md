# UI and AI-agent change review

Run one review of source text/layout and protected visual regions:

```sh
saccade review ui before.png after.png \
  --reference-source before-source.json --candidate-source after-source.json \
  --box 20,20,80,40 --out ui-review.json
```

A previously frozen region can replace `--box` with `--region frozen-region.json`.
The packet includes source findings and existing localized inside, boundary and
protected-complement measurements. Outside preservation compares exact native
samples by default. `--perceptual-outside --maximum-outside-flip 0.01` declares a
perceptual alternative without suppressing text findings. Exit 1 requests review
for any source/OCR change, collateral change, missing intended change or
incomplete source coverage. Completion never approves an agent's work.

Source documents use `saccade-ui-source.v1`, an exact screenshot SHA-256,
`dimensions`, `kind` (`dom` or `accessibility_tree`), `producer`, `complete` and
`nodes`. Each node needs a stable `id` and exact `text`; optional `role`,
`bounds` [x,y,width,height], `reading_order`, `keyboard_order` and `disclosure`
retain producer facts. Coordinates are screenshot pixels. A producer must export
actual semantic traversal, not infer reading order from geometric sorting.
Null ordinals mean unavailable. IDs and non-null ordinals must be unique.
Prices, punctuation, identifiers and Unicode content are compared exactly.

```json
{
  "schema":"saccade-ui-source.v1",
  "capture_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "dimensions":[800,600], "kind":"dom", "producer":{"name":"my-playwright-capture","version":"1"}, "complete":true,
  "nodes":[{"id":"price","text":"€19.99","role":"text","bounds":[20,20,80,20],"reading_order":0,"keyboard_order":null,"disclosure":false,"ocr_confidence":null}]
}
```

Replace the illustrative hash with the actual screenshot hash. Source metadata
is checked against each screenshot; it remains a producer assertion, not an
independent recapture. Disappeared disclosures are asserted only with complete
candidate source coverage. Partial captures instead report `node_not_observed`.
Relative order of common nodes avoids calling deletion-induced renumbering a
reorder. Text, layout and source role/order changes remain distinct.

Playwright tests can attach a reference/candidate source array as
`saccade-ui-sources-0` (increment the suffix for subsequent sorted snapshots).
The supplied reporter carries it into additive `ui_sources` manifest metadata.
Ingest validates both screenshot hashes and dimensions and writes
`ui-sources/0000-0.json` and `ui-sources/0000-1.json`, listed in
`playwright-mapping.json`. Use those files and the corresponding ingested images
in the command above. The CLI does not execute browser code. Capture metadata
for the baseline must come from that baseline's capture, not the candidate DOM.

If source metadata is absent, enable the optional CLI `ocr` feature and supply
`--ocr-contract tesseract.json`. A supplied source takes priority; stale or
malformed source evidence is an error and is never quietly replaced with OCR.
Only Tesseract is supported. The `saccade-tesseract.v1` contract includes
`executable`, `executable_sha256` (`sha256:` digest), `version`, `version_output_sha256` (exact `--version` output),
`tessdata`, a `models` map from language names to exact traineddata digests,
`psm` (6 or 11), and `timeout_ms` (1..60000). Paths are relative to the contract.
The executable hash and version output fingerprint are verified. The version
output records linked-library versions; individual library binaries are not pinned.
Model bytes are hash-checked and staged in a private temporary directory, with
explicit languages, segmentation settings and bounded process time/output.
The runtime and official traineddata are Apache-2.0; nothing is downloaded or
bundled automatically. Record the local native dependencies' licences too.

OCR TSV retains word boxes and confidence as observations. Even confidence 100
cannot prove an identifier, price, disclosure, semantic order or complete
source coverage. OCR IDs identify word slots and may shift when segmentation
changes. OCR disagreements require inspection of the original captures.
Constructed tests cover exact identifiers, prices, punctuation, accents and
removed disclosures; a live OCR corpus across small/rotated text remains unrun.
No Tesseract binary or weights were available in the implementation environment.

References: [Tesseract TSV](https://tesseract-ocr.github.io/tessdoc/Command-Line-Usage.html),
[quality limits](https://tesseract-ocr.github.io/tessdoc/ImproveQuality.html), and
[official fast models](https://github.com/tesseract-ocr/tessdata_fast).
