# Document inputs: deferred adapter

The single-page renderer interface records input format, DPI and page index,
with 64 MiB encoded inputs, 36..600 DPI, <=500 pages and <=16M pixels per raster.
Multi-page callers must stream pages rather than hold every raster in memory.
Unavailable rendering is a typed error, never a blank image or a successful
comparison. SVG adapters must disable scripts and external resources.

A concrete SVG/PDF adapter and page-level comparison summary are deferred.
No `resvg`, `usvg`, `pdfium-render` or pure Rust PDF renderer source exists in
the permitted local registry, so the required engine/licence verification cannot
be completed without external source provisioning. No guessed licence, native
renderer or unimplemented Cargo feature is advertised. Existing commands still
accept their supported raster inputs; they do not yet rasterize SVG/PDF.

The planned adapter choices are resvg/usvg for SVG and dynamically loaded pdfium
for PDF, contingent on local source/licence review. A pure Rust PDF renderer may
replace pdfium if its supported page semantics and permissive licence are proven.
Every comparison family still needs to route through the adapter, with declared
DPI, source hashes, page numbers, missing/extra-page failures and summary counts.

Generated SVG/PDF contract fixtures are written in the ignored
`heavy: documents-deferred` CLI gate. They assert actual successful rendering and
will fail until the adapter/routing is completed; they are acceptance work, not
an unavailable-backend test pretending to qualify rendering. The coordinator
must provision/review source, implement the adapter and run that gate before
claiming Wave 6 document acceptance.
