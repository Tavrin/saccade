# Contracts

Measured pair reports remain `saccade-report.v1`. Canonical cases, requests,
proposals, human decisions and receipts use `saccade-evidence.v1`.
CLI and MCP use bounded `saccade-result.v2`. `identity --json` keeps the
`saccade-result.v1` discriminator for existing integrations (the Moss game
engine reads it); it retains `schema`,
`mode`, `verdict`, `totals.{pass,fail,error,missing,new,total}` and
`failing[].error`. Read the complete report to establish exact equality; a
lean pass count alone is insufficient. See the [recorded decision](design-decisions/r5.md).

`saccade-ablate.v1` additively records `base_repeats`,
`excluded_base_repeats`, `repeat_qualification`, `repeat_reasons`, and each
arm's accepted `repeats` and `excluded_repeats`. Historical readers can omit
these fields. An excluded run contributes neither image nor timing evidence.
The ablation model also records `base_stability`, arm `repeat_stability`
(`image_hashes` and `max_flip_by_image`), `validity_findings`, and
`next_actions`. The bounded result sets `validity: invalid` when an arm's
repeat variation exceeds its base's for an image.

`saccade-report.v1` additively records `pass_with_local_change` on entries
and `hotspot_local_max` / `hotspot_local_min_pixels` in effective config.
`status: pass` retains its prior meaning; consumers must check the new flag
before calling a passing metric uneventful. The bounded result's `data`
counts local changes and lists at most three, with an inspect action.
`zero_flip_native_difference` is an additive diagnostic class for a FLIP-zero
pair whose native decoded samples differ.

`saccade-perf-diff.v1` additively records `gpu_clock_before` and
`gpu_clock_after` when adjacent `gpu_clock.json` sidecars exist. The
engine-neutral input schema is `saccade-gpu-clock.v1`; Moss
`moss.gpu-clock.v2` is adapted on read. Clock qualification failures and
missing clocks and cross-arm differences append named `qualification_reasons` and set
`comparability: rejected`. The performance-noise path applies the same checks
across repeats.

## Result v2 fields for compare and identity

The [result v2 schema](../crates/saccade-core/schemas/saccade-result.v2.schema.json) lists every
field. Agents should read these first:

- `verdict` takes a third value, `performance_rejected`: image thresholds pass
  but performance comparability is rejected. The process exit code still
  reports the image result (0).
- `performance` (`verdict`, `comparability`, `repeat_qualification`,
  `summary`) is present when performance evidence was compared. `overall` is
  `performance_rejected` in the case above and absent otherwise.
- `worst` is `{entry, metric, value, threshold}` for the failing entry with the
  highest value/threshold ratio, or `null`. `failing` uses the same order;
  entries without a value follow measured failures, then sort by name.
- `next_actions[].cwd` is the absolute directory the command ran in. Relative
  paths in `cli_argv` resolve against it. Other paths stay relative unless
  `--record-absolute-paths` is set.
- `validity_missing` (`keys`, `source`) names missing provenance fields and
  the sidecar that can supply them.

`inspect REPORT --entry NAME` returns that entry's `status`, `metric`,
`value`, `threshold`, up to three `hotspots` and an `explanation`.
`inspect REPORT --validity-reasons` pages through capture-validity reasons with
`index` and `reason`; it rejects entry and status filters. `review REPORT
--out DIR` writes `DIR/requests.json` and `DIR/preview.json`; each payload
carries `request_bytes`, `estimated_input_tokens`, `estimated_output_tokens`,
`estimated_cost_usd` and `cost_reason`. See [agents](agents.md) for examples.

## Identity and bundles

Semantic identity uses sorted canonical object keys and round-trip numbers.
Input content, sidecars, effective config, scope, intent, question/encoder,
transforms and actual observation/fallback identities are bound. Timestamps
and relocatable path spellings remain provenance rather than semantic identity.
Changed content invalidates cached requests and reviewed decisions.
Missing data stays explicitly unavailable.

Report bundles emit `.saccade-run` at the root. Archive readers recognize
`saccade-report.vN.json`, `saccade-view.vN.json`, historical
`flipdiff-report.vN.json`, `flipdiff-view.vN.json`, `explain.json` and
`.saccade-run`. Keep reports outside captures and retain historical readers.

