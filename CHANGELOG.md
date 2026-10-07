# Changelog

## Unreleased

- Add `timed-text` for plain SRT/WebVTT against frame maps, with image-bound OCR,
  sampled timing/missing/mismatch/extra findings and reused text-legibility
  measurements in `saccade-timed-text.v1`. Missing OCR explicitly skips; video
  extraction stays external. Include generated caption-video and cross-domain
  acceptance fixtures.

## 0.2.7 (2026-10-07)

- Correct the quality-report and geometry v1 schemas to include the existing
  optional report links, preserving unlinked reports. Check generated schema bytes
  in CI with minimal compression/schema, combined assist/evaluation and all core features.

- Add `saccade manifest build|verify|link|classify` and `saccade-manifest.v1` / `saccade-link.v1`:
  one manifest per output directory built on `report_id` and `reports/index.jsonl`, duplicates listed
  once, approval kept separate from last-good, and `link_missing` / `stale_link` failures for moved or
  changed artifacts. Add `saccade export-regions` (worst hotspots as crops with coordinates,
  `saccade-region-export.v1`), `saccade view --open`, and worst-first ordering plus a `w` shortcut in
  the HTML report. See `docs/discovery.md`.
- Decode TrustMark Q payload bits and all four BCH schemas with explicit ECC
  outcomes, pinned local models and reference-encoder tests. Watermark reports
  use versioned successor schemas; missing models remain explicitly unavailable
  and inspection never downloads.
- Add optional `saccade-print` extension and `print` CLI/MCP comparison for ICC-managed CMYK rasters, ΔE2000, TAC, separations, target gamut diagnostics and small four-colour mark candidates.

- Add a task map, exit-code table and threshold-unit notes to the quickstart, and
  five task guides under `docs/guides/` (visual CI, controlled rendering,
  document export, media intake, delivery tuning). `scripts/test-guides.py` runs
  every guide block against a built binary: known-good, known-bad, missing-input
  and unavailable-dependency cases, with the expected exit codes.
- `saccade --help` now lists exit codes 0-4, the task map and threshold units.
- `saccade doctor` adds `command_availability`: which command groups this build
  can run, the feature each needs, and the fix text.
- Unavailable-model errors point at `saccade doctor` instead of an argument hint;
  `sweep`, `imgtune`, `design` and `notify` on a build without `products` report
  the missing feature instead of an unrecognized subcommand.
- Add `saccade score` as an alias of `quality-score`, and `saccade init --template
  producer-strict` (`require_matching_meta` and `require_valid_arms`, nothing
  waived). No command is removed and no default changes.
- State unit and direction in help for numeric flags (`--threshold`, `--ppd`,
  `dedupe --threshold`, `--maximum-outside-flip`, `--limit`, `--top` and others).
- Correct stale documentation: LPIPS, DISTS and MUSIQ run through `quality-score`
  on an operator-supplied reviewed export (none ships), and SVG/PDF comparison
  needs the `documents` feature rather than being deferred.
- Add `saccade mask-metrics`: IoU, Dice, precision/recall and tolerance-based boundary
  F-score between two integer label images, with per-class results, void labels,
  explicit empty/missed/spurious class states and no resampling (`saccade-mask-metrics.v1`).
- Add `saccade boxes export|import|transform`: COCO and YOLO bounding-box interchange
  from a `saccade-boxes.v1` document with explicit pixel/top-left/xywh coordinates,
  counted clipping, and crop and resize re-expression (`saccade-boxes-result.v1`).
- Add `saccade frame-map check` and the `saccade-frame-map.v1` input contract for
  externally extracted frames: gaps, variable frame rate, file integrity and settling
  restated in the map's own timestamps, with an explicit `never_settled` state.
- Add the performance sidecar kit (`examples/perf-kit`, `scripts/gen-perf-kit.py`,
  `docs/perf-kit.md`) with declared `timing ab` outcomes.
- Add feature-gated `tofu` missing-glyph shape triage and `text-legibility`
  per-region variant sampling evidence, with versioned schemas, explicit
  abstention, optional cached OCR and generated multi-script fixtures.
- Add opt-in segmented exact embedding indexes, incremental changed-byte replacement
  and pruning, atomic durable manifest updates, and bounded-memory v2 queries.
  Preserve flat v1 readers and record synthetic scale costs.
- Add budgeted `experiment transition` and `experiment animation` over external
  timestamped captures, with localized error trajectories, popping/convergence,
  steady level differences and motion-aware diagnostics in versioned packets.

