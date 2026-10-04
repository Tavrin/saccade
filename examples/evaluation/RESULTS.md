# Constructed-truth evaluation

60 constructed cases and 26 recorded cases. 12 recorded cases have unknown truth and are excluded from scoring.
Splits: {'calibration': 40, 'development': 23, 'held_out': 23}. All cases permit provider egress under design §16. No images or private paths are published.
Frozen manifest: `sha256:1d2b0d4df6a04f18e8c584f559fec482dda9cf19be3699ecc57eaa1126cf1366`. Calls: Jev 389/400; Gemini 250/250.

| Question | Provider / encoding | Held-out cases | Coverage | Availability | Accuracy | Qualified |
|---|---|---:|---:|---:|---:|---|
| triage.route.v1 | rules / deterministic | 23 | 100.0% | 100.0% | 30.0% | no |
| triage.route.v1 | jev / direct | 23 | 56.5% | 100.0% | 50.0% | no |
| triage.route.v1 | jev / enriched | 23 | 73.9% | 100.0% | 57.1% | no |
| triage.route.v1 | jev / enriched_without_priors | 23 | 65.2% | 100.0% | 54.2% | no |
| triage.route.v1 | gemini / gemini_alone | 23 | 76.1% | 76.1% | 79.3% | no |
| vision.route.v1 | rules / deterministic | 23 | 100.0% | 100.0% | 0.0% | no |
| vision.route.v1 | jev / direct | 23 | 60.9% | 100.0% | 58.3% | no |
| vision.route.v1 | jev / enriched | 23 | 69.6% | 100.0% | 50.0% | no |
| vision.route.v1 | jev / enriched_without_priors | 23 | 67.4% | 100.0% | 52.0% | no |
| vision.route.v1 | gemini / gemini_alone | 23 | 100.0% | 100.0% | 75.0% | no |
| perf.interpret.v1 | rules / deterministic | 2 | 100.0% | 100.0% | 100.0% | no |
| perf.interpret.v1 | jev / direct | 2 | 50.0% | 100.0% | 100.0% | no |
| perf.interpret.v1 | jev / enriched | 2 | 0.0% | 100.0% | — | no |
| perf.interpret.v1 | jev / enriched_without_priors | 2 | 0.0% | 100.0% | — | no |
| capture.disposition.v1 | rules / deterministic | 23 | 100.0% | 100.0% | 100.0% | no |
| capture.disposition.v1 | jev / direct | 23 | 8.7% | 100.0% | 100.0% | no |
| capture.disposition.v1 | jev / enriched | 23 | 15.2% | 100.0% | 57.1% | no |
| capture.disposition.v1 | jev / enriched_without_priors | 23 | 8.7% | 100.0% | 100.0% | no |
| intent.match.v1 | rules / deterministic | 23 | 65.2% | 100.0% | 100.0% | no |
| intent.match.v1 | jev / direct | 23 | 65.2% | 73.9% | 100.0% | no |
| intent.match.v1 | jev / enriched | 23 | 95.7% | 97.8% | 92.1% | no |
| intent.match.v1 | jev / enriched_without_priors | 23 | 95.7% | 97.8% | 92.1% | no |
| intent.match.v1 | gemini / gemini_alone | 23 | 95.7% | 95.7% | 94.7% | no |

| Gemini model | Attempts | HTTP successes | Pinned valid / scheduled |
|---|---:|---:|---:|
| gemini-3.8-flash | 135 | 112 | 4 / 4 |
| gemini-3.7-flash | 29 | 8 | 4 / 4 |
| gemini-3.6-flash | 40 | 16 | 4 / 4 |
| gemini-3.5-flash | 46 | 22 | 4 / 4 |

Job outcomes: {'answered': 514, 'invalid': 1, 'unavailable': 4, 'budget_blocked': 1}. Unattempted provider jobs: 4.
Per-model counts, latency, calibration, order sensitivity, truth-source accuracy, per-class scores and grouped confidence bounds are in RESULTS.json.

Newest-first retained. No quality-based reorder is supported by this unqualified corpus. Default: gemini-3.8-flash, gemini-3.7-flash, gemini-3.6-flash, gemini-3.5-flash.

No question qualifies. The corpus is below the required held-out support; important-miss tolerance is undeclared.

The private source mapping retains scenes, cameras, interventions, declarations, recorded verdict citations and provider responses.
HDR and temporal scope, model failures and exact availability shortfalls are recorded in the JSON.

<!-- dispatch-completion-record -->

