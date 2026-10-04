# Constructed-truth evaluation

60 constructed cases and 26 recorded cases. 12 recorded cases have unknown truth and are excluded from scoring.
Splits: {'calibration': 40, 'development': 23, 'held_out': 23}. All cases permit provider egress under design §16. No images or private paths are published.
Frozen manifest: `sha256:1d2b0d4df6a04f18e8c584f559fec482dda9cf19be3699ecc57eaa1126cf1366`. Calls: Jev 151/400; Gemini 122/250.

| Question | Provider / encoding | Held-out cases | Coverage | Availability | Accuracy | Qualified |
|---|---|---:|---:|---:|---:|---|
| triage.route.v1 | rules / deterministic | 23 | 100.0% | 100.0% | 30.0% | no |
| triage.route.v1 | jev / direct | 23 | 56.5% | 100.0% | 50.0% | no |
| triage.route.v1 | jev / enriched | 23 | 21.7% | 30.4% | 55.6% | no |
| triage.route.v1 | jev / enriched_without_priors | 23 | 19.6% | 30.4% | 62.5% | no |
| triage.route.v1 | gemini / gemini_alone | 23 | 23.9% | 23.9% | 80.0% | no |
| vision.route.v1 | rules / deterministic | 23 | 100.0% | 100.0% | 0.0% | no |
| vision.route.v1 | jev / direct | 23 | 60.9% | 100.0% | 58.3% | no |
| vision.route.v1 | jev / enriched | 23 | 19.6% | 30.4% | 75.0% | no |
| vision.route.v1 | jev / enriched_without_priors | 23 | 19.6% | 30.4% | 75.0% | no |
| vision.route.v1 | gemini / gemini_alone | 23 | 30.4% | 30.4% | 84.6% | no |
| perf.interpret.v1 | rules / deterministic | 2 | 100.0% | 100.0% | 100.0% | no |
| perf.interpret.v1 | jev / direct | 2 | 50.0% | 100.0% | 100.0% | no |
| perf.interpret.v1 | jev / enriched | 2 | 0.0% | 0.0% | — | no |
| perf.interpret.v1 | jev / enriched_without_priors | 2 | 0.0% | 0.0% | — | no |
| capture.disposition.v1 | rules / deterministic | 23 | 100.0% | 100.0% | 100.0% | no |
| capture.disposition.v1 | jev / direct | 23 | 8.7% | 100.0% | 100.0% | no |
| capture.disposition.v1 | jev / enriched | 23 | 4.3% | 30.4% | 50.0% | no |
| capture.disposition.v1 | jev / enriched_without_priors | 23 | 2.2% | 30.4% | 100.0% | no |
| intent.match.v1 | rules / deterministic | 23 | 65.2% | 100.0% | 100.0% | no |
| intent.match.v1 | jev / direct | 23 | 65.2% | 73.9% | 100.0% | no |
| intent.match.v1 | jev / enriched | 23 | 28.3% | 28.3% | 83.3% | no |
| intent.match.v1 | jev / enriched_without_priors | 23 | 28.3% | 28.3% | 83.3% | no |
| intent.match.v1 | gemini / gemini_alone | 23 | 28.3% | 28.3% | 91.7% | no |

| Gemini model | Attempts | HTTP successes | Pinned valid / scheduled |
|---|---:|---:|---:|
| gemini-3.8-flash | 29 | 6 | 0 / 4 |
| gemini-3.7-flash | 23 | 2 | 0 / 4 |
| gemini-3.6-flash | 33 | 9 | 1 / 4 |
| gemini-3.5-flash | 37 | 13 | 0 / 4 |

Job outcomes: {'deferred': 112, 'answered': 214, 'unavailable': 194}. Unattempted provider jobs: 257.
Per-model counts, latency, calibration, order sensitivity, truth-source accuracy, per-class scores and grouped confidence bounds are in RESULTS.json.

Newest-first retained. No quality-based reorder is supported by this unqualified corpus. Default: gemini-3.8-flash, gemini-3.7-flash, gemini-3.6-flash, gemini-3.5-flash.

No question qualifies. The corpus is below the required held-out support; important-miss tolerance is undeclared.

The private source mapping retains scenes, cameras, interventions, declarations, recorded verdict citations and provider responses.
HDR and temporal scope, model failures and exact availability shortfalls are recorded in the JSON.

<!-- dispatch-completion-record -->

All-job processing complete: true. Unresolved provider jobs: 306. Evaluation incomplete: true.

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

## Resume after the transport fix

Amendment `sha256:cf9226c2e5771f99113138a09396f732c01d665ad5f0fc68529cd28987a58da8` (epoch `r12-transport-fix/1`) replaces only transport identities and opens a new elapsed window; corpus, splits, rubrics and plan are unchanged. Calls this run: Jev 39/288, Gemini 5/133 (Jev includes one diagnosis replay of a refused request).

