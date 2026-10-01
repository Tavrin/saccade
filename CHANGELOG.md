# Changelog

This project follows [Semantic Versioning](https://semver.org/). Entries are
newest first.

## Unreleased

- Engine-neutral run performance sidecars with strict validation, exact-ID
  attribution/counter diffs, nested scopes and stacked frame composition.
- Repeat performance noise ranges, configurable beyond-noise gates, CLI/MCP
  ablation tables and combined image/performance verdicts across reports,
  run overviews and unblinded viewer sessions.

- Replace the C++ FLIP bindings with the pinned pure-Rust flip-rs backend.
  HDR-FLIP now evaluates float exposures using NVIDIA's reference algorithm;
  LDR keeps sRGB input and saccade's nearest-rank statistics. Rust 1.88 is
  required, HDR exposure counts start at two, and automatic all-black HDR
  baselines require explicit endpoints. CI fetches flip-rs with a deploy key.

- Dashboard deep links: single-run and single-image pages, repeated `run=`
  parameters, absolute-path `/open` redirects and `/api/roots`. Explicit external
  symlink targets and bounded storage timeouts support captures on NAS mounts.

- Portable report and decisions provenance with SHA-256 binding, absolute-path opt-in,
  and directory-derived `approve --report` / `--decisions` forms.
- Filtered, paginated entry inspection with MCP list/get tools, configuration
  explanation and four commented init templates.
- File-pair comparison, repeatable name filters, unchanged-build noise calibration
  with schema/TOML suggestions, JUnit export, bundled demo and actionable error hints.

- Release archives include README, MIT/Apache licences and the NVIDIA FLIP
  third-party notice, with per-asset SHA-256 files and a combined `SHA256SUMS`.
- Showcase reports are generated outside the source tree. `build.py --out`
  creates an offline gallery; a Pages workflow builds and deploys it at
  `/saccade/showcase/`. Committed media remains available to the README.
- Numerical-buffer heatmaps use per-image p99 error floored by the buffer
  threshold, recorded as `buffer.heatmap_max`; small depth errors are visible.
- Sidecar timings pair identical key names only; one-sided timing keys are
  reported as `perf_not_comparable` and labelled in text and Markdown.
- Judge CLI and MCP tools are labelled experimental, with a calibration reminder.
- Expanded Claude Code and Codex instruction packs, draft MCP Registry metadata
  (not yet published), install/checksum instructions and a distribution roadmap.

- Renamed from the working title flipdiff before first release.

- Per-image output folders now end in `.d`; CLI outputs next to capture
  metadata warn on stderr (`--allow-out-near-captures` silences the warning).

- Judge mode: typed bounded questions, deterministic text evidence and blind
  both-order strips; weighted panels (Jev, Gemini, OpenCode, compatible HTTP and
  local humans), canary down-weighting, calibration/ECE/human agreement,
  invariance self-tests and Bradley–Terry ranking intervals. Adds vote pages,
  CLI/MCP tools, schemas and a provider audit trail; retains decision proposals
  and deterministic-failure gates, with file-only secrets and isolated free
  OpenCode calls.

- Binary-search image divergence with existing runs or CLI user-command captures;
  skipped probes, observed reversals, per-probe reports and `saccade-bisect.v1`.
- Debounced capture watch with notify/polling fallback, JSONL, MCP status and logs.
- Persistent local human inbox, secure private serve discovery, `ask --wait`,
  browser answers and MCP ask/get tools, with schemas and agent instruction packs.

- Numbered image sequences: per-frame FLIP, added temporal instability,
  `saccade-sequence.v1` and a server-rendered SVG curve in the report.
- Numerical G-buffer rules (`[[buffer]]`): depth including single-channel EXR,
  normal angles, motion end-point errors, exact mask/id samples and heatmaps.
- Candidate ranking by mean/p95/p99/max FLIP: `saccade-rank.v1`, competition
  ties, common-image overall means, Markdown tables and per-candidate reports.
- MCP `saccade_sequence` and `saccade_rank`, lean JSON outputs and schemas.
- Dispatch-only baseline-update PR flow in the Action, explicit pruning,
  fork/write guards and `example-update-baselines.yml`.

- Run overview: comparing 2 or more runs in `serve` opens `/runs` first (per-run
  summary with a "no visible effect" flag, run-level config differences, a
  matrix tinted by FLIP, a contact sheet with one shared swipe slider). Also
  `GET /api/runs`, `saccade runs` (static page or `saccade-runs.v1` JSON) and
  the MCP tool `saccade_compare_runs`. Mismatched file names offer pairing by
  position or by hand. `serve` takes several roots and
  `--follow-symlinks-within-roots`; run rows expand to single images that can
  be compared; the viewer labels sets "only in <label>", sorts them last and
  can hide them, and names the missing side in the empty swipe state.

- Agent-addressable pages: the report, `view` and `serve` sessions keep their
  view in the URL hash, offer "Copy link to this view" and expose
  `window.saccade` (`get`, `set`, `sets`/`entries`, `next`/`prev`, `snapshot`,
  `on`). `saccade snapshot` and the MCP tool `saccade_snapshot` render a view
  state to PNG with no browser. Fixed: the viewer's arrow keys outside swipe
  threw (a local variable shadowed `step`).
- Bounded decisions: `decision-request` (also `compare --json=decision` and
  MCP) asks fixed `accept`, `triage`, `cause`, `ask_human` and `mask_suggest`
  questions; `decide` records answers as proposals, shown as a chip with
  `y`/`n` confirm keys; `[decisions]` confidence gate; deterministic failures
  are never model-answerable. Adapters for Jev, the OpenAI Decisions API
  (unverified) and any JSON-schema chat model in `examples/adapters/`.
- Diagnostics: every compared pair gets `diagnostics` (class, plain-English
  description, global tone fit with its explained fraction, sub-pixel shift by
  phase correlation, signed difference PNG, non-finite mask, paired sidecar
  timings). Class and description appear in the text table, Markdown, the lean
  result, MCP and `explain.md`. Config `[diagnostics]`.

- Agent surface: `view --blind` embeds no reference, per-set order, seed or
  FLIP data and names panes neutrally per set (`--key-out` for the key);
  `explain --blind` needs `--key-out` outside the pack and omits the report
  path and labels. With `--json`, every error is a `saccade-error.v1` on
  stdout; `compare --json` and `identity --json` print a lean
  `saccade-result.v1` (`--json=full` for the report); every schema id has a
  file in `schemas/`. `saccade mcp --root DIR` confines paths, the tools gain
  `outputSchema`, annotations, more arguments and image content blocks.
  `explain`: strips at most 1536 px wide, `hotspot_min_share` (default 0.01),
  hot-pixel and box areas labelled apart.
- Safety: `compare`, `identity`, `view` and `explain` refuse (exit 2) an
  `--out` that is not empty and not a previous saccade output, and `--out`
  inside an input directory. `approve` verifies the directories and the SHA-256
  of the files recorded in the report or decisions file (`--force` overrides),
  and refuses a blind decisions file.
- CI correctness: a run that compared nothing exits 1 (`--allow-empty` opts
  out); NaN or infinite HDR captures are errors (`fail_on_nonfinite`);
  all-black and all-white images are warned about; `hotspot_fail` and the
  `p99` metric catch local defects behind a low mean.
- Report additions (all optional on read): `baseline_dir`, `capture_dir`,
  `baseline_sha256`, `capture_sha256`, `baseline_properties`, `warnings`,
  `meta_ignored_diff`, `nan_count`, `inf_count`, `negative_count`. Decisions
  files gain `dirs`, `chosen_dir` and `sha256`.
- Default meta ignore globs narrowed: `*time*` is gone.
- Pairs are compared in parallel; `unblind` writes `"blind": false`.
- Markdown numbers use 4 significant digits without trimming; the text table
  moves status text to `↳` lines.

## 0.1.0

First release.

- `saccade compare`: compares a directory of baseline images with a directory
  of captures using NVIDIA FLIP. Writes a JSON report
  (`saccade-report.v1.json`), a self-contained HTML report (table, side by
  side, swipe, flicker, heatmap, zoom) and a text table. Statuses `pass`,
  `fail`, `new`, `missing` and `error`. Exit code 0 (no regression), 1
  (regression) or 2 (usage or IO error). Metrics `mean`, `p95` and `max`,
  threshold and metric per image through `saccade.toml` overrides.
- `saccade identity`: identity proof for optimizations. Metric `max`, threshold
  0, and `bit_identical` per image.
- `saccade approve`: copy captures over baselines, by name, from a report
  (`--all-failing`, `--include-errors`, `--prune-missing`) or from a decisions
  file (`--decisions`).
- `saccade summary`: Markdown or text summary of a report.
- `saccade view`: self-contained review viewer for 2 to 6 directories (layouts,
  synchronised zoom, pixel inspector, exposure and channel controls, regions of
  interest), with accept, reject and needs-work decisions exported as
  `saccade-decisions.v1.json`.
- Blind mode (`view --blind`) with `blind-key.json` and `saccade unblind`.
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