- Harden experimental assist pre-spend accounting: shared campaign reservations,
  charged and quarantined usage overruns, request-bound recorded execution receipts,
  nonvacuous qualification, independent control and glyph-localization checks, and
  dispatch-key reflection suppression. Live provider dispatch is disabled until
  verified billing ceilings are available. Add an OpenRouter chat-completions
  adapter and a fixture-only `qualify-wave4.sh --dry-run` with frozen expected output.

## 0.2.6 (2026-10-06)

- The shared report index is written beside each report inside --out; commands no longer create reports/index.jsonl in the working directory.

- Refresh command/schema references and agent packs; allow documentation generation
  without the AVIF codec when dav1d is unavailable, recording the compiled features.

- Fingerprint records accept 16 MiB by default, with map/CLI byte limits, a 64 MiB hard ceiling and file-specific oversize diagnostics.

- Analyze external paired timings with same-session A/A controls, block-bootstrap HL
  verdicts and declared sequential looks; add event-relative tile settling trajectories.
- Show per-arm repeat timing distributions and ranked ablation tables; add native mask
  shortcuts and automatic fingerprint subtree mappings with exclusions.
- Link measurement reports to external capture indexes using the existing semantic
  measurement identity, source refs and JSON/JSONL export. Strict linked reports use
  versioned successor schemas; legacy schemas and authority bindings stay readable.
- Expose stable public mask-spec parsers in `saccade-core`; document block-bootstrap
  validity and declared sequential stopping with a repeated-peeking null acceptance gate.

### Model configuration and distribution

- One model and runtime configuration for the CLI, MCP server, Python and Rust library:
  `SACCADE_MODELS_DIR`, `SACCADE_MODELS_REGISTRY`, `SACCADE_MODELS_RUNTIME_LIBRARY`,
  `SACCADE_MODELS_EMBEDDING_CONTRACT`, or the `[models]` table of
  `~/.config/saccade/config.toml`. `saccade models config --json` and Python
  `saccade.model_config()` show each resolved value and its source
  (`saccade-model-config.v1`). `SACCADE_MODEL_CACHE` is still read. See `docs/models.md`.
- `saccade models pull` is the one provisioning verb: it also pulls `ocr` and
  `embedding` contracts. MCP requests can no longer choose a cache, registry or runtime
  path (a request may only restate the configured one; anything else is refused with
  `model_location_not_request_controlled`) and never download.
- Release bundles: `default` (unchanged legacy assets, also published as
  `saccade-default-<target>`), `media` (C2PA, PDF/SVG, imgtune, OCR) and `full`, each with an
  inventory, smoke-test result and `SHA256SUMS-bundles`. See `docs/install-matrix.md`.
- The Playwright package is prepared for npm (`npm pack --dry-run`); publishing stays a
  maintainer step (`docs/releasing.md`).

### Deprecations and the 0.3.0 plan

- Deprecated, still working, with a stderr notice (Python `DeprecationWarning`), not removed
  before 0.4.0: `--cache`, `--model-cache`, `--model-dir`, `--api-model-dir`, `--registry`,
  `--model-registry`, `--runtime-library`, `--library`, embedding `--model`, `--ocr-contract`
  with a shared registry, `--download-model`, `--allow-download`, and Python `model_dir=`,
  `registry=`, `allow_download=`. Use the configuration above and `saccade models pull`.
- Removed in 0.3.0 (already announced in 0.2.5): automatic gpu-clock v2 telemetry
  ingestion (the schema ID named in the 0.2.5 deprecation below); use `--gpu-clock-map FILE`.

## 0.2.5

- Add map-level and per-field `absent = "missing"|"value"` policies for optional
  fingerprint fields, retaining the strict default and reporting absent states.

- Add TOML/JSON GPU telemetry maps via `--gpu-clock-map FILE`, config
  `gpu_clock_map`, and MCP, sharing fingerprint-map dotted paths and array indices.
- Deprecate automatic `moss.gpu-clock.v2` telemetry ingestion; it remains accepted
  with a map migration warning through 0.2.x and will be removed in 0.3.0.
- Keep result v2 `mode` as the measurement operation; its schema and values remain compatible.
- Document `cost-card.json` as Saccade’s provenance sidecar and neutralize producer examples.
- Guard against origin-project names and vocabulary with exact-line exceptions
  for the acknowledgement and temporary protocol compatibility.

## 0.2.4

- Match vary and ignore tokens against mapped destinations or source paths,
  reporting the token and matched name for each covered field.
- Support numeric array indices in dotted fingerprint source paths, with clear
  rejection of source wildcards; accept comma-separated `arms check --vary` and `--ignore` values.
- Add opt-in `compare = "mapped_only"` fingerprint maps for mixed setup and
  outcome records, with `--compare mapped-only|all` and MCP overrides.
- Add explicit outcome globs in either mode and bounded key summaries with
  exact totals. The default `all` mode preserves existing comparisons.

