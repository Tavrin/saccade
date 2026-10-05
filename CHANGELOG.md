# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Add experimental `review explain`, `review audit-mask`, `review check-ui` and
  `review assist batch submit|status|collect`; optional Jev routing remains unqualified.
- Add the local Playwright matcher, `sweep plan|compare`, `imgtune audit|search`,
  `design pull|compare`, `notify` and verified `--baseline last-good` lookup.
- Add explicit registration/resampling, `hash`, `dedupe`, `similar`,
  `index build|query|export-inputs|calibrate`, `text`, `assess`, `inspect-image`,
  `capabilities` and comparison question routing.
- Add optional SVG/PDF rendering, offline C2PA validation and pinned Rust OCR.
  Model, OCR readability, forensic specificity and broad renderer qualification
  retain the limits recorded in their evidence and documentation.
- Gate Butteraugli AVX-512 dispatch on compiler support while retaining Rust 1.88.

## [0.1.2]

Fixed: MCP Registry name uses the case-sensitive io.github.Tavrin namespace.

## [0.1.1] - Unreleased

### Added

- Claude Code and portable Agent Plugins packages, marketplace catalogs, and
  local manifest validation.
- MCP Registry metadata and the Cargo package README ownership marker.
- Release checks for the source, length and contents of packaged READMEs.

### Changed

- The `saccade` crates.io page uses the full repository README.
- `saccade-core` has library documentation with a tested Rust example and
  feature descriptions.

### Fixed

- Windows capture shell syntax in `experiment bisect` (`c8bb6da`).
- Regenerated the CLI reference after the bisect help change (`966ecc5`).

## [0.1.0]

First public release.

### Added

- `compare`: pair baseline and capture images by relative name and score each
  pair with NVIDIA FLIP (mean, p95, p99 or max). Writes an HTML report (table,
  side by side, swipe, flicker, heatmap, zoom, numbered hotspots), a JSON
  report (`saccade-report.v1`), a text table and optional JUnit. Exit 0 means
  no image regression, 1 an image regression, 2 that the command could not run.
- `identity`: exact native decoded-sample equality for optimizations.
- `approve`: copy reviewed captures over baselines from a report or a
  decisions file, with `--dry-run` plans, SHA-256 checks and explicit
  `--prune-missing`.
- `view`: a self-contained viewer for 2 to 6 image directories, with blind mode
  and accept/reject/needs-work decisions. `serve`: a local workbench for report
  and image archives.
- `inspect`: bounded, paginated reading of reports (`--entry`, `--status`,
  `--validity-reasons`, `--cursor`), plus `inspect evidence`, `inspect export`,
  `inspect config` and `inspect capabilities`.
- `review`: local review previews with exact payloads, token and optional cost
  estimates; closed requests, proposals and human review items. Provider calls
  need explicit human authorization, user-configured endpoints and a budget.
- `noise`: threshold calibration from repeated captures of an unchanged build.
- `init` templates for `saccade.toml`: thresholds, metrics, regions, masks,
  capture requirements and declared changes.
- HDR input (`.exr`, `.hdr`) with HDR-FLIP, numerical G-buffer rules, alpha
  handling and capture metadata sidecars, including Moss engine
  `cost-card.json` provenance keys.
- Performance sidecars (`saccade-perf.v2`) with repeat-noise qualification. The
  `compare` and `identity` JSON results report `performance`, and a
  `performance_rejected` verdict when images pass but timings cannot be compared.
- Agent support: `saccade-result.v2` JSON results with `worst`, sorted
  `failing` entries and typed `next_actions` that carry `cwd`; a local MCP
  server with six tools and no baseline-write tool; Claude Code and Codex
  instruction packs.
- Experimental: `experiment` commands (ablation, sequences, ranking, bisection,
  photosensitivity and accessibility prechecks) and `review eval`.
- GitHub Action with checksum-verified binary install, report upload, job
  summary and one updated pull-request comment.
- `demo`, `doctor --json` capability names, JSON schemas for every artifact,
  and nine reproducible showcases.

### Deprecated

Pre-release command names still work: they run the replacement command and
print one warning. These aliases will be removed after the 1.0 release.

| Deprecated command | Replacement |
| --- | --- |
| `ablate`, `sequence`, `rank`, `bisect`, `safety`, `a11y` | `experiment` followed by the same command |
| `config` | `inspect config` |
| `entries`, `summary` | `inspect ARTIFACT` |
| `explain` | `inspect evidence` |
| `snapshot` | `inspect export` |
| `decision-request`, `decide`, `ask` | `review request`, `review propose`, `review ask` |
| `judge` | `review` |
| `runs` | `view` |
| `unblind` | `view --unblind` |
| `watch` | `compare BASE CAPTURE` once; schedule repeats in the capture producer |

### Removed

Pre-release flags fail with an error that names the replacement.

| Removed flag | Replacement |
| --- | --- |
| `identity --threshold`, `identity --metric` | `compare --threshold`, `compare --metric` |
| `--json=full`, `--json=decision` | `inspect export ARTIFACT --format json --out FILE`; `review request` for closed questions |
| `--entries` | repeat `--entry GLOB` |
| `entries --name`, `entries --offset` | `inspect --entry NAME`, `inspect --cursor TOKEN` |
| `runs --pair-by-position` | `view` with matching relative image names |
| `mcp --watch` | Producer capture scheduling and `mcp --root DIR` |
| `watch --debounce-ms` | Producer capture scheduling and `compare BASE CAPTURE` |
| `--compat` | `--json` and `inspect export --format json` |
| `approve --force` | `approve --dry-run`, then `approve --decisions` |

[Unreleased]: https://github.com/Tavrin/saccade/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/Tavrin/saccade/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/Tavrin/saccade/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Tavrin/saccade/releases/tag/v0.1.0