Serve preserves `/open?abs=PATH`, repeated `abs` and optional `ref` for 2–6
captures, logical archive aliases, `/api/roots` with name/path entries, and
`serve ROOT...`. Keep `--follow-symlinks-within-roots`, repeatable
`--symlink-target DIR`, `--meta-name cost-card.json`, bounded storage deadlines
and read-only browsing. Serve and MCP share the root resolver.

## Build identity and behavior capabilities

`saccade doctor --json` reports the package `version`, a `build` object, and a
sorted `capabilities` array. `build.git_commit` (full hash),
`build.git_commit_short`, and `build.git_dirty` describe the checkout at build
time. They are `null` for a published crate tarball or when Git is unavailable.
`build.profile` and `build.rustc_version` identify the Cargo profile and compiler.
`saccade --version` adds `+g<short commit>` and, for a dirty checkout, `.dirty`
to the package version. The suffix is absent without a Git identity.

Scripts should gate on capability names, rather than the package version or
the presence of a CLI command. Names are append-only; an existing name will
not be renamed or removed without a deprecation period.

| Capability | Guarantee |
| --- | --- |
| `mask-mode-v1` | Explicit exclude/neutralize mask modes with recorded original-error audits. |
| `compat-aliases` | Legacy command aliases remain accepted with a replacement warning. |
| `removed-flag-errors` | Removed flags fail with a targeted replacement or removal error. |
| `version-skew-errors` | Newer persisted report versions fail with a distinct version skew error. |
| `provenance-warnings` | Missing capture provenance is reported as a warning. |
| `repeat-detection` | Repeated capture evidence is detected and reported. |
| `perf-v2` | Performance evidence schema `saccade-perf.v2` is supported. |
| `identity-json-v1` | Identity can emit its versioned JSON result. |
| `prechecks` | The `prechecks` feature adds safety and accessibility prechecks. |
| `mcp` | The `mcp` feature adds the local MCP server. |
| `review` | The `ai` feature adds provider-backed review execution. Local review preview remains available without it. |

The last three names appear only when the corresponding feature is compiled in.

## Generated schema index