| Gemini model | Provider quota | Limit |
|---|---|---:|
| gemini-3.5-flash | GenerateRequestsPerDayPerProjectPerModel-FreeTier | 20 |
| gemini-3.6-flash | GenerateRequestsPerDayPerProjectPerModel-FreeTier | 20 |
| gemini-3.7-flash | GenerateRequestsPerDayPerProjectPerModel-FreeTier | 20 |
| gemini-3.8-flash | GenerateRequestsPerDayPerProjectPerModel-FreeTier | 20 |

The 429s are a daily per-model free-tier quota, not a request rate; pacing cannot recover them. The transport honours the provider retry delay and stops spending attempts. Every Gemini chain model hit the same limit, so the chain recommendation is unchanged: newest-first, with fallback as the only way past one model's daily quota on this tier. No model is consistently missing.

Remainder: 306 provider jobs. Gemini jobs wait for the provider's daily reset; Jev enriched jobs wait on their Gemini observations. Unused caps: Jev 249, Gemini 128. Resuming after the reset continues from the ledger; raising the quota is a paid-tier (spending) decision.

| Unresolved provider jobs | Encoding | Mode | Reason | Jobs |
|---|---|---|---|---:|
| gemini | gemini_alone | pinned_provider | deferred: provider retry window | 15 |
| gemini | gemini_alone | production_policy | deferred: provider retry window | 97 |
| jev | enriched | production_policy | required visual extraction unavailable | 97 |
| jev | enriched_without_priors | production_policy | required visual extraction unavailable | 97 |

### Held-out calibration and order sensitivity

| Question | Provider / encoding | Raw ECE by answering model | Order disagreement (pairs) |
|---|---|---|---:|
| triage.route.v1 | jev / direct | jev-1.13.0: 0.352 | — |
| triage.route.v1 | jev / enriched | jev-1.13.0 (vision gemini-3.5-flash): 0.273; jev-1.13.0 (vision gemini-3.8-flash): 0.245; jev-1.13.0 (vision gemini-3.6-flash): 0.313; jev-1.13.0 (vision gemini-3.5-flash): 0.390 | 0.0% (3) |
| triage.route.v1 | jev / enriched_without_priors | jev-1.13.0 (vision gemini-3.5-flash): 0.427; jev-1.13.0 (vision gemini-3.8-flash): 0.215; jev-1.13.0 (vision gemini-3.6-flash): 0.415; jev-1.13.0 (vision gemini-3.5-flash): 0.380 | 33.3% (3) |
| triage.route.v1 | gemini / gemini_alone | — | 0.0% (2) |
| vision.route.v1 | jev / direct | jev-1.13.0: 0.257 | — |
| vision.route.v1 | jev / enriched | jev-1.13.0 (vision gemini-3.5-flash): 0.210; jev-1.13.0 (vision gemini-3.8-flash): 0.670; jev-1.13.0 (vision gemini-3.6-flash): 0.320 | 0.0% (3) |
| vision.route.v1 | jev / enriched_without_priors | jev-1.13.0 (vision gemini-3.5-flash): 0.212; jev-1.13.0 (vision gemini-3.8-flash): 0.650; jev-1.13.0 (vision gemini-3.6-flash): 0.360 | 0.0% (3) |
| vision.route.v1 | gemini / gemini_alone | — | 0.0% (3) |
| perf.interpret.v1 | jev / direct | jev-1.13.0: 0.400 | — |
| perf.interpret.v1 | jev / enriched | — | — |
| perf.interpret.v1 | jev / enriched_without_priors | — | — |
| capture.disposition.v1 | jev / direct | jev-1.13.0: 0.465 | — |
| capture.disposition.v1 | jev / enriched | jev-1.13.0 (vision gemini-3.5-flash): 0.530 | 33.3% (3) |
| capture.disposition.v1 | jev / enriched_without_priors | jev-1.13.0 (vision gemini-3.5-flash): 0.510 | 0.0% (3) |
| intent.match.v1 | jev / direct | jev-1.13.0: 0.031 | — |
| intent.match.v1 | jev / enriched | jev-1.13.0 (vision gemini-3.5-flash): 0.088; jev-1.13.0 (vision gemini-3.8-flash): 0.280; jev-1.13.0 (vision gemini-3.6-flash): 0.153; jev-1.13.0 (vision gemini-3.5-flash): 0.035 | 0.0% (3) |
| intent.match.v1 | jev / enriched_without_priors | jev-1.13.0 (vision gemini-3.5-flash): 0.102; jev-1.13.0 (vision gemini-3.8-flash): 0.285; jev-1.13.0 (vision gemini-3.6-flash): 0.157; jev-1.13.0 (vision gemini-3.5-flash): 0.040 | 0.0% (3) |
| intent.match.v1 | gemini / gemini_alone | — | 0.0% (3) |

### Qualification gates (§9, applied as written)

| Question | Truth-known held-out cases | More needed for 50 |
|---|---:|---:|
| triage.route.v1 | 20 | 30 |
| vision.route.v1 | 20 | 30 |
| perf.interpret.v1 | 2 | 48 |
| capture.disposition.v1 | 20 | 30 |
| intent.match.v1 | 20 | 30 |

No question qualifies: every question is below the held-out gate, important-miss tolerance is undeclared, no calibration identity is qualified, and provider rows below 95% attempted completion also fail that gate. Gates are not lowered.