## 0.2.3

- Embed schema discovery and performance-sidecar validation for installed producers.
- Distinguish null fingerprint values from absent fields; record ignored states,
  exact matched-unreached exceptions, and ordered run-record selection.
- Attribute shifts per ID, flag whole-frame scope, accept generic screen-space dumps,
  export float32 NumPy/EXR maps, and estimate noise envelopes from same-arm repeats.
- Expose Python version and NumPy maps; inventory optional dependencies in doctor
  with early missing-dependency hints.

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.2

### Added

- Python package published on PyPI as `saccade-vision` (import name stays `saccade`); wheels for Linux, macOS and Windows.
- Producer arm fingerprints, `--require-valid-arms`, `saccade arms check`
  and TOML/JSON fingerprint-map configuration. Per-field `derives` declarations
  cover keys computed from varied fields and report both values separately;
  undeclared differences still refuse comparison. See [arm validity](docs/arm-validity.md).

## 0.2.1

- Removed internal development notes and local paths; no functional change.

## [0.2.0] - 2026-10-06

### Added

- Add opt-in rendering evidence: required effect occupancy, intended experiment
  variables, spatial and layer scope, automatic evidence regions, fixed-camera
  temporal tiles, noisy offline references, preregistered blind trials and warmup
  convergence qualification. Capability discovery lists the new workflows;
  media records can expose descriptive spatial tiles through `quality_tile_size`.
  Structural classes and human preferences do not qualify performance or approve baselines.
- Add versioned media records with per-section status/provenance, reusable optional
  CPU sessions, deterministic saliency and declared crop fitness.
- Add the abi3 Python package, exact image/text index APIs, external ffmpeg
  keyframe sampling and saved-fingerprint usage matching.
- Add a bounded local HTTP API and unprivileged container recipe; configured
  compatible/Azure provider mappings are verified with fixtures only.
- Add pinned official SigLIP 2 checkpoint/tokenizer provenance and a reproducible
  CPU export script; text retrieval requires that joint model and remains uncalibrated.
- Add experimental `review explain`, `review audit-mask`, `review check-ui` and
  `review assist batch submit|status|collect`; optional Jev routing remains unqualified.
- Add the local Playwright matcher, `sweep plan|compare`, `imgtune audit|search`,
  `design pull|compare`, `notify` and verified `--baseline last-good` lookup.
- Add explicit registration/resampling, `hash`, `dedupe`, `similar`,
  `index build|query|export-inputs|calibrate`, `text`, `assess`, `inspect-image`,
  `capabilities` and comparison question routing.
- Add optional SVG/PDF rendering, offline C2PA validation and pinned Rust OCR.
  Model, OCR readability, forensic specificity and broad renderer qualification
  keep the limits recorded in their evidence and documentation.
- Add vision commands and explicit face/crop/watermark observations in image
  reports, fixture-only hosted mappings and advisory check-ui localization.
  Shared model contracts retain legacy readers. Browser/sweep masks neutralize
  excluded pixels before filtering; core score-exclusion defaults remain.
  Model export/parity and generated OCR review keep separate qualification gates.

### Changed

- Raise the MSRV to Rust 1.89 and upgrade Butteraugli to 0.9.3; remove the
  vendored compatibility patch.
- Replace the local ocRs adapter with pinned PP-OCRv5 Latin on CPU ONNX Runtime
  1.22. Add explicitly selected Mistral document OCR with fixture-tested spend
  and egress controls. Strict scores, expectations and thresholds stay unchanged;
  a separate declared typographic-equivalence view handles curly apostrophes,
  narrow/nonbreaking/thin spaces and en/em dashes.
- Update Cargo, Python, plugin and MCP Registry release metadata to 0.2.0.

### Known limitations

- PP-OCRv5 can misread œ in large serif text and omit dashes with some sans
  fonts. Missing spaces before € remain errors. See the
  [strict and typographic scores](docs/ocr-contract-results-2026-10-06.md).
  Live Mistral API and billing behavior remains unqualified.
- Constructed AI assist qualification could not start: the release has no frozen
  corpus with an observed immutable Gemini revision or exact-source gate receipt.
  All assist features remain experimental and unqualified; no provider spend occurred.
- Image/text retrieval calibration remains unqualified. GI effect occupancy is a
  supplied proxy, not proof of physical illumination or causality.
- LPIPS, DISTS and MUSIQ exports remain deferred. TrustMark neural inference
  does not establish payload decoding.

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
  handling and capture metadata sidecars, including rendering-engine
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

[0.2.0]: https://github.com/Tavrin/saccade/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/Tavrin/saccade/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/Tavrin/saccade/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Tavrin/saccade/releases/tag/v0.1.0
