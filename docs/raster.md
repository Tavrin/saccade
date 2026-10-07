# Native multichannel rasters and tile sets

Enable the first-party extension with `cargo install saccade --features geo`.
The extension uses a pure Rust TIFF reader, with no GDAL system dependency.
Core comparison remains independent of the extension.

```sh
saccade geo compare reference.tiff candidate.tiff --out raster-report --json
saccade geo compare reference.tiff candidate.tiff --out preview-report \
  --rgb-bands 3,2,1 --rgb-min 0,0,0 --rgb-max 4096,4096,4096 --json
saccade geo mask-metrics reference-classes.tiff candidate-classes.tiff \
  --each-label --boundary-tolerance-px 1 --out class-report --json
saccade geo tiles reference-tiles candidate-tiles --out tile-report --json
```

`geo compare` measures signed candidate-minus-reference differences in native
units, independently for every band. Reports retain exact input SHA-256,
original sample type, CRS keys, affine grid tags and nodata declarations.
The six geotransform values are origin x, x step, x skew, origin y, y skew,
y step; original GeoTIFF raster-type keys retain pixel-area/pixel-point semantics.

Both inputs must have exactly matching grid metadata and dimensions. Mismatches
return `different_crs_grid`, including both descriptions and an explicit
external reprojection/resampling instruction. Even metadata-only differences
are conservatively refused; prepare matching metadata externally if appropriate.
Untagged TIFFs use the pixel grid. Matching metadata is a prerequisite, not a
claim about registration accuracy or the meaning of the data.

Per-band statistics are mean signed delta, mean absolute delta, RMSE, minimum
and maximum signed delta. Counts distinguish valid pairs, changed pairs,
nodata on both sides, and nodata on just one side. There is no implicit numerical
quality threshold. All-nodata bands have null statistics.

Each band writes a float64 signed delta TIFF (NaN for excluded pairs) and a
change PNG: 0 equals, 255 changes, 128 both nodata, 64 reference nodata,
192 candidate nodata. Delta TIFFs are derived pixel-space artifacts: use the
report's preserved grid metadata, not inferred georeferencing in these outputs.

An RGB preview requires three one-based bands and three declared min/max
ranges, shared by both inputs. Repeated bands are allowed. Native values are
linearly scaled, clipped and rounded to sRGB bytes; the mapping is recorded.
FLIP measures this declared display interpretation, not native scientific
accuracy. Nodata on any mapped band in either input excludes that pixel;
neutralised comparisons and hatched heatmaps reuse the core implementation.
There is no automatic contrast stretch or implicit band-to-colour mapping.

`geo mask-metrics` requires one band and exact non-negative u32 class values.
It uses the existing Lane E overlap and boundary evaluator. `--class` supports
`NAME=id=1,2`, `NAME=range=1,5`, `NAME=above=0`, or `NAME=mask`. The default
class is nonzero foreground. Nodata on either side and an optional reference
`--void NAME=PREDICATE` are excluded from all counts and boundaries. Reports
include the preserved input grids and the existing mask-metrics record.

Tile trees contain only `z/x/y.png`, `.jpg`, `.jpeg` or `.webp`, using canonical
unsigned decimal coordinates within zooms 0..30. Codec changes at the same
coordinate are compared. Duplicate coordinates, malformed files, symlinks,
unrelated files and empty trees are refused. Coverage is relative to the
reference: missing and extra coordinates are listed per zoom. Shared tiles
get core RGBA FLIP metrics (alpha on black and white), decoded equality, input
hashes and a heatmap. Unmatched tiles are decoded to check input validity too.
The tile verdict changes for coverage differences or differing decoded RGBA
samples, even when an aggregate perceptual error is tiny. No tile is fetched,
rendered or resampled.

All commands require an empty output directory outside the inputs and emit
versioned linked JSON: `saccade-raster.v1`, `saccade-raster-mask.v1`, or
`saccade-tiles.v1`. Discover contracts with `saccade schema list|get|path`.
Raster/class measurements exit 0; tile differences exit 1; input or grid
refusals exit 2. Completion is evidence, not data approval.

Supported TIFF samples: signed/unsigned 8/16/32-bit integers, float32, planar
or interleaved, up to 1024 bands and 32 million scalar samples, 512 MiB encoded.
The reader requires top-left orientation and a direct black-is-zero/RGB/CMYK
sample interpretation. Nonfinite samples need an explicit nodata declaration.
Multiple IFDs/pages/overviews and non-affine/ambiguous grid definitions require
external extraction. Tile trees have at most 200,000 entries; tiles at most
64 MiB encoded and 16 million pixels. These are bounded in-memory workflows,
not streaming comparisons of arbitrarily large rasters.

MCP mirrors are a follow-up. Reprojection, vector formats, map rendering and
web map servers remain outside this extension.
