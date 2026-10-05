# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0]

### Added

- Shared analysis evidence contracts with explicit availability and provenance.
  Missing historical evidence stays unknown; provenance alone does not prove
  reproducibility, capture validity or approval.
- Exclusion audits for capture selection, masks, ignored error, decoder losses
  and threshold headroom, with an unmasked counterfactual. Configured verdicts
  remain authoritative; undeclared, unsupplied captures remain unknown.
- Performance regression-onset candidates from hash-verified history, partitioned
  by comparison identity and qualified timing. Synthetic tests establish detector
  mechanics, not field false-alert calibration; confirmation needs fresh repeats.
- Optional static OBJ/glTF/GLB geometry measurements and ordered mesh identity.
  Sampled distances are not certified continuous bounds. Identity covers ordered
  world vertices, oriented triangles and declared units, excluding appearance
  attributes, materials, textures and renderer acceptance.
- Motion diagnostics beside raw FLIP, with qualified global translation and
  per-hotspot evidence. Constructed opaque SDR translations are the tested scope;
  HDR, transparency, independent object motion, dense DIS flow and field TAA
  qualification remain unsupported or unqualified. Raw FLIP decides the verdict.
- Snapshot variation and cumulative drift suggestions using declared run and
  environment identities. Normal-variation advice requires ten unchanged-build
  runs; producer declarations do not prove independence. Drift remains a candidate,
  needs fresh repeats and neither attributes commits nor edits policy. Live
  browser nuisance-alert and capture-fix evaluation remains unrun.
- Compression-quality sweeps over externally encoded opaque RGB8 sRGB outputs,
  with staged SSIMULACRA2 and Butteraugli evidence and smallest-file selection
  under the frozen SSIMULACRA2 policy. Both metrics are qualified against pinned
  official libjxl v0.12.0 and Cloudinary v2.1 references on a small synthetic
  fixture set. Arbitrary-image visibility and blind human/pipeline evaluation
  remain unqualified; no encoder or live service adapter is invoked. Alpha, HDR,
  embedded EXIF/ICC and dimension changes are rejected.
- Capture inventory with stable case IDs, capture hashes and complete accounting
  of expected cases, including missing, unusable, skipped, stale, quarantined and
  duplicate attempts. Playwright ingestion requires explicit expected/actual
  attachments; legacy attachment-only manifests cannot establish suite completeness.
  Live Playwright/browser qualification remains unrun.
- Localized-change checks with frozen pixel boxes, inclusion masks or
  reference-bound DOM geometry. Full-frame FLIP and native complement samples
  measure intended changes and collateral without configuration exclusions.
  SDR spatial evidence does not establish semantic edit success; HDR/float inputs
  are unsupported and live selector, scroll, device-scale and reflow checks remain
  unvalidated.
- Grounded numerical explanations with source hashes, JSON pointers and paginated
  MCP citations. Typed claims must match one compatible fact and region exactly.
  Semantic and causal claims abstain; semantic/OCR and independent human support
  benchmarks remain unrun. Numerical consistency does not prove physical correctness.
- Frozen-region mask import retaining the phrase, reference and source-mask
  hashes, plus optional checkpoint cache and ONNX runtime-loading plumbing.
  ONNX inference is not included. Checkpoint provenance and graph loading do not
  qualify export parity, segmentation, ambiguity handling, determinism or latency;
  weights and runtime binaries are not shipped and ordinary measurement does not
  download them. Imported phrase selection still needs human review.
- Optional RenderDoc 1.34 Vulkan extraction worker and native-payload divergence
  localization. Synthetic alignment/payload fixtures and unavailable-capability
  handling are tested; live Vulkan replay is unvalidated. First observed divergence
  is a candidate, not root cause or final-output relevance. Clear destinations
  abstain; coverage is bounded to observed resources and one texture mip/layer/
  sample. D3D12/GL and broader resource coverage remain unsupported.

### Fixed

- Retain complete history witnesses, verify stored measurement objects, preserve
  capture ordering and detect repeated or recently anchored drift.
- Preserve Playwright snapshot identity and suite accounting for missing or
  corrupt attachments; reject contradictory IDs and duplicate source pairings.
- Bind inventory and grounded facts to the same retained source bytes, expose
  exclusion scope, validate frozen-region provenance, and diagnose unknown nested
  fields and newer schemas rather than silently accepting them.
- Supervise RenderDoc work with terminating deadlines and bound regular-file
  payload reads; reject FIFOs, missing formats and incomplete resource identities.
- Reject malformed geometry before expansion and periodic motion aliases; retain
  decoder-loss evidence and complete sequence selection scope.
- Include optional geometry dependencies in all-features notices and permit the
  pinned semantic-region manifest in packages while rejecting model weights.
  New dependencies retain the declared Rust 1.88 floor; execution on Rust 1.88
  has not been qualified.

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

[Unreleased]: https://github.com/Tavrin/saccade/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Tavrin/saccade/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/Tavrin/saccade/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/Tavrin/saccade/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Tavrin/saccade/releases/tag/v0.1.0
