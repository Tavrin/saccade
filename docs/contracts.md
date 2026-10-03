# Contracts

Measured pair reports remain `saccade-report.v1`. Canonical cases, requests,
proposals, human decisions and receipts use `saccade-evidence.v1`.
CLI and MCP use bounded `saccade-result.v2`. The preserved Moss CLI
`identity --json` discriminator is `saccade-result.v1`; it retains `schema`,
`mode`, `verdict`, `totals.{pass,fail,error,missing,new,total}` and
`failing[].error`. Read the complete report to establish exact equality; a
lean pass count alone is insufficient. See the recorded R5 decision.

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

## Generated schema index

<!-- schema-index:start -->
- [saccade-a11y.v1.schema.json](../schemas/saccade-a11y.v1.schema.json) — a11y PRE-CHECK only; not certification or formal compliance
- [saccade-ablate.v1.schema.json](../schemas/saccade-ablate.v1.schema.json) — Ablation
- [saccade-approve.v1.schema.json](../schemas/saccade-approve.v1.schema.json) — saccade-approve.v1
- [saccade-ask-result.v1.schema.json](../schemas/saccade-ask-result.v1.schema.json) — AskResult
- [saccade-bisect.v1.schema.json](../schemas/saccade-bisect.v1.schema.json) — BisectResult
- [saccade-blind-key.v1.schema.json](../schemas/saccade-blind-key.v1.schema.json) — BlindKey
- [saccade-calibration.v1.schema.json](../schemas/saccade-calibration.v1.schema.json) — Historical reader contract
- [saccade-decide-result.v1.schema.json](../schemas/saccade-decide-result.v1.schema.json) — saccade-decide-result.v1
- [saccade-decision-request.v1.schema.json](../schemas/saccade-decision-request.v1.schema.json) — saccade-decision-request.v1
- [saccade-decisions.v1.schema.json](../schemas/saccade-decisions.v1.schema.json) — Decisions
- [saccade-entries.v1.schema.json](../schemas/saccade-entries.v1.schema.json) — EntriesPage
- [saccade-error.v1.schema.json](../schemas/saccade-error.v1.schema.json) — saccade-error.v1
- [saccade-evidence.v1.schema.json](../schemas/saccade-evidence.v1.schema.json) — Document
- [saccade-explain-blind-key.v1.schema.json](../schemas/saccade-explain-blind-key.v1.schema.json) — ExplainBlindKey
- [saccade-explain-result.v1.schema.json](../schemas/saccade-explain-result.v1.schema.json) — saccade-explain-result.v1
- [saccade-explain.v1.schema.json](../schemas/saccade-explain.v1.schema.json) — ExplainPack
- [saccade-inbox-item.v1.schema.json](../schemas/saccade-inbox-item.v1.schema.json) — Item
- [saccade-judge-bench.v1.schema.json](../schemas/saccade-judge-bench.v1.schema.json) — Historical reader contract
- [saccade-judge-selftest.v1.schema.json](../schemas/saccade-judge-selftest.v1.schema.json) — Historical reader contract
- [saccade-judge-vote-api.v1.schema.json](../schemas/saccade-judge-vote-api.v1.schema.json) — Historical reader contract
- [saccade-judge-votes.v1.schema.json](../schemas/saccade-judge-votes.v1.schema.json) — VoteRun
- [saccade-judge.v1.schema.json](../schemas/saccade-judge.v1.schema.json) — Historical reader contract
- [saccade-labels.v1.schema.json](../schemas/saccade-labels.v1.schema.json) — Labels
- [saccade-labels.v2.schema.json](../schemas/saccade-labels.v2.schema.json) — Labels
- [saccade-noise.v1.schema.json](../schemas/saccade-noise.v1.schema.json) — NoiseReport
- [saccade-perf-diff.v1.schema.json](../schemas/saccade-perf-diff.v1.schema.json) — PerfDiff
- [saccade-perf.v1.schema.json](../schemas/saccade-perf.v1.schema.json) — CapturePerf
- [saccade-perf.v2.schema.json](../schemas/saccade-perf.v2.schema.json) — PerfDocument
- [saccade-rank.v1.schema.json](../schemas/saccade-rank.v1.schema.json) — RankReport
- [saccade-report.v1.schema.json](../schemas/saccade-report.v1.schema.json) — Report
- [saccade-result.v1.schema.json](../schemas/saccade-result.v1.schema.json) — saccade-result.v1
- [saccade-result.v2.schema.json](../schemas/saccade-result.v2.schema.json) — ResultEnvelope
- [saccade-review.v1.schema.json](../schemas/saccade-review.v1.schema.json) — Historical reader contract
- [saccade-runs.v1.schema.json](../schemas/saccade-runs.v1.schema.json) — saccade-runs.v1
- [saccade-safety.v1.schema.json](../schemas/saccade-safety.v1.schema.json) — safety PRE-CHECK only; not certification or formal compliance
- [saccade-sequence.v1.schema.json](../schemas/saccade-sequence.v1.schema.json) — SequenceReport
- [saccade-summary.v1.schema.json](../schemas/saccade-summary.v1.schema.json) — saccade-summary.v1
- [saccade-view-summary.v1.schema.json](../schemas/saccade-view-summary.v1.schema.json) — saccade-view-summary.v1
<!-- schema-index:end -->

Historical validators and fixtures do not imply that retired writers or commands
are supported. Active families are report, evidence, result v2, performance v2,
image noise and labels v2; evaluation and precheck families are experimental.
