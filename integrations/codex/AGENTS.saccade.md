# Visual checks with saccade

Use saccade for screenshot regressions, renderer refactors, ablations and capture archives. Commands below run from a saccade checkout with the binary on PATH; replace the example input directories with the project's captures. Keep all outputs separate from inputs. For MCP, use project-relative paths inside the server's `--root`.

## Choose a command

1. Checking new captures against an approved reference? Use `compare`. Checking a refactor that should change no samples? Use `identity` and inspect `bit_identical` as well as the perceptual verdict.
2. Comparing alternatives? Use `rank` for reference error, `runs` for a run matrix, or `view --blind` for human preference. Reference error is not a preference judgement.
3. Comparing animation frames? Use `sequence`. Locating the first changed revision in ordered captures? Use `bisect --runs`. Iterating on captures? Use `watch`.
4. A comparison failed? Read its lean JSON, then use `summary`, `explain` and `snapshot` to inspect evidence. Request bounded questions with `decision-request`, and record answers as proposals with `decide`.
5. Intent is ambiguous? Use `ask` with a local evidence link and wait for a human. An experimental `judge` panel can suggest answers; calibrate it against the project's own human finals before trusting a gate.

## Copyable CLI commands

Run the comparison first. The shipped example intentionally exits 1; that still produces a report. Read `verdict`, `totals`, `failing`, `hotspots`, `paths` and `next_step` before opening the full JSON.

```sh
saccade compare examples/baseline examples/capture --out .saccade-agent/report --json
saccade summary .saccade-agent/report/saccade-report.v1.json --format json
saccade entries .saccade-agent/report/saccade-report.v1.json --status fail,error --limit 20 --json
saccade explain .saccade-agent/report/saccade-report.v1.json --out .saccade-agent/explain
saccade snapshot .saccade-agent/report/saccade-report.v1.json --entry sphere_shadow.png --state 'layout=swipe&split=0.5&heat=0.6&hotspot=1&zoom=3' --out .saccade-agent/shadow.png --json
saccade decision-request .saccade-agent/report/saccade-report.v1.json --all-failing --question accept --intent 'Soften the shadow'
saccade decide .saccade-agent/report/saccade-report.v1.json --entry sphere_shadow.png --question accept --answer needs_human --prob 0.7 --source agent --note 'Confirm the intended shadow appearance' --json
```

Other jobs, using the committed datasets:

```sh
saccade identity examples/baseline examples/capture --out .saccade-agent/identity --json
saccade rank showcases/texture-compression/baseline showcases/texture-compression/candidates/block-low showcases/texture-compression/candidates/block-high --out .saccade-agent/rank --metric mean --threshold 0.001 --json
saccade runs examples/baseline examples/capture --json
saccade sequence showcases/lod-transition/baseline showcases/lod-transition/capture --pattern 'frame_*.png' --threshold 0.005 --out .saccade-agent/sequence --json
saccade bisect --runs examples/baseline examples/capture --out .saccade-agent/bisect --json
saccade watch examples/baseline examples/capture --out .saccade-agent/watch --once --json
saccade view examples/baseline examples/capture --blind --key-out .saccade-agent/blind-key.json --out .saccade-agent/view
```

For a continuous watch remove `--once`. JSON watch output is JSONL; Ctrl-C returns the last result's exit code. Bisect assumes monotonic divergence and reports observed reversals and skipped-probe uncertainty. CLI capture-command bisect executes user shell code; MCP only supports existing runs.

For a human review, start the server in a separate terminal, open the inbox URL, then post a question. This server's discovery cache stays inside the project for MCP access:

```sh
saccade serve examples --port 7878 --cache-dir .saccade-agent/serve-cache
```

```sh
saccade ask --serve http://127.0.0.1:7878 --cache-dir .saccade-agent/serve-cache --question 'Is the shadow change intended?' --answers accept,reject,needs_work --link '/compare?runs=baseline,capture#entry=sphere_shadow.png' --wait --timeout 600 --json
```

Humans answer at `http://127.0.0.1:7878/inbox`. Keep `serve.json` and its token private. The answer records feedback; it does not approve or update a baseline.

Judge mode is experimental: answers are proposals. Start with a dry run; omit `--dry-run` only when provider calls and sharing the encoded evidence are authorized. For calibration, replace the label/run filenames with actual human final decisions and judge results.

