# Vector and document inputs

Build with `documents` (opt-in; no default dependency change). SVG uses
resvg/usvg 0.48.1 (MIT/Apache-2.0); PDF uses pure Rust hayro 0.3.0
(Apache-2.0). No PDFium/native library, scripts or network backend is loaded.
The current hayro 0.8.0 requires Rust 1.92; the older renderer preserves the
repository's declared Rust 1.89 baseline, subject to the MSRV check.

```sh
saccade compare a.svg b.svg --dpi 144 --out vector-report --json
saccade compare before.pdf after.pdf --dpi 96 --out pages --json
```

File-pair same-render comparison streams corresponding pages and writes
`saccade-documents.v1.json` plus HTML and per-page registration/FLIP evidence.
Source file SHA-256, declared DPI, raster engines, alpha policy, page numbers,
counts and page artifacts are recorded. Missing/extra pages and rendering
errors remain failures (exit 1). Malformed input/options or an absent feature
exit 2. A page's comparison pass is equality under the chosen density/metric,
not original vector, source text or byte identity. Pages match by ordinal;
there is no semantic page alignment. Threshold, metric and explicit alignment
are supported; evidence/config/intent options requiring the ordinary report
are refused. Default document DPI is 96; allowed range is 36..600.

The general-image loader also accepts one-page SVG/PDF at fixed 96 DPI for
hash/dedupe, similar/index, text, assess, inspect-image and explicit registered
comparison. The ordinary directory comparator scans one-page documents at the
same density. Multipage input in these single-image paths fails explicitly;
use file-pair document compare for a page summary. This is bounded input
support, not completion of every historical comparison family's renderer route.

Limits: 64 MiB encoded input, 500 pages, 100000 PDF objects/SVG nodes, 16M pixels
and 16384 per dimension per raster. SVG supports static paths/shapes and local
fragment references. Text must be outlined; `<image>`, active/foreign content
and external references fail rather than disappearing. Both usvg image
resolvers are disabled, including its default local-file resolver. No host
fonts are discovered. PDFs requiring unavailable non-embedded fonts or reported
interpreter warnings fail the page. Hayro 0.3.0 does not support every PDF
semantic (its source documents blending/isolation/encryption limitations);
wide PDF correctness and independent worker memory/time isolation remain
unqualified. Byte/page/raster bounds do not bound every upstream decompression
or interpreter allocation. Do not treat the adapter as a hostile-document sandbox.

MCP `saccade_general` operation `documents_compare` mirrors reference/capture,
output, DPI and threshold under registered roots. No native execution authority
is needed. The ignored `heavy: documents` gate compares generated nonblank SVG
and two-page PDF fixtures and checks pixel content, summaries and missing-page
failures. It is implemented, not run during development.