<!-- schema-index:start -->
- [saccade-a11y.v1.schema.json](../crates/saccade-core/schemas/saccade-a11y.v1.schema.json) — a11y PRE-CHECK only; not certification or formal compliance
- [saccade-ablate.v1.schema.json](../crates/saccade-core/schemas/saccade-ablate.v1.schema.json) — Ablation
- [saccade-api-analyze.v1.schema.json](../crates/saccade-core/schemas/saccade-api-analyze.v1.schema.json) — Historical reader contract
- [saccade-api-compare.v1.schema.json](../crates/saccade-core/schemas/saccade-api-compare.v1.schema.json) — Historical reader contract
- [saccade-api-health.v1.schema.json](../crates/saccade-core/schemas/saccade-api-health.v1.schema.json) — Historical reader contract
- [saccade-api-search.v1.schema.json](../crates/saccade-core/schemas/saccade-api-search.v1.schema.json) — Historical reader contract
- [saccade-approve.v1.schema.json](../crates/saccade-core/schemas/saccade-approve.v1.schema.json) — saccade-approve.v1
- [saccade-ask-result.v1.schema.json](../crates/saccade-core/schemas/saccade-ask-result.v1.schema.json) — AskResult
- [saccade-assess.v1.schema.json](../crates/saccade-core/schemas/saccade-assess.v1.schema.json) — Historical reader contract
- [saccade-asset-view-report.v1.schema.json](../crates/saccade-core/schemas/saccade-asset-view-report.v1.schema.json) — Report
- [saccade-asset-views.v1.schema.json](../crates/saccade-core/schemas/saccade-asset-views.v1.schema.json) — Manifest
- [saccade-assist-batch-plan.v1.schema.json](../crates/saccade-core/schemas/saccade-assist-batch-plan.v1.schema.json) — FrozenPlan
- [saccade-assist-batch.v1.schema.json](../crates/saccade-core/schemas/saccade-assist-batch.v1.schema.json) — Job
- [saccade-assist-gates.v1.schema.json](../crates/saccade-core/schemas/saccade-assist-gates.v1.schema.json) — saccade-assist-gates.v1
- [saccade-assist-mask-audit.v1.schema.json](../crates/saccade-core/schemas/saccade-assist-mask-audit.v1.schema.json) — saccade-assist-mask-audit.v1
- [saccade-assist-masks.v1.schema.json](../crates/saccade-core/schemas/saccade-assist-masks.v1.schema.json) — saccade-assist-masks.v1
- [saccade-assist-observations.v1.schema.json](../crates/saccade-core/schemas/saccade-assist-observations.v1.schema.json) — saccade-assist-observations.v1
- [saccade-assist-requests.v1.schema.json](../crates/saccade-core/schemas/saccade-assist-requests.v1.schema.json) — saccade-assist-requests.v1
- [saccade-assist-vision-provider.v1.schema.json](../crates/saccade-core/schemas/saccade-assist-vision-provider.v1.schema.json) — Historical reader contract
- [saccade-assist.v1.schema.json](../crates/saccade-core/schemas/saccade-assist.v1.schema.json) — Envelope
- [saccade-bisect.v1.schema.json](../crates/saccade-core/schemas/saccade-bisect.v1.schema.json) — BisectResult
- [saccade-blind-key.v1.schema.json](../crates/saccade-core/schemas/saccade-blind-key.v1.schema.json) — BlindKey
- [saccade-brand-review.v1.schema.json](../crates/saccade-core/schemas/saccade-brand-review.v1.schema.json) — Report
- [saccade-brand-source.v1.schema.json](../crates/saccade-core/schemas/saccade-brand-source.v1.schema.json) — Evidence
- [saccade-calibration.v1.schema.json](../crates/saccade-core/schemas/saccade-calibration.v1.schema.json) — Historical reader contract
- [saccade-capabilities.v1.schema.json](../crates/saccade-core/schemas/saccade-capabilities.v1.schema.json) — Historical reader contract
- [saccade-capture-layers.v1.schema.json](../crates/saccade-core/schemas/saccade-capture-layers.v1.schema.json) — Manifest
- [saccade-constructed-oracle.v1.schema.json](../crates/saccade-core/schemas/saccade-constructed-oracle.v1.schema.json) — saccade-constructed-oracle.v1
- [saccade-constructed-truth.v1.schema.json](../crates/saccade-core/schemas/saccade-constructed-truth.v1.schema.json) — saccade-constructed-truth.v1
- [saccade-crop-check.v1.schema.json](../crates/saccade-core/schemas/saccade-crop-check.v1.schema.json) — CropReport
- [saccade-decide-result.v1.schema.json](../crates/saccade-core/schemas/saccade-decide-result.v1.schema.json) — saccade-decide-result.v1
- [saccade-decision-request.v1.schema.json](../crates/saccade-core/schemas/saccade-decision-request.v1.schema.json) — saccade-decision-request.v1
- [saccade-decisions.v1.schema.json](../crates/saccade-core/schemas/saccade-decisions.v1.schema.json) — Decisions
- [saccade-dedupe.v1.schema.json](../crates/saccade-core/schemas/saccade-dedupe.v1.schema.json) — Historical reader contract
- [saccade-design-captures.v1.schema.json](../crates/saccade-core/schemas/saccade-design-captures.v1.schema.json) — saccade-design-captures.v1
- [saccade-design-map.v1.schema.json](../crates/saccade-core/schemas/saccade-design-map.v1.schema.json) — saccade-design-map.v1
- [saccade-design-pull.v1.schema.json](../crates/saccade-core/schemas/saccade-design-pull.v1.schema.json) — saccade-design-pull.v1
- [saccade-design-report.v1.schema.json](../crates/saccade-core/schemas/saccade-design-report.v1.schema.json) — saccade-design-report.v1
- [saccade-documents.v1.schema.json](../crates/saccade-core/schemas/saccade-documents.v1.schema.json) — Historical reader contract
- [saccade-dom-regions.v1.schema.json](../crates/saccade-core/schemas/saccade-dom-regions.v1.schema.json) — DomMetadata
- [saccade-embedding-corpus.v1.schema.json](../crates/saccade-core/schemas/saccade-embedding-corpus.v1.schema.json) — Historical reader contract
- [saccade-embedding-export-inputs.v1.schema.json](../crates/saccade-core/schemas/saccade-embedding-export-inputs.v1.schema.json) — Historical reader contract
- [saccade-embedding-export-receipt.v1.schema.json](../crates/saccade-core/schemas/saccade-embedding-export-receipt.v1.schema.json) — Historical reader contract
- [saccade-embedding-index.v1.schema.json](../crates/saccade-core/schemas/saccade-embedding-index.v1.schema.json) — Historical reader contract
- [saccade-embedding-model.v1.schema.json](../crates/saccade-core/schemas/saccade-embedding-model.v1.schema.json) — Historical reader contract
- [saccade-embedding-qualification.v1.schema.json](../crates/saccade-core/schemas/saccade-embedding-qualification.v1.schema.json) — Historical reader contract
- [saccade-embedding-query.v1.schema.json](../crates/saccade-core/schemas/saccade-embedding-query.v1.schema.json) — Historical reader contract
- [saccade-entries.v1.schema.json](../crates/saccade-core/schemas/saccade-entries.v1.schema.json) — EntriesPage
- [saccade-error.v1.schema.json](../crates/saccade-core/schemas/saccade-error.v1.schema.json) — saccade-error.v1
- [saccade-evidence.v1.schema.json](../crates/saccade-core/schemas/saccade-evidence.v1.schema.json) — Document
- [saccade-explain-blind-key.v1.schema.json](../crates/saccade-core/schemas/saccade-explain-blind-key.v1.schema.json) — ExplainBlindKey
- [saccade-explain-result.v1.schema.json](../crates/saccade-core/schemas/saccade-explain-result.v1.schema.json) — saccade-explain-result.v1
- [saccade-explain.v1.schema.json](../crates/saccade-core/schemas/saccade-explain.v1.schema.json) — ExplainPack
- [saccade-faces.v1.schema.json](../crates/saccade-core/schemas/saccade-faces.v1.schema.json) — FaceReport
- [saccade-frozen-region.v1.schema.json](../crates/saccade-core/schemas/saccade-frozen-region.v1.schema.json) — FrozenRegion
- [saccade-general-result.v1.schema.json](../crates/saccade-core/schemas/saccade-general-result.v1.schema.json) — saccade-general-result.v1
- [saccade-geometry.v1.schema.json](../crates/saccade-core/schemas/saccade-geometry.v1.schema.json) — Document
- [saccade-gpu-clock.v1.schema.json](../crates/saccade-core/schemas/saccade-gpu-clock.v1.schema.json) — GpuClock
- [saccade-grounded.v1.schema.json](../crates/saccade-core/schemas/saccade-grounded.v1.schema.json) — Explanation
- [saccade-hash.v1.schema.json](../crates/saccade-core/schemas/saccade-hash.v1.schema.json) — Historical reader contract
- [saccade-imgtune-audit.v1.schema.json](../crates/saccade-core/schemas/saccade-imgtune-audit.v1.schema.json) — saccade-imgtune-audit.v1
- [saccade-imgtune-search.v1.schema.json](../crates/saccade-core/schemas/saccade-imgtune-search.v1.schema.json) — saccade-imgtune-search.v1
- [saccade-imgtune.v1.schema.json](../crates/saccade-core/schemas/saccade-imgtune.v1.schema.json) — saccade-imgtune.v1
- [saccade-inbox-item.v1.schema.json](../crates/saccade-core/schemas/saccade-inbox-item.v1.schema.json) — Item
- [saccade-inspect-image.v1.schema.json](../crates/saccade-core/schemas/saccade-inspect-image.v1.schema.json) — Historical reader contract
- [saccade-inventory-report.v1.schema.json](../crates/saccade-core/schemas/saccade-inventory-report.v1.schema.json) — Inventory
- [saccade-inventory.v1.schema.json](../crates/saccade-core/schemas/saccade-inventory.v1.schema.json) — Manifest
- [saccade-judge-bench.v1.schema.json](../crates/saccade-core/schemas/saccade-judge-bench.v1.schema.json) — Historical reader contract
- [saccade-judge-selftest.v1.schema.json](../crates/saccade-core/schemas/saccade-judge-selftest.v1.schema.json) — Historical reader contract
- [saccade-judge-vote-api.v1.schema.json](../crates/saccade-core/schemas/saccade-judge-vote-api.v1.schema.json) — Historical reader contract
- [saccade-judge-votes.v1.schema.json](../crates/saccade-core/schemas/saccade-judge-votes.v1.schema.json) — VoteRun
- [saccade-judge.v1.schema.json](../crates/saccade-core/schemas/saccade-judge.v1.schema.json) — Historical reader contract
- [saccade-keyframes.v1.schema.json](../crates/saccade-core/schemas/saccade-keyframes.v1.schema.json) — Historical reader contract
- [saccade-labels.v1.schema.json](../crates/saccade-core/schemas/saccade-labels.v1.schema.json) — Labels
- [saccade-labels.v2.schema.json](../crates/saccade-core/schemas/saccade-labels.v2.schema.json) — Labels
- [saccade-learned-quality.v1.schema.json](../crates/saccade-core/schemas/saccade-learned-quality.v1.schema.json) — QualityReport
- [saccade-localized.v1.schema.json](../crates/saccade-core/schemas/saccade-localized.v1.schema.json) — Measurement
- [saccade-locate.v1.schema.json](../crates/saccade-core/schemas/saccade-locate.v1.schema.json) — LocateReport
- [saccade-media-compare.v1.schema.json](../crates/saccade-core/schemas/saccade-media-compare.v1.schema.json) — Historical reader contract
- [saccade-media-error.v1.schema.json](../crates/saccade-core/schemas/saccade-media-error.v1.schema.json) — Historical reader contract
- [saccade-media-index-query.v1.schema.json](../crates/saccade-core/schemas/saccade-media-index-query.v1.schema.json) — Compact media index query
- [saccade-media-record.v1.schema.json](../crates/saccade-core/schemas/saccade-media-record.v1.schema.json) — Versioned media record
- [saccade-model-registry.v1.schema.json](../crates/saccade-core/schemas/saccade-model-registry.v1.schema.json) — Registry
- [saccade-model-status.v1.schema.json](../crates/saccade-core/schemas/saccade-model-status.v1.schema.json) — saccade-model-status.v1
- [saccade-motion-review.v1.schema.json](../crates/saccade-core/schemas/saccade-motion-review.v1.schema.json) — Report
- [saccade-motion-vectors.v1.schema.json](../crates/saccade-core/schemas/saccade-motion-vectors.v1.schema.json) — Sidecar
- [saccade-near-duplicate.v1.schema.json](../crates/saccade-core/schemas/saccade-near-duplicate.v1.schema.json) — Historical reader contract
- [saccade-noise.v1.schema.json](../crates/saccade-core/schemas/saccade-noise.v1.schema.json) — NoiseReport
- [saccade-notification.v1.schema.json](../crates/saccade-core/schemas/saccade-notification.v1.schema.json) — saccade-notification.v1
- [saccade-notify-result.v1.schema.json](../crates/saccade-core/schemas/saccade-notify-result.v1.schema.json) — saccade-notify-result.v1
- [saccade-ocrs.v1.schema.json](../crates/saccade-core/schemas/saccade-ocrs.v1.schema.json) — Historical reader contract
- [saccade-onnx-runtime-authority.v1.schema.json](../crates/saccade-core/schemas/saccade-onnx-runtime-authority.v1.schema.json) — Historical reader contract
- [saccade-onset.v1.schema.json](../crates/saccade-core/schemas/saccade-onset.v1.schema.json) — Document
- [saccade-perf-diff.v1.schema.json](../crates/saccade-core/schemas/saccade-perf-diff.v1.schema.json) — PerfDiff
- [saccade-perf-pairs.v1.schema.json](../crates/saccade-core/schemas/saccade-perf-pairs.v1.schema.json) — Samples
- [saccade-perf-plan.v1.schema.json](../crates/saccade-core/schemas/saccade-perf-plan.v1.schema.json) — Plan
- [saccade-perf.v1.schema.json](../crates/saccade-core/schemas/saccade-perf.v1.schema.json) — CapturePerf
- [saccade-perf.v2.schema.json](../crates/saccade-core/schemas/saccade-perf.v2.schema.json) — PerfDocument
- [saccade-pipeline-choice.v1.schema.json](../crates/saccade-core/schemas/saccade-pipeline-choice.v1.schema.json) — Historical reader contract
- [saccade-playwright-matcher.v1.schema.json](../crates/saccade-core/schemas/saccade-playwright-matcher.v1.schema.json) — saccade-playwright-matcher.v1
- [saccade-provider-mapping.v1.schema.json](../crates/saccade-core/schemas/saccade-provider-mapping.v1.schema.json) — saccade-provider-mapping.v1
- [saccade-quality-report.v1.schema.json](../crates/saccade-core/schemas/saccade-quality-report.v1.schema.json) — Sweep
- [saccade-quality-sweep.v1.schema.json](../crates/saccade-core/schemas/saccade-quality-sweep.v1.schema.json) — Manifest
- [saccade-question-report.v1.schema.json](../crates/saccade-core/schemas/saccade-question-report.v1.schema.json) — Historical reader contract
- [saccade-rank.v1.schema.json](../crates/saccade-core/schemas/saccade-rank.v1.schema.json) — RankReport
- [saccade-reference-evidence.v1.schema.json](../crates/saccade-core/schemas/saccade-reference-evidence.v1.schema.json) — Report
- [saccade-region-models.v1.schema.json](../crates/saccade-core/schemas/saccade-region-models.v1.schema.json) — ModelManifest
- [saccade-registration.v1.schema.json](../crates/saccade-core/schemas/saccade-registration.v1.schema.json) — saccade-registration.v1
- [saccade-renderdoc-extract.v1.schema.json](../crates/saccade-core/schemas/saccade-renderdoc-extract.v1.schema.json) — Capture
- [saccade-renderdoc-localization.v1.schema.json](../crates/saccade-core/schemas/saccade-renderdoc-localization.v1.schema.json) — Localization
- [saccade-report.v1.schema.json](../crates/saccade-core/schemas/saccade-report.v1.schema.json) — Report
- [saccade-required-effect.v1.schema.json](../crates/saccade-core/schemas/saccade-required-effect.v1.schema.json) — EffectResult
- [saccade-result.v1.schema.json](../crates/saccade-core/schemas/saccade-result.v1.schema.json) — saccade-result.v1
- [saccade-result.v2.schema.json](../crates/saccade-core/schemas/saccade-result.v2.schema.json) — ResultEnvelope
- [saccade-review.v1.schema.json](../crates/saccade-core/schemas/saccade-review.v1.schema.json) — Historical reader contract
- [saccade-runs.v1.schema.json](../crates/saccade-core/schemas/saccade-runs.v1.schema.json) — saccade-runs.v1
- [saccade-safety.v1.schema.json](../crates/saccade-core/schemas/saccade-safety.v1.schema.json) — safety PRE-CHECK only; not certification or formal compliance
- [saccade-sequence.v1.schema.json](../crates/saccade-core/schemas/saccade-sequence.v1.schema.json) — SequenceReport
- [saccade-similar.v1.schema.json](../crates/saccade-core/schemas/saccade-similar.v1.schema.json) — Historical reader contract
- [saccade-spatial-evidence.v1.schema.json](../crates/saccade-core/schemas/saccade-spatial-evidence.v1.schema.json) — SpatialReport
- [saccade-summary.v1.schema.json](../crates/saccade-core/schemas/saccade-summary.v1.schema.json) — saccade-summary.v1
- [saccade-sweep-captures.v1.schema.json](../crates/saccade-core/schemas/saccade-sweep-captures.v1.schema.json) — saccade-sweep-captures.v1
- [saccade-sweep-report.v1.schema.json](../crates/saccade-core/schemas/saccade-sweep-report.v1.schema.json) — saccade-sweep-report.v1
- [saccade-sweep.v1.schema.json](../crates/saccade-core/schemas/saccade-sweep.v1.schema.json) — saccade-sweep.v1
- [saccade-tesseract.v1.schema.json](../crates/saccade-core/schemas/saccade-tesseract.v1.schema.json) — OcrContract
- [saccade-text.v1.schema.json](../crates/saccade-core/schemas/saccade-text.v1.schema.json) — Historical reader contract
- [saccade-tile-temporal.v1.schema.json](../crates/saccade-core/schemas/saccade-tile-temporal.v1.schema.json) — Report
- [saccade-ui-review.v1.schema.json](../crates/saccade-core/schemas/saccade-ui-review.v1.schema.json) — Report
- [saccade-ui-source.v1.schema.json](../crates/saccade-core/schemas/saccade-ui-source.v1.schema.json) — Source
- [saccade-usage.v1.schema.json](../crates/saccade-core/schemas/saccade-usage.v1.schema.json) — Historical reader contract
- [saccade-vector-buffer.v1.schema.json](../crates/saccade-core/schemas/saccade-vector-buffer.v1.schema.json) — Buffer
- [saccade-view-summary.v1.schema.json](../crates/saccade-core/schemas/saccade-view-summary.v1.schema.json) — saccade-view-summary.v1
- [saccade-vision-observation.v1.schema.json](../crates/saccade-core/schemas/saccade-vision-observation.v1.schema.json) — ObservationReport
- [saccade-visual-trial-frozen.v1.schema.json](../crates/saccade-core/schemas/saccade-visual-trial-frozen.v1.schema.json) — Frozen
- [saccade-visual-trial-plan.v1.schema.json](../crates/saccade-core/schemas/saccade-visual-trial-plan.v1.schema.json) — Plan
- [saccade-visual-trial-receipt.v1.schema.json](../crates/saccade-core/schemas/saccade-visual-trial-receipt.v1.schema.json) — Receipt
- [saccade-watermark.v1.schema.json](../crates/saccade-core/schemas/saccade-watermark.v1.schema.json) — WatermarkReport
<!-- schema-index:end -->

