# saccade-geo

First-party Saccade extension for native multichannel raster measurements.

This crate depends on `saccade-core`; the core never depends on this crate.
The CLI enables it with `--features geo`. No GDAL installation is required.

## Raster inputs

Read single-image TIFF/GeoTIFF with any number of bands up to 1024:
8/16/32-bit signed and unsigned integers, and float32. Planar and interleaved
samples are preserved. Integer samples convert exactly to f64 for arithmetic.
Compressed strips and tiled TIFF storage are handled by the pure Rust reader.

The reader preserves CRS keys, affine grid metadata and nodata declarations.
Different grids are refused with both descriptions. Reprojection and resampling
must be performed explicitly by the producer, outside this crate.
Untagged rasters are compared on their pixel grid.

## Measurements

`compare` writes per-band native-unit signed difference statistics, signed
float64 delta TIFFs and categorical change PNGs. Nodata on either side is
excluded from numerical pairs; one-sided nodata changes remain visible.
All-nodata bands have null statistics, never fabricated zero errors.

An optional `RgbMapping` declares three bands and fixed native-unit ranges.
Only that explicit mapping enables shared Saccade perceptual measurements.

`mask_metrics` reads single-band class rasters and reuses Saccade overlap and
boundary scoring, excluding nodata on either side and declared reference void.

`tiles::compare` compares PNG/JPEG/WebP z/x/y trees, reporting missing and extra
tiles by zoom and shared RGBA FLIP measurements for corresponding tiles.

## Bounds and limitations

Inputs are bounded to 512 MiB and 32 million scalar samples per raster.
Tile inventories are bounded to 200,000 entries and 16 million pixels per tile.
Multiple TIFF pages/overviews, non-affine grids, colour-transforming TIFF
interpretations and nonfinite samples without nodata declarations are refused.
There is no map renderer, network tile fetcher or vector-format support.
Measurements do not approve data or establish domain accuracy.

Licensed MIT OR Apache-2.0. Procedural fixtures use the same licence.
