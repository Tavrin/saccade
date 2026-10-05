# Single-image provenance and integrity indicators

```sh
saccade inspect-image received.jpg --output-size 1600x900 --crop 0,0,1920,1080 --out inspection --json
saccade inspect-image received.png --hash-index hashes/saccade-hash.v1.json --text-source observed.json --out inspection --json
```

The command emits `saccade-inspect-image.v1.json`, HTML and a weak ELA layer.
Its authenticity verdict is always unknown; generation status requires an explicit
validated signed declaration. Each indicator states
what it can and cannot show. Heuristics never label a picture real/fake or infer
AI generation. No original is modified and no network access is used.

Implemented evidence includes:

- Bounded JPEG/PNG EXIF extraction of capture/digitization/metadata time and
  declared timezone, camera/lens, software and orientation. GPS coordinates are
  withheld unless `--include-gps` is supplied. Metadata is unsigned data.
- Editing-software tags, conditional same-timezone chronology conflicts and
  thumbnail/main pHash disagreement as consistency observations. Legitimate
  edits, wrong clocks, crops and forged metadata can produce these observations.
- JPEG header quantisation tables, with exact compatibility to conventional
  Annex K luminance quality scaling. This does not identify an encoder or prove
  compression history. Tables are in encoded zigzag order, before the first scan.
- Reciprocal self-matches from the registration detector, excluding neighbours
  within 24px and grouped into 12px translation bins with >=4 matches. Up to 32
  candidate region pairs include boxes, displacement and residual. Natural
  repetition can match; rotated/projective and textureless copies can be missed.
- Error level analysis at JPEG quality 90, display gain 16, visual only and labelled
  weak in HTML. ELA depends on content and recompression; it is never a verdict.
- Near-duplicate lookup against a hash/dedupe document, inclusive pHash radius 6,
  top 20 candidates. Paths and metadata do not prove which image was an original
  or earlier; current archive source bytes are not revalidated. Embedding retrieval
  is separately available through `index query`.
- Raw-pixel adequacy for up to 32 declared output sizes/crops, quality measures
  from `assess`, and optional image-bound OCR confidence plus sampled local
  Michelson contrast. Confidence is not probability and local min/max contrast is
  not foreground/background WCAG contrast or readability proof. Raw raster
  dimensions do not apply EXIF orientation.

Wave 6b adds offline C2PA validation, known XMP/IPTC fields and raw compression/
resampling observations as described below. Full metadata formats, encoder
attribution and forensic specificity remain explicitly deferred. No recorded
watermark evidence is supplied, so generation remains unknown without an
explicit validated signed declaration.

The existing raster bounds apply. Invalid inputs, crops, stale observations or
indexes exit 2; exit 0 means the inspection executed, never publication approval.
MCP `saccade_general` / `inspect_image` mirrors image/out, explicit GPS opt-in,
archive, text-source, crop and output-size options under root containment. Images
are not returned by default. Generated quantisation, malformed metadata, GPS
privacy and duplicated-region tests are written; heavy CLI inspection is gated.

## Wave 6b offline credential and metadata adapters

Build with `credentials` to read/validate embedded JPEG/PNG C2PA manifests using
c2pa 0.90.22 (MIT OR Apache-2.0, reviewed registry manifest/README; separate
licence files omitted from that package). Default features are disabled and
only Rust-native crypto is enabled. No HTTP backend, remote-manifest feature,
OCSP fetching, native OpenSSL, file I/O or thumbnail generation is enabled.
Per-reader settings require validation after reading and disable remote and
OCSP fetch. The output projects active-manifest signer, claim generator,
actions, ingredients and validation codes; it does not dump arbitrary assertion
metadata, GPS or thumbnails. Source status and certificate trust remain distinct.

Top-level `credentials` reports validation state and integrity validity.
`ai_generation` is `declared_in_validated_signed_credentials` only when a valid
or trusted active claim explicitly declares the exact C2PA/IPTC trained-model
source type. A valid signature records a declaration, not depicted truth.
Absent/invalid/unreadable/remote-only credentials retain `unknown`. Unsigned
metadata and compression/copy-move heuristics never supply generation labels.
MCP uses the same offline adapter and keeps GPS opt-in.

Known XMP namespaces/attributes/elements now yield capture/edit timestamps,
camera/lens, software/orientation and profile names. EXIF XMP GPS is opt-in;
unknown/free-form location properties are omitted. Photoshop APP13 resource
blocks yield selected IPTC IIM dates/byline/copyright with UTF-8 coded charset
support and an explicit fallback encoding note. Malformed packets do not claim
successful parsing. Extended/compressed XMP and extended IIM lengths are
explicitly unsupported; arbitrary/full metadata extraction remains deferred.

Compression now includes conventional Annex K signature compatibility (never
exact encoder attribution), approximate decoded-pixel DCT histogram gaps using
current JPEG quantizers and at most 4096 sampled blocks, and second-derivative
energy periodicity at lags 2..16 in a centered <=512x512 raster region. These
are raw, unqualified observations; textures and single compression can mimic
them, while same-quality or misaligned recompression can escape them. They do
not infer a compression count, resize factor or authenticity verdict.

Focused generated tests exercise metadata privacy, malformed lengths,
uninformative constant images and periodic patterns. `heavy: forensics`
records sensitivity on constructed single/double/resize histories, not a
specificity qualification. Encoder attribution and discrimination qualification
remain deferred: no reviewed signature corpus or frozen positive/negative
operating policy was supplied. The signed C2PA gate requires a generated,
licence-recorded `SACCADE_W6_C2PA_ASSET` and its `SACCADE_W6_C2PA_SHA256` pin;
credential qualification is NOT RUN. No real/fake approval follows from any
successful execution.
