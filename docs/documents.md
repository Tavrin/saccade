# Vector and document inputs

Build with `documents` (opt-in; no default dependency change). SVG uses
resvg/usvg 0.48.1 (MIT/Apache-2.0); PDF uses pure Rust hayro 0.3.0
(Apache-2.0). No PDFium/native library, scripts or network backend is loaded.
The current hayro 0.8.0 requires Rust 1.92; the older renderer preserves the
repository's declared Rust 1.89 baseline, subject to the MSRV check.

```sh
saccade compare a.svg b.svg --dpi 144 --out vector-report --json
saccade compare before.pdf after.pdf --page-map pages.json --dpi 96 --out pages --json
```

File-pair same-render comparison streams declared corresponding pages and writes
`saccade-documents.v3.json` plus HTML and per-page registration/FLIP evidence.
Historical v1/v2 schemas remain shipped. Source SHA-256, DPI, raster engines,
alpha policy, counts, explicit correspondence and resource caps are recorded.
Missing/extra pages and page rendering errors remain failures (exit 1).
Malformed input, cap failures during intake, absent isolation/features or an
invalid/missing map exit 2 with a stable code. A page pass establishes equality
at the chosen density/metric, not original vector, text or byte identity.
Threshold, metric and alignment are supported. Default DPI is 96; range 36..600.

Multipage comparisons require `--page-map FILE`, even when counts are equal.
There is no silent positional pairing or automatic semantic inference. Single
page inputs explicitly record `single_page` correspondence. A declared map binds
both encoded SHA-256 digests and includes every page exactly once. Page numbers
are one-based; null means insertion/removal. Reordering is a declared pair:

```json
{
  "schema": "saccade-page-map.v1",
  "reference_sha256": "<64 lowercase hex digits>",
  "candidate_sha256": "<64 lowercase hex digits>",
  "pairs": [
    {"reference": 1, "candidate": 3},
    {"reference": 2, "candidate": 1},
    {"reference": null, "candidate": 2}
  ]
}
```

```sh
saccade compare before.pdf after.pdf --page-map pages.json --dpi 96 --out pages --json
```

The inserted page stays `new` and the overall result fails even when both
matched pages pass. Removed pages stay `missing`. Stale hashes, duplicate,
omitted or out-of-range pages return `document_page_map_invalid`; missing
multipage correspondence returns `document_page_map_required`. A declared map
is user-supplied evidence, not proof of semantic correspondence.

The general-image loader also accepts one-page SVG/PDF at fixed 96 DPI for
hash/dedupe, similar/index, text, assess, inspect-image and explicit registered
comparison. The ordinary directory comparator scans one-page documents at the
same density. Multipage input in these single-image paths fails explicitly;
use file-pair document compare for a page summary. This is bounded input
support, not completion of every historical comparison family's renderer route.

Every shipped document count and render operation uses a fresh Linux worker.
The parent installs OS limits with `/usr/bin/prlimit` before exec and kills and
reaps the worker at its deadline. The internal entry point refuses uncapped
execution. Parser panics, allocation failure and signals stay in the worker;
worker stderr and source diagnostics are not forwarded. Responses are bounded
and validated before pixels enter the host. Embedders must provide a trusted
`documents`-enabled CLI through `SACCADE_DOCUMENT_WORKER`, or install `saccade`
beside their executable. The CLI invokes its own binary. Linux without the
launcher, and other platforms, fail closed with `document_isolation_unavailable`.
This is resource isolation, not an OS filesystem/network syscall sandbox.

All caps are fixed and cannot be raised by input or environment:

| Cap | Default | Stable refusal |
| --- | --- | --- |
| Encoded input | 64 MiB | `document_encoded_limit` (core worker API) |
| Worker virtual address space | 512 MiB | `document_worker_resource_limit` on abnormal exit |
| Wall time per count/render, including startup | 15 s | `document_wall_time_limit` |
| Worker CPU time | 10 s soft and hard | `document_worker_resource_limit` on abnormal exit |
| Pages per document | 500 | `document_page_limit` |
| Raster/embedded image dimension | 16384 pixels | `document_dimensions_limit` |
| Raster/embedded image area | 16777216 pixels | `document_dimensions_limit` |
| Dictionary/array, indirect-reference or XML depth | 64 | `document_depth_limit` |
| PDF objects / SVG nodes | 100000 | `document_objects_limit` |
| Aggregate decoded PDF streams | 64 MiB | `document_decompressed_limit` |
| PDF lexical tokens / token bytes | 1000000 / 1024 | `document_objects_limit` / `document_unsupported` |
| Worker output file | 64 MiB + 4096 bytes | `document_worker_resource_limit` |
| Worker open descriptors / core dump | 32 / 0 bytes | `document_worker_resource_limit` |
| Parent response header | 4096 bytes | `document_worker_protocol` |
| Declared map file / pairs | 256 KiB / 1000 | invalid map / bounded-read error |

The ordinary file reader retains its existing bounded-read errors for oversized
encoded files. An abnormal exit uses a common resource code because a signal
alone cannot reliably identify the allocation or interpreter failure.

PDF preflight accepts classic xref tables, direct stream lengths, unfiltered or
single FlateDecode streams. Object/xref streams, filter arrays, other filters,
predictor/decode parameters, inline-image operators, indirect critical sizes and external streams are
refused with `document_unsupported`. The conservative inline-image token guard
can also refuse that token in opaque stream data. This intake profile avoids
unbounded upstream decoding; expanding it requires bounded decoding evidence.
Malformed xref offsets fail with `document_malformed` before the renderer can
repair them. Literal and indirect depth are checked before hayro parsing;
`/Parent` chains are checked separately from forward references to avoid
counting the normal page-tree backpointer as a cycle.
SVG supports static paths/shapes and local fragment references. Text must be
outlined; images, active/foreign content and external references fail. Both
usvg image resolvers are disabled; no host fonts are discovered. PDF missing
fonts/interpreter warnings fail the page. Hayro still has PDF semantic limits;
resource containment does not establish wide rendering fidelity.

Generate the hostile corpus with `python3 scripts/gen-document-hostile.py DIR`.
The manifest records expected codes, encoded hashes, procedural provenance and
MIT OR Apache-2.0 licence. Tests assert oversized pages, direct/reference depth,
decompression bombs (including escaped filter names), huge/inline images,
501 pages, deep parent chains and malformed xref are refused,
then render a valid document to prove the host survived. Separate fault workers
exercise OS memory limits and the parent deadline. Generated report-export and
scientific-figure pairs prove insertion and reordering without shifting pairs.

Existing MCP single-page comparison uses the worker. The new map argument has
no MCP mirror in this lane; multipage MCP intake refuses missing correspondence.
Independent security review remains required before release. Native platform
coverage beyond Linux and broad third-party PDF fidelity are not qualified.
