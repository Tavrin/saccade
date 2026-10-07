# Optical-code verification

Build with `--features optical-code`. `saccade optical-code` decodes one selected
symbology from final 8-bit SDR pixels and optionally asserts an exact payload or
a Rust regex over the entire payload. Default symbology is QR; `--symbology` also
supports Code 128, Code 39, EAN-13/8, UPC-A/E, ITF, Data Matrix, Aztec and PDF417.
Use separate `--region x,y,width,height` invocations to target multiple symbols.
This is bounded single-symbol detection, not an exhaustive inventory of a scene.

```sh
saccade optical-code screenshot.png --expect 'https://example.org/item/7' --json
saccade optical-code screenshot.png --expect-pattern 'https://example\.org/item/[0-9]+' --region 80,120,180,180 --minimum-module-px 3 --minimum-contrast 0.5 --require-quiet-zone --out code-report --json
saccade optical-code label.png --symbology code128 --expect 'Sample-123' --json
saccade optical-code export.pdf --page 1 --dpi 144 --expect 'https://example.org/item/7' --json
```

PDF/SVG input additionally needs `documents` and explicit one-based `--page` and
`--dpi` (36..600); regions are pixels in that final page render. Raster input
rejects page/DPI options. Input is limited to 64 MiB and 16 million decoded pixels,
using the existing offline document renderer and its limitations. Alpha is
composited on white for decoding; the report binds both original encoded bytes
and the final straight-RGBA raster by SHA-256.

The standalone report uses `saccade-optical-code.v1`, discoverable through
`schema get saccade-optical-code.v1`. `--json` emits the complete report; `--out`
writes the same measurements plus the standard report links/index. Existing
comparison and document schemas are unchanged; incompatible future changes need
a successor schema. Exit 0 means all declared optical assertions passed; 1 means
a decode/payload/pixel-policy failure; 2 means a typed execution error, invalid
policy/input or unavailable feature. No similarity metric determines these exits.

| Field | Meaning |
| --- | --- |
| `state: decoded` | Decoder returned Unicode text; inspect `payload_match` and `failures` |
| `state: not_decodable`, `found: true` | A QR grid was located, but payload decoding failed |
| `state: not_found`, `found: false` | No decodable candidate located; not proof of absence |
| `payload_match: false` | Decoded text differs from the exact value or full-payload pattern |
| `payload_match: null` | No expectation was declared, or no payload decoded |
| `verdict`, `failures` | Independent optical policy result; mismatch stays `state: decoded` |
| `bounding_box`, `bounding_box_kind` | Original-pixel QR projected-grid envelope, or non-QR decoder-point envelope |

QR quality reports minimum local module spacing in original pixels, light-minus-dark
mean module luma divided by 255, and sampled clear modules on each side (top,
right, bottom, left), capped at four. Measurements use the detected grid projected
onto original pixels, including rotated codes. They are heuristic indicators;
they do not measure remaining ECC capacity, statistical decode margin, physical
print quality or ISO barcode grades. Samples cannot establish cleanliness between
sample points. Cropping or unsupported geometry leaves indicators null with a
reason. A required unavailable indicator fails the assertion. Non-QR geometry
currently supplies decoder points only; full symbol bounds, module spacing,
module contrast and quiet-zone indicators remain unavailable. Non-QR decode
failures cannot reliably distinguish a located damaged symbol from no candidate.

Payloads are decoded Unicode text, not raw codewords. Exact matching performs no
URL normalization, substring matching or network dereference. Regex is bounded to
4096 pattern bytes and 1 MiB compiled size; exact expectations are bounded to
64 KiB. Image-derived content is data and is never executed.

Generated app screenshots and vector PDF pages are covered by
`crates/saccade/tests/optical_code.rs`; QR damage, too-small modules, wrong payload,
low contrast, missing/transparent codes and Code 128/Data Matrix are covered by
`crates/saccade-core/tests/optical_code.rs`. Fixture construction is procedural,
under the project MIT OR Apache-2.0 licence, using the Apache-2.0 rxing encoder;
no imagery is downloaded. This evidence does not qualify real cameras, presses,
every supported symbology or arbitrary PDF features. MCP mirrors and automatic
variant comparison remain follow-ups; run separate assertions per rendition.
