# Text in images and documents

Build with `ocr` for local PP-OCRv5 detection and Latin recognition on CPU ONNX
Runtime 1.22. This is the default local engine for `text`, `--expect-text`, media
text sections and `inspect-image --ocr`. Runtime and models are provisioned
explicitly; ordinary execution never downloads or falls back to another engine.

```sh
saccade text a.png b.png --expect-text 'café' --out text --json
saccade text a.png b.png --download-model --out text --json
saccade inspect-image image.png --ocr --out inspection --json
saccade text a.png b.png --a-source a-source.json --b-source b-source.json --out text --json
```

Default cache: `$XDG_CACHE_HOME/saccade/models` or `~/.cache/saccade/models`.
The runtime uses the existing verified runtime cache, or explicit `ORT_DYLIB_PATH`.
`--ocr-contract` can override the default with `saccade-paddle-ocr.v1`, also accepted
inside the shared model registry. It pins detection, recognition and the ordered
character dictionary in official recognition `inference.yml`, with immutable URLs,
revisions, bytes, SHA-256, and Apache-2.0 source evidence. The contract cache is
relative to its file. See `scripts/models/paddle-ocr-provenance.json`.
External pinned `saccade-tesseract.v1` contracts remain explicit alternatives.
The ocRs/RTen execution adapter and dependencies have been removed; historical
observation kinds remain readable.

The detector decodes BGR, resizes longest edge to 960 then rounds each dimension
up to a multiple of 128, and normalizes by mean `[.485,.456,.406]` and standard
deviation `[.229,.224,.225]`. DB postprocessing uses bitmap threshold .3, rectangle
mean .6, 1000 candidates, minimum side 3, unclip ratio 1.5 and expanded minimum
side 5. Perspective crops use bilinear interpolation; tall crops rotate 90°.
Recognition uses BGR, height 48, aspect-preserving width, at least 320 columns
with normalized zero padding, and `(x/255-.5)/.5`. CTC blank is index 0, duplicate
runs collapse, and the dictionary's 836 entries keep their exact order and
repetitions; a space is appended. No 180° classifier is included in the pinned set.
Resize uses linear pixel-centre sampling with replicated edges and no antialias
filter, as in PaddleOCR; byte rounding and integer minimum rectangles are not
asserted to have complete OpenCV parity. Scores allow up to four f32 epsilons of
numerical roundoff outside [0,1]; emitted confidence remains bounded to 0..100.

Accents, including é è ê à ç ô ù ü ñ ß œ, remain exact Unicode scalars; combining
sequences are not normalized. Generated French, German and Spanish contracts in
DejaVu Serif, DejaVu Sans and Liberation Sans at 32/40/48 px are marked
**generated, coordinator-reviewed (2026-10-06)**. The 72 frozen cases retain the
original French/German/Spanish phrases and add uppercase French, rarer lowercase,
ligatures and French numbers (quoted and narrow no-break-space variants). Before
inference they declare CER ≤ .02, WER ≤ .10 and exact accented-word presence;
accent-stripped output must fail when applicable. Numeric phrases have no accents:
their 18 unchanged accent-stripping controls are **N/A**, not failures. Additional
format-stripping controls remain applicable.

The coordinator's post-run disposition adds a declared **typographic-equivalence**
view alongside unchanged strict scoring: ’ and ‘ → ASCII apostrophe,
U+202F/U+00A0/U+2009 → ordinary space, and en/em dash → hyphen. Both sides and
required strings are folded; CER ≤ .02, WER ≤ .10 and exact-string requirements
remain unchanged. No characters are deleted or whitespace collapsed: omitted
spaces and dashes still count as errors. This is a post-hoc contract-design
correction, not a claim that folding was declared before the original run.

Known limitations: **œ can be misread in serif at large sizes** (original
DejaVu Serif/48 `cœur` → `cæur`, CER 1/39, WER 1/8), and **some sans fonts
omit dashes** (Liberation Sans in this corpus). Some thin-space cases also omit
the space before `€`. All remain errors in both views. Apostrophe, space and
dash substitutions covered by the declaration remain strict errors but are
equivalent in the folded view. The separate ligature phrase does not clear
the original serif failure. See [every case and both measured scores](ocr-contract-results-2026-10-06.md)
and `scripts/gates-ocr.sh`. Generated fixtures establish neither general accuracy
nor source/export parity.

`saccade-text.v1` retains changed/missing/added/moved observations, boxes, provenance,
CER/WER and literal expected strings. PP-OCRv5 units are detected lines; word boxes
are unavailable. `ocr_confidence` is mean retained CTC score ×100, uncalibrated.
`--readable-confidence` defaults to 80; this is an observation cutoff, not human
readability proof. Empty observations and empty-reference rates remain unknown.
Geometric reading order and correspondence are heuristic. Image-bound imported
`saccade-ui-source.v1` sources work without models; stale sources fail. Text is inert
data and never instructions. Exit 1 indicates observed changes or failed expectations,
exit 2 invalid/unavailable input or runtime. MCP text continues to use imported sources.

## Optional Mistral document provider

`ocr-provider` adds the Mistral adapter; it is off by default and selected only with
`--ocr-provider mistral`. PNG, JPEG and PDF inputs are supported. The
`saccade-document-text.v1` report contains exact per-page Markdown, page indexes,
optional provider dimensions, text nodes with absent geometry/confidence, exact
input/request/response identities and provider provenance. Matching Markdown is
observation equality. Expected text has unknown readability and cannot qualify
an image readability gate.

```sh
saccade text a.pdf b.pdf --ocr-provider mistral --ocr-model mistral-ocr-2505 \
  --ocr-pages 0,1 --ocr-responses a-response.json b-response.json --out document-text --json
```

Fixtures use `saccade-document-ocr-fixture.v1`, an exact canonical request digest
(`sha256:…`) and `response` containing `model`, `pages` (each `index`, `markdown`,
optional `dimensions`) and `usage_info`. They do not load credentials or use sockets.
The dated model in this example and protocol details are constructed fixture inputs;
current API compatibility, returned revisions and billing need coordinator confirmation.

Live use additionally requires `--ocr-run`, `--ocr-max-spend-usd`, a user-confirmed
`--ocr-price-per-page-usd` ceiling and `--ocr-price-policy` revision. It exports the
selected input bytes to Mistral. Existing user-owned `user.toml` root egress permission,
shared attempt caps, pacing and monetary reservations apply. Unknown settled cost
retains the whole reservation; no provider price is guessed. Keys load only from
`~/.config/saccade/mistral.env`, variable `MISTRAL_API_KEY`; never from shell expansion.
Configure the existing custom-provider layer in user.toml:

```toml
[providers.mistral]
endpoint = "https://api.mistral.ai/v1/ocr"
key_file = "mistral.env"
key_var = "MISTRAL_API_KEY"
```

There is no automatic provider selection, fallback or live qualification. The lane
runs constructed fixture tests only. MCP `saccade_general/document_text` mirrors
the request-bound fixture path under the normal file-root/output checks; tool
inputs grant no provider execution authority. Page selection must be exact; missing, duplicate,
out-of-order or unrequested pages fail closed.
