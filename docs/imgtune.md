# Image delivery tuning

Build `products` for local JPEG/lossless WebP and generic HTTP template adapters.
`imgtune-avif` additionally enables the pure Rust ravif/rav1e AVIF encoder and
native dav1d decoder; the coordinator must provide its system development library.
This optional native dependency never enters the default feature set.

```sh
saccade imgtune audit --urls images.txt --accept 'image/avif,image/webp,image/*' \
  --accept 'image/*' --out audit.json --json
saccade imgtune search tuning.json --out tuning-report.json --json
```

The URL list has one HTTP URL per line. Audit records each requested Accept header,
actual response Content-Type, sniffed decoded format, bytes and dimensions. Responses
that cannot be decoded are explicit errors rather than zero-size successes.

A search manifest:

```json
{
  "schema": "saccade-imgtune.v1",
  "images": [{"id":"image-1","source":"source.png","current":"current.jpg"}],
  "widths": [320, 640], "formats": ["jpeg", "webp"], "qualities": [40,60,80,95],
  "target_score": 90, "butteraugli_ceiling": null,
  "adapter": {"kind":"local"}
}
```

Relative files resolve against the manifest directory; HTTP sources/current outputs
are also supported. Originals are retained. Sources must already have normalized
orientation, opaque sRGB samples and no ICC/EXIF metadata; alpha needs a declared
background before tuning. Widths use Lanczos3 aspect-preserving resizing, bounded to
16,384 pixels per side and 64 million output pixels. A candidate is measured at the
exact declared dimensions with SSIMULACRA2 and optional Butteraugli at 80 cd/m2.

For any image delivery service, configure the generic adapter:

```json
{"kind":"url_template", "template":"{base}{path}?width={w}&format={fm}&quality={q}",
 "accept":"image/avif,image/webp,image/*"}
```

The template defines parameter names and path layout. `{base}` is the source origin,
`{path}` its encoded path; `{w}`, `{fm}`, `{q}` are declared transform parameters.
Queries on the source URL are intentionally not inherited. Unknown placeholders
fail. Requested format and actually served Content-Type/decoded format are separate
evidence. Use an explicit full template if your service routes images differently.

The lowest-bytes qualifying setting is selected **among the declared grid** (at most
512 format/quality combinations per width), without assuming monotonic scores or
byte sizes. An incomplete candidate grid cannot establish the minimum and therefore
has no selection. Local WebP uses the available permissive lossless encoder: quality
is ignored and only one candidate is measured. JPEG quality is 1–100; AVIF quality
uses the same declared scale. Reports contain per-image/width candidates, retained
hashes, bytes saved versus current output, and group totals/worst savings fraction.
Negative savings are preserved. Thresholds are numerical budgets, not a guarantee
of invisible changes or permission to publish.

Exit 0 means every image/width has a complete selected grid; 1 means an incomplete
or unsatisfied search; 2 means an invalid input/execution error. Network/AVIF fixture
tests are ignored heavy groups in `scripts/gates-wave5.sh`; no live vendor API is
needed for qualification.
