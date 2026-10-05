# Single-image provenance and integrity indicators

```sh
saccade inspect-image received.jpg --output-size 1600x900 --crop 0,0,1920,1080 --out inspection --json
saccade inspect-image received.png --hash-index hashes/saccade-hash.v1.json --text-source observed.json --out inspection --json
```

The command emits `saccade-inspect-image.v1.json`, HTML and a weak ELA layer.
Its verdict and AI-generation status are always unknown. Each indicator states
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

The following portions remain explicitly deferred: C2PA manifest validation,
signer/claim generator/actions/ingredients and signed AI assertions; full XMP/IPTC
parsing; encoder signatures, double-compression and resampling detectors and their
constructed qualification. The permitted registry has no `c2pa` source, so its
licence and dependency surface could not be reviewed. JUMBF container presence
is merely an unvalidated header observation, never a signed credential. ICC/XMP/
APP13 presence and hashes are recorded without parsing or validation claims.
No recorded watermark evidence is supplied, so AI generation remains unknown.

The existing raster bounds apply. Invalid inputs, crops, stale observations or
indexes exit 2; exit 0 means the inspection executed, never publication approval.
MCP `saccade_general` / `inspect_image` mirrors image/out, explicit GPS opt-in,
archive, text-source, crop and output-size options under root containment. Images
are not returned by default. Generated quantisation, malformed metadata, GPS
privacy and duplicated-region tests are written; heavy CLI inspection is gated.