Historical validators and fixtures do not imply that retired writers or commands
are supported. Active families are report, evidence, result v2, performance v2,
image noise and labels v2; evaluation and precheck families are experimental.

<!-- wave4 -->
Experimental AI advice is a separate [saccade-assist.v1](../crates/saccade-core/schemas/saccade-assist.v1.schema.json)
sidecar; it cannot alter measured reports or numerical grounded explanations.
The [Batch receipt](../crates/saccade-core/schemas/saccade-assist-batch.v1.schema.json)
separates uncertain submission and per-item failure from job completion.
See [assist](assist.md) for the closed input, identity and advisory contracts.

<!-- wave4 artifact wrappers -->
Prepared requests, replay records and mask accounting use
`saccade-assist-requests.v1`, `saccade-assist-observations.v1` and
`saccade-assist-mask-audit.v1`. Individual-mask input is `saccade-assist-masks.v1`.
Constructed truth, its private oracle and heavy-gate receipts use
`saccade-constructed-truth.v1`, `saccade-constructed-oracle.v1` and
`saccade-assist-gates.v1`; schemas are in the existing schema directory.

<!-- wave4b -->
The [source-bound Batch plan](../crates/saccade-core/schemas/saccade-assist-batch-plan.v1.schema.json)
uses `saccade-assist-batch-plan.v1`. Public submission, status and collection
reproduce its frozen payloads from hash-verified transitive source files.

