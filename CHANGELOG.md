# Changelog

This project follows [Semantic Versioning](https://semver.org/). Entries are
newest first.

## 0.1.0

First release.

- `flipdiff compare`: compares a directory of baseline images with a directory
  of captures using NVIDIA FLIP. Writes a JSON report
  (`flipdiff-report.v1.json`), a self-contained HTML report (table, side by
  side, swipe, flicker, heatmap, zoom) and a text table. Statuses `pass`,
  `fail`, `new`, `missing` and `error`. Exit code 0 (no regression), 1
  (regression) or 2 (usage or IO error). Metrics `mean`, `p95` and `max`,
  threshold and metric per image through `flipdiff.toml` overrides.
- `flipdiff identity`: identity proof for optimizations. Metric `max`, threshold
  0, and `bit_identical` per image.
- `flipdiff approve`: copy captures over baselines, by name, from a report
  (`--all-failing`, `--include-errors`, `--prune-missing`) or from a decisions
  file (`--decisions`).
- `flipdiff summary`: Markdown or text summary of a report.
- `flipdiff view`: self-contained review viewer for 2 to 6 directories (layouts,
  synchronised zoom, pixel inspector, exposure and channel controls, regions of
  interest), with accept, reject and needs-work decisions exported as
  `flipdiff-decisions.v1.json`.
- Blind mode (`view --blind`) with `blind-key.json` and `flipdiff unblind`.
- HDR: `.exr` and `.hdr` inputs compared with HDR-FLIP (8-bit-per-exposure
  approximation), tone mappers `aces`, `hable` and `reinhard`.
- Regions and masks (`[[region]]`, `[[mask]]`) given as fractions of the frame
  or as a mask image.
- Alpha handling: images with transparency are compared over black and over
  white.
- Metadata sidecars: record how captures were made, show the differences, and
  with `--require-matching-meta` refuse a verdict when they differ in a key that
  was not declared.
- GitHub Action: installs a checksum-verified release binary (or builds from
  source), runs `compare`, uploads the report, writes the job summary and keeps
  one pull-request comment up to date. Matrix jobs through `artifact-name` and
  `comment-key`.
- `examples/` with a generated baseline and capture set.