All-job processing complete: true. Unresolved provider jobs: 5; terminal invalid provider jobs: 1. Evaluation incomplete: true.

| Provider | Requested model | Failure category | Attempts |
|---|---|---|---:|
| gemini | gemini-3.5-flash | 429 | 14 |
| gemini | gemini-3.5-flash | 429 RESOURCE_EXHAUSTED | 1 |
| gemini | gemini-3.5-flash | 503 | 9 |
| gemini | gemini-3.6-flash | 429 | 10 |
| gemini | gemini-3.6-flash | 429 RESOURCE_EXHAUSTED | 1 |
| gemini | gemini-3.6-flash | 503 | 13 |
| gemini | gemini-3.7-flash | 429 RESOURCE_EXHAUSTED | 1 |
| gemini | gemini-3.7-flash | 503 | 18 |
| gemini | gemini-3.7-flash | 503 UNAVAILABLE | 1 |
| gemini | gemini-3.7-flash | transient | 1 |
| gemini | gemini-3.8-flash | 429 | 6 |
| gemini | gemini-3.8-flash | 429 RESOURCE_EXHAUSTED | 1 |
| gemini | gemini-3.8-flash | 503 | 15 |
| gemini | gemini-3.8-flash | transient | 1 |
| jev | jev-latest | 400 | 14 |

Latency excludes queueing and full pipeline extraction. Harmful-miss bounds use the frozen catalog; they do not establish semantic safety. Missing responses and unresolved pairs remain excluded from committed-answer accuracy.

## Paid-tier resume

Amendment `sha256:a8f1bf8c0674ece9c359b9c7f8eea76d07e882cbcd4ba169ed6d2f2de59697c4` (epoch `r12-paid-tier/1`) changes transport pacing and opens a new elapsed window. The frozen corpus, splits, rubrics and job plan are unchanged. Calls in this epoch: Jev 238/249, Gemini 128/128.