## Mask treatment

`mask_mode = "exclude"` retains historical score exclusion after spatial filtering.
`mask_mode = "neutralize"` replaces masked test samples (including alpha/HDR channels)
with reference samples before FLIP filtering, and excludes the same bitmap from scoring.
Core defaults to `exclude`; Playwright dynamic exclusions, sweeps and the generated UI
configuration use `neutralize`. No mask grows automatically. Each pair report records
`config.mask_mode`; older reports read as `exclude`. Exclusion audits retain original
full-map errors and the diagnostic verdict with masks removed.

The shared model registry can hold typed embedding and OCR contracts under `contracts`.
Historical individual contracts remain readable projections. Generated local exports
are supplied by hash in the cache; their `example.invalid/local-exports` URLs explicitly
have no remote distribution endpoint. Official checkpoint URLs, licences and revisions
are in the export receipt. These local exports must be reproduced rather than downloaded.

The generated qualification registry (its host location is recorded in
`INTEGRATION-REPORT.md`) combines
vision, embedding and OCR pins under the same registry schema. Legacy test inputs are
checked projections of it. Host-specific OCR paths stay in this supplied registry, not
in the distributed default catalogue. Runtime flags select that single supplied registry.

<!-- wave8 -->
Media analysis uses [saccade-media-record.v1](../crates/saccade-core/schemas/saccade-media-record.v1.schema.json).
Each section has `ok|skipped|failed`, algorithm/model provenance and elapsed timing.
See [media analysis](media.md); failed optional sections remain observable without aborting
unless `strict` is set. Existing report/result contracts keep their meaning.