```sh
saccade judge .saccade-agent/report/saccade-report.v1.json --panel examples/panel.toml --intent 'Soften the shadow' --both-orders --dry-run
saccade judge calibrate --labels reviewer-a.json reviewer-b.json --runs .saccade-agent/report/saccade-judge.v1.json --out .saccade-agent/saccade-calibration.v1.json
```

## CLI ↔ MCP mapping

Start MCP with `saccade mcp --root .`. Names and input keys differ from CLI flags; call tools by their advertised schemas.

| CLI command | MCP tool | Main MCP inputs |
|---|---|---|
| `saccade compare` | `saccade_compare` | `baseline_dir`, `capture_dir`, `out_dir`, optional `config` |
| `saccade identity` | `saccade_identity` | `parent_dir`, `candidate_dir`, `out_dir` |
| `saccade summary` | `saccade_summary` | `report_json` |
| `saccade entries` | `saccade_list_entries` | `report_json`, optional `status`, `name`, `limit`, `offset` or `cursor` |
| `saccade entries --name sphere_shadow.png --limit 1 --json` (page wrapper) | `saccade_get_entry` (one full entry) | `report_json`, exact `name` |
| `saccade explain` | `saccade_explain` | `report_json`, `out_dir`, optional `blind` and `key_out` |
| `saccade snapshot` | `saccade_snapshot` | `report_json` or `view_dir`, optional `entry` and `state`; server chooses output path |
| `saccade decision-request` | `saccade_decision_request` | `report_json`, optional `entry`, `all_failing`, `question`, `intent` |
| `saccade decide` | `saccade_decide` | `report_json`, `entry`, `answer`, `source`, optional `prob`, `question`, `note` |
| `saccade rank` | `saccade_rank` | `reference_dir`, `candidate_dirs`, `out_dir` |
| `saccade runs` | `saccade_compare_runs` | `ref_dir`, `run_dirs` |
| `saccade sequence` | `saccade_sequence` | `baseline_dir`, `capture_dir`, `out_dir`, optional `pattern` |
| `saccade bisect --runs` | `saccade_bisect` | `runs`, `out_dir`, optional `good`, `entries` |
| `saccade watch` | `saccade_watch_status` | Start server with `--watch examples/baseline:examples/capture`; query `capture_dir` |
| `saccade ask` | `saccade_ask_human` | `serve`, `question`, `allowed_answers`, optional `wait`, `timeout`, `link`, `cache_dir` |
| `saccade ask --wait` (answer included) | `saccade_inbox_get` (read existing item) | `serve`, `id`, optional `cache_dir`; no separate CLI get command |
| `saccade judge` (experimental) | `saccade_judge` | `target`, `panel`, optional `intent`, `dry_run`, `calibration` |
| `saccade judge calibrate` (experimental) | `saccade_judge_calibrate` | `labels`, optional `runs`, `out`, `target_accuracy`, `min_support` |
| `saccade view`, `saccade serve`, `saccade approve`, `saccade judge selftest` | CLI only | No corresponding MCP tool |

Example MCP arguments for `saccade_compare`:

```json
{"baseline_dir":"examples/baseline","capture_dir":"examples/capture","out_dir":".saccade-agent/report","include_images":true}
```

## Decisions and exit codes

**never approve without a human or a calibrated gate**. Inspect evidence first. Do not change thresholds to hide regressions or copy baselines from a model proposal, uncalibrated panel agreement or inbox answer. A calibrated gate must be explicitly configured for this project, source and question against human final decisions with sufficient support; deterministic failures remain refused. `approve` consumes final decisions and checks capture hashes. Human baseline updates need explicit authorization; do not invoke `approve` just because an agent suggested acceptance.

| Code | Meaning |
|---|---|
| `0` | Comparison passes, or the requested operation completed. For judge/decide/ask this does not mean the image was accepted. |
| `1` | Regression, or bisect found the first diverging run. Read the report even on this exit. |
| `2` | Usage, config or IO error; also inconclusive/non-monotonic bisect and `ask` timeout (`timed_out: true`, item remains open). |

MCP regressions are successful tool results with `verdict: "regression"`; `isError: true` means the call failed. JSON mode emits `saccade-error.v1` for errors. Lean floats are rounded; use full reports for exact stored statistics, `buffer.heatmap_max` for numerical heatmap legends and `diagnostics.perf_not_comparable` for unpaired timing names.