Gemini spend estimate for this epoch: $3.4566 USD from API token usage and [published standard paid-tier prices](https://ai.google.dev/gemini-api/docs/pricing); 0 successful responses lack usage metadata. This is an estimate, not an invoice.

| Gemini model | Paid-epoch attempts | HTTP successes | Schema-valid jobs |
|---|---:|---:|---:|
| gemini-3.8-flash | 106 | 106 | 88 |
| gemini-3.7-flash | 6 | 6 | 6 |
| gemini-3.6-flash | 7 | 7 | 7 |
| gemini-3.5-flash | 9 | 9 | 9 |

Source revalidation: 244 recorded upstream files are absent; frozen generated packets and their hashes were rechecked. This limits source-level replay assurance.

Remainder: 5 unresolved provider jobs plus 1 terminal invalid response. Unused caps: Jev 11, Gemini 0. The table below gives the unresolved reasons.

| Unresolved provider jobs | Encoding | Mode | Reason | Jobs |
|---|---|---|---|---:|
| gemini | gemini_alone | production_policy | budget_exhausted | 1 |
| jev | enriched | production_policy | required visual extraction unavailable | 2 |
| jev | enriched_without_priors | production_policy | required visual extraction unavailable | 2 |

### Held-out calibration and order sensitivity

| Question | Provider / encoding | Raw ECE by answering model | Order disagreement (pairs) |
|---|---|---|---:|
| triage.route.v1 | jev / direct | jev-1.13.0: 0.352 | — |
| triage.route.v1 | jev / enriched | jev-1.13.0 (vision gemini-3.8-flash): 0.190; jev-1.13.0 (vision gemini-3.5-flash): 0.333; jev-1.13.0 (vision gemini-3.6-flash): 0.168; jev-1.13.0 (vision gemini-3.5-flash): 0.390 | 9.5% (21) |
| triage.route.v1 | jev / enriched_without_priors | jev-1.13.0 (vision gemini-3.8-flash): 0.332; jev-1.13.0 (vision gemini-3.5-flash): 0.427; jev-1.13.0 (vision gemini-3.6-flash): 0.340; jev-1.13.0 (vision gemini-3.5-flash): 0.380 | 9.5% (21) |
| triage.route.v1 | gemini / gemini_alone | — | 12.5% (16) |
| vision.route.v1 | jev / direct | jev-1.13.0: 0.257 | — |
| vision.route.v1 | jev / enriched | jev-1.13.0 (vision gemini-3.8-flash): 0.187; jev-1.13.0 (vision gemini-3.5-flash): 0.142; jev-1.13.0 (vision gemini-3.6-flash): 0.335 | 9.5% (21) |
| vision.route.v1 | jev / enriched_without_priors | jev-1.13.0 (vision gemini-3.8-flash): 0.186; jev-1.13.0 (vision gemini-3.5-flash): 0.314; jev-1.13.0 (vision gemini-3.6-flash): 0.365 | 14.3% (21) |
| vision.route.v1 | gemini / gemini_alone | — | 0.0% (21) |
| perf.interpret.v1 | jev / direct | jev-1.13.0: 0.400 | — |
| perf.interpret.v1 | jev / enriched | — | 0.0% (2) |
| perf.interpret.v1 | jev / enriched_without_priors | — | 0.0% (2) |
| capture.disposition.v1 | jev / direct | jev-1.13.0: 0.465 | — |
| capture.disposition.v1 | jev / enriched | jev-1.13.0 (vision gemini-3.8-flash): 0.427; jev-1.13.0 (vision gemini-3.5-flash): 0.550 | 4.8% (21) |
| capture.disposition.v1 | jev / enriched_without_priors | jev-1.13.0 (vision gemini-3.8-flash): 0.315; jev-1.13.0 (vision gemini-3.5-flash): 0.515 | 0.0% (21) |
| intent.match.v1 | jev / direct | jev-1.13.0: 0.031 | — |
| intent.match.v1 | jev / enriched | jev-1.13.0 (vision gemini-3.8-flash): 0.117; jev-1.13.0 (vision gemini-3.5-flash): 0.075; jev-1.13.0 (vision gemini-3.6-flash): 0.230; jev-1.13.0 (vision gemini-3.5-flash): 0.035 | 4.8% (21) |
| intent.match.v1 | jev / enriched_without_priors | jev-1.13.0 (vision gemini-3.8-flash): 0.149; jev-1.13.0 (vision gemini-3.5-flash): 0.087; jev-1.13.0 (vision gemini-3.6-flash): 0.225; jev-1.13.0 (vision gemini-3.5-flash): 0.040 | 0.0% (21) |
| intent.match.v1 | gemini / gemini_alone | — | 4.8% (21) |

### Qualification gates (§9, applied as written)

| Question | Truth-known held-out cases | More needed for 50 |
|---|---:|---:|
| triage.route.v1 | 20 | 30 |
| vision.route.v1 | 20 | 30 |
| perf.interpret.v1 | 2 | 48 |
| capture.disposition.v1 | 20 | 30 |
| intent.match.v1 | 20 | 30 |

No question qualifies: every question is below the held-out gate, important-miss tolerance is undeclared, no calibration identity is qualified, and provider rows below 95% attempted completion also fail that gate. Gates are not lowered.

## Constructed-truth expansion freeze

The second manifest (`moss-pilot-expanded.toml`, SHA-256 `73fa53df42348eff4ac8821b38f18fa029222bcbc4ff43ff8c1533539dbeca86`) extends the original 86-case freeze as an exact prefix. Its `freeze-amendment.json` binds 180 new independent construction records: 90 held-out, 36 calibration and 54 development. The rubric hash is unchanged. This expansion made **zero provider calls**; its 630 new Jev and 268 new Gemini initial requests are planned, not attempted. The paid-tier results above were not rescored.

| New family | Cases |
|---|---:|
| Native performance | 78 |
| Measured no-effect | 20 |
| Local regression | 22 |
| Intended visual | 22 |
| Metadata confound | 19 |
| Semantic HUD | 19 |

Truth-known held-out support is 110 cases each for `triage.route.v1`, `vision.route.v1`, `capture.disposition.v1` and `intent.match.v1`, and 62 for `perf.interpret.v1`. Thus the numeric 50-case gate is met for all five questions. New performance evidence comprises 97 producer-rejected or incomparable pairs and one qualified whole-frame pair. The latter uses three independently rendered unchanged captures and a hash-bound projection that excludes unstable pass-gap attribution and dynamic measured fields from configuration identity. Production noise and comparison qualify its frame measurement, but the unattributed frame bound prevents a no-effect conclusion; its `perf.interpret.v1` truth is `collect_more_evidence`. No pass-level or causal performance claim follows.

The provider jobs in the expanded manifest remain unattempted. No question is newly qualified for automatic routing: completion, calibration identity and the declared important-miss tolerance still require a later evaluation. Older recorded private inputs remain absent, so the original public freeze is sealed and preserved but those source files were not revalidated in this lane.
