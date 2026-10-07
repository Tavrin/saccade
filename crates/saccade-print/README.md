# saccade-print

First-party Saccade extension for ICC-managed CMYK raster comparison.
Uses vendored MIT-licensed Little CMS; no runtime system CMM installation.
The CLI enables this library with `--features print`.

Compare 8/16-bit CMYK TIFF, 8-bit CMYKA TIFF, Adobe CMYK/YCCK JPEG,
and a conservative single-image PDF raster subset. Each input needs an embedded
CMYK ICC profile or an explicit override. RGB is explicitly not a print input.

Reports contain CIELAB D50 delta E 2000 statistics, ink separation maps,
TAC masks, optional target-profile CMM gamut checks and small
four-colour mark candidates. These are measured diagnostics, not print acceptance.

See the repository `docs/print.md` for policy, limits and CLI examples.
Synthetic fixtures and their generated ICC profile are original project code,
licensed MIT OR Apache-2.0, and do not characterize a physical printing device.
