# Advisory video judge

Requires `assist` and `--experimental`. `saccade assist video-judge` prepares a
credential-free OpenRouter schedule from one or two PNG frame maps. It never
contacts a provider. Use an **absolute** rubric JSON path, ten score anchors
(indexed 1..10), and a closed list of user-owned cues. The example
[rubric](../examples/rubrics/motion-naturalness.json) asks how natural the motion
of the material looks. Scores are advisory, never baseline approval or metric overrides.

```sh
python3 scripts/assist/video_corpus.py --out "$PWD/procedural-video"
saccade assist video-judge --experimental --json \
  --rubric "$PWD/examples/rubrics/motion-naturalness.json" \
  --frame-map procedural-video/continuous/frame-map.json procedural-video/displaced/frame-map.json \
  --fps 1 --max-edge 128 \
  --model google/gemini-3.8-flash --revision google/gemini-3.8-flash-20260902 \
  --max-spend-usd 2 --out video-plan
```

Output is a fresh directory containing `requests.json` (the existing smoke runner
row dialect) and `plan.json` (`saccade-video-judge-plan.v1`). An A/B pair produces
**both presentation orders** for every model/revision arm. Repeat `--model` and
`--revision` for additional arms. Models without pinned prices are refused; the
inherited allowlist currently admits only the named model. No multi-model or
video-capability qualification is claimed. Root IDs identify arms and orders;
slot A/B always follows presentation order. Source-map hashes identify the
underlying clips independently of slot.

The [frame-map contract](frame-map.md) governs relative paths, monotonic source
indices and timestamps. PNG bytes are bounded and hashes checked when present.
`--fps` is a maximum selection rate: retain the first frame then select the next
frame at least `1/fps` seconds later. No interpolation, uniform-time assumption,
or fabricated timestamps. `--max-edge` preserves aspect ratio and only downsizes.
The whole source map, selected timestamps, fps, sent dimensions, encoded media
hashes, rubric and presentation order participate in request identity. Native
MP4 input is unavailable: the repo has no sourced video-specific token rule,
calibration receipt or video price pin. Extract MP4 externally to PNGs plus an
accurate frame map. Do not relabel the per-image policy as a native-video bound.

Frame reservation uses `assist-image-ceilings/2`: 3,086 prompt tokens per rounded
524,288-pixel block (2 × the largest supplied image prompt count), without a
low-detail discount, plus one token per serialized non-media UTF-8 byte and 1,024
framing tokens. Unknown resolutions retain the maximum bound. At most 32 frames
and 128,000 prompt tokens are admitted for this new workload; image requests
retain their 16,000-token cap. Output is capped at 1,024 aggregate tokens,
including the 256-token reasoning hint. Oversized rubrics/long clips must be
sampled more sparsely or split into independent clips; the plan never silently
truncates frames to fit. Sparse sampling limits semantic evidence: abstain when
it cannot support a score.

Pricing uses the `openrouter-price-allowlist/2026-10-07-v2` source pin
([models API](https://openrouter.ai/api/v1/models), supplied 2026-10-07): 750
nanodollars per prompt/image token and 3,750 per completion token for the admitted
Gemini arm. The independent GPT arm pins 4,500 per completion token. These are **frame-image prices**, not an invented native-video price.
Each reservation is `input_bound × 750 + 1024 × arm_completion_price` integer nanodollars.
`plan.json` lists exact per-request and aggregate reservations; the whole
schedule must fit a positive cap ≤ $2 before any output plan is written. This
is an exact reservation estimate, not a prediction of billed usage.

The closed answer has `request_hash`, `outcome` (`scored` or `abstain`), per-slot
`scores` (1..10, or null for abstention), all rubric cues in rubric order, and
`preferred` (`A`, `B`, `tie`, `single`, `abstain`). Cue state is `present`, `absent`
or `unknown`; present/absent timestamps must cite actual sent frames of that
slot, strictly increasing; unknown cues have no timestamps. Absence means
absence in those sampled frames, not proof about unsent intervals. Whole-request
abstention retains every slot with null scores. Local validation preserves full
array bounds; provider projection removes only `minItems`/`maxItems`, retaining
closed objects, enums, required fields and numeric ranges. The strict wire name
is `saccade_video_judge_drop_array_bounds_v1`.

Live operator command (prepared only; implementation and acceptance are offline):

```sh
cargo run --locked -p saccade-core --features assist --example assist_openrouter_smoke -- \
  --stage2 --requests video-plan/requests.json --roots 2 \
  --max-spend-usd 2 --user-policy "$USER_POLICY" --out video-results
```

`USER_POLICY` is the operator's existing authorized policy file. Use the actual
plan row count for `--roots`. The existing Executor enforces provider-verified
ceilings, monetary reservations/receipts, revision pins, post-call usage/cost
breach stops, and reconciliation. Stage-2 retains billed malformed answers as
`invalid_answer` and applies its existing safety valve. Native transport,
identity, accounting and storage failures remain campaign-stopping. Reconcile
with `--reconcile-only video-results`; resume with the original command's exact
plan/policy/cap/safety flags and `--resume video-results` instead of `--out`.
Neither order agreement nor a successful fixture establishes semantic truth.

Calibration export:

```sh
python3 scripts/assist/video_scores.py --requests video-plan/requests.json \
  --results video-results --out video-scores.jsonl
```

`saccade-video-judge-scores.v1` JSONL records root, source-map SHA-256, anonymous
slot, request hash, requested and returned model/revision, routed provider, outcome, score (or null), cues,
`trusted_for: []`, and advisory authority. Preserve every scheduled slot;
missing/invalid/unrun results have null scores even when stale answer files
exist. The exporter requires the runner's frozen request-file identity. The
constructed-negative motion-statistics calibration lane can join by `source`
and consume `score`; null rows remain unavailable, never zero. That branch's
score-file adapter has not landed in this checkout, so this documented JSONL is
the integration point. It grants no trust until independent constructed-negative
calibration passes its declared thresholds.

The procedural generator is CC0 and uses fixed geometry and exact frame hashes.
The checked-in response envelope is an authored offline transport fixture, not
a paid provider response or model-quality evidence. Reference footage is never
copied into public artifacts.

MCP mirrors preparation as `saccade_review`, operation `video-judge`, with
`rubric`, `frame_map`, `model`, `revision`, `fps`, `max_edge`, `max_spend_usd`
(string), `out`, and `experimental: true`; normal filesystem roots apply.

Unified media and local gates:

- `--image PNG [PNG]` accepts direct still candidates, alongside `--frame-map`
  motion candidates (one or two candidates total). `--view-id` values follow
  frame-map then image order; omitted ids use source content identity. The kind
  and `sample-0` identity follow the source through both presentation orders.
- `--contact-sheet` first applies the fps filter, then selects up to eight eligible
  frames at indices `floor(i*(n-1)/7)` when `n > 8`. The first and last eligible
  frames are retained. A four-column, two-row PNG labels each cell with its
  original timestamp, rounded to three decimals for display; the packet retains
  exact times and source indices. It requires `--max-edge >= 128`, preserves
  aspect ratios, and binds both selected-frame hashes and the sent sheet hash.
- `--reference PNG` or `--reference-frame-map MAP` adds one reference after the
  candidates, outside A/B slots. Reference frames count toward admission bounds
  and never receive candidate scores. Both options are exclusive.

Rubrics may additionally contain `criteria`, each with `id`, integer `minimum`
and `maximum` (0..100), and one `anchors` string for each integer on that scale.
They may contain a closed `forbidden` string list. The answer then requires
criterion scores in configured order, numeric within that criterion's scale
(or null on abstention), plus every forbidden condition with
`present`/`absent`/`unknown` state. No scale conversion is inferred. Scalar scores
remain independently anchored 1..10. The exporter retains both extra arrays,
source-remapped `preferred_source`, kind, view/sample identity and replay status.

```sh
python3 scripts/assist/judge_gate.py --scores video-scores.jsonl \
  --config judge-gate.json --out judge-report.json --markdown judge-report.md --strict
python3 scripts/assist/judge_gate.py --self-test
```

The closed gate config contains `providers` (each with `provider`, `model`,
`revision`), `evaluations` (each with `source`, `view_id`, `kind`, `sample_ids`,
`order_count` of 1 or 2), `threshold`, `required_cues`, `criteria`, `forbidden`,
and `scorecard`. Each gate criterion has `id`, `minimum`, `maximum`, `threshold`.
`scorecard` has `floor`, `mean` (numeric or null) and `conjunction` (boolean).
An example config is [judge-gate.json](../examples/rubrics/judge-gate.json);
replace its explicit identity placeholders with the planned source and returned
provider/model/revision identity. Use `revision: "absent"` only when that is the
actual admitted, returned revision contract.

The denominator is exactly the declared fresh sample ids for each required
provider/model/revision, view and kind. Both orders form one sample: map the
preference to source identity, require agreement, average its two numeric scores,
then take the median across declared samples (even counts average the middle two).
One-order evaluations use their sole score. Each group reports `consistent`,
`inconsistent`, or `incomplete` samples. Missing, invalid, abstained, replayed,
duplicate, additional, or identity-mismatched evidence prevents pass. All required
provider/view/kind medians must meet the threshold AND every required cue must be
present in every order/sample AND every forbidden condition must be absent.
Criterion floor, mean and configured per-criterion conjunction apply to each
order separately. Floor/mean require a common numeric scale; the reducer refuses
implicit normalization across scales. `--strict` exits 1 on an advisory gate
failure, 0 on pass. JSON and optional Markdown keep empty `trusted_for`.

Generic deterministic image-quality measurements:

```sh
python3 scripts/assist/image_quality.py candidate.png --config quality.json \
  --baseline baseline.png --mask candidate-mask.png --baseline-mask baseline-mask.png \
  --out quality-report.json --strict
```

Pillow and numpy are required. Config keys are `absolute` and `relative` maps of
metric name to `[">=", threshold]` or `["<=", threshold]`, optional `palette`
(list of `#rrggbb`), `delta_e` (default 20), and `exposure_band` (`[lo,hi]`).
An empty object uses the declared default gates; overrides replace thresholds,
never silently remove missing configured measurements. Nonzero mask pixels
exclude clipping only, with masks required to match image dimensions.

Absolute metrics use a grid with steps `max(1,width//160)` and
`max(1,height//90)`, floor-sized rows/columns; Rec.709 luma on 8-bit sRGB;
sorted p5/p95 at floor indices; Shannon entropy and dominant share of 4-bit RGB
buckets; and edge cells whose right/down maximum luma step exceeds 12.
Clipping is measured over every pixel with any channel >=254, divided by the
unmasked pixel count (at least one). Palette coverage uses every pixel with
luma >10 within CIE76 distance of a configured palette entry, excluding black.
Exposure gates the sampled median luma. Metrics retain the declared decimal
rounding before absolute and relative predicates. Defaults: contrast >=60,
entropy >=3, dominant share <=0.6, edges >=0.04, clipping <=0.02,
palette coverage >=0.70 when configured.

The eight relative predicates are edge ratio >=0.5, contrast ratio >=0.7,
median luma ratio >=0.5 AND <=2, absolute entropy delta <=1.5,
dominant-share delta <=0.10, clipping-share delta <=0.02, and mean-colour
CIE76 distance <=10. Ratios use baseline denominators at least 1e-9 (edge and
contrast) or 1 (median luma). Mean colour is Lab of the full-image mean sRGB
triplet, rather than mean Lab. Relative comparisons precede display rounding.
Reports list new absolute failures relative to the baseline, relative failures
and `flagged`; pass requires absolute pass AND every relative predicate.
These are image measurements, never model calibration or baseline approval.

Independent priced model admission and fresh-repeat scheduling remain separate
integration points: the planner still schedules one sample per arm/order, refuses
unpriced models and never counts resume/replay as a new sample. Future schedulers
must bind new sample ids into the packet and reserve the entire schedule.
Independent calibration must bind returned provider/model/revision, rubric,
transform/sheet selector and kind to constructed-negative evidence before
assigning trust. The mandatory offline scorer proof now exercises this reducer
and the deterministic quality gates with oracle-perfect and oracle-wrong cases;
it is offline implementation proof, not empirical calibration.

MCP preparation also accepts `image`, `reference`, `reference_frame_map`,
`view_id` and `contact_sheet`, with the same filesystem authorization as frame maps.

Independent arms and repetitions are prepared offline with `--repeats N`
(1–32), repeated `--model` and matching `--revision` flags. Each item/arm/order
has a distinct request root; both orders share one sample identity, and each
repetition changes the packet and cache identity. Duplicate model arms are
refused. Resume/replay never creates another fresh median sample. The complete
schedule is reserved before publication; no partial denominator passes.

The independent allowlist admits `google/gemini-3.8-flash` and
`openai/gpt-5.4-mini`. The latter pins the supplied 2026-10-07 models API prices
at $0.75 per million prompt tokens and $4.50 per million completion tokens.
Image input uses the prompt rate because no separate image price was supplied.
Its separate provisional calibrated policy reserves 16,384 tokens per image,
regardless of dimensions or detail. This is a conservative local policy,
not measured tokenizer qualification. A returned usage/cost bound breach stops
the campaign. Gemini's measured image policy is not evidence for the second arm.
Receipts bind each arm's image table, price version and actual returned identity.

Provider schema projections have separate versioned wire names. Gemini retains
its array-cardinality keyword removal. The second arm defaults independently to
the same conservative projection: supported JSON-schema keywords remain unknown.
Full local cardinality, score, cue, identity and timestamp checks still apply.
No provider keyword-support or quality claim follows from offline fixtures.

Constructed calibration uses the existing deterministic degradation generator,
including freezing, flicker, speed changes, hitches, overlays, temporal blur,
drops and spatial controls. Original frame maps and images remain user-owned:

```sh
saccade experiment calibrate-degradations --positive POSITIVE_MAP \
  --strengths 1 --seed 42 --threshold 0.8 --out NEGATIVES --json
python3 scripts/assist/judge_plan.py --bin ABSOLUTE_BIN \
  --evidence NEGATIVES --positive POSITIVE_MAP --rubric ABSOLUTE_RUBRIC \
  --model google/gemini-3.8-flash --revision absent \
  --model openai/gpt-5.4-mini --revision absent --repeats 1 \
  --cap-nano-usd 2000000000 --out NEW_PLAN
python3 scripts/assist/judge_show.py --requests NEW_PLAN/requests.json \
  --out NEW_PLAN/hand-check.html
```

Repeat `--positive` in the original source order for multiple independent clips.
The plan script preserves all classes unless an explicit `--classes` subset is
requested for a smoke. It prepares both orders at 120 fps, then uniformly selects
up to eight frames for a 128-pixel timestamped contact sheet. This sparse
representation must be hand-checked for the intended defects before any spend.
The plan includes exact integer worst reservations and an explicitly hypothetical
expected-cost scenario: half the bounded input and 256 aggregate output tokens.
It never reads credentials or invokes a provider. Unaffordable complete plans
are refused, rather than silently dropping arms, classes or samples.

After separately authorized execution and score export, run:

```sh
python3 scripts/assist/judge_calibration.py --bin ABSOLUTE_BIN \
  --manifest NEGATIVES/manifest.json --schedule NEW_PLAN/schedule.json \
  --scores SCORES_JSONL --positive POSITIVE_MAP --threshold 0.8 --out NEW_RESULT
python3 scripts/assist/judge_gate.py --self-test
```

The adapter passes scalar medians to `experiment calibrate-degradations` as an
external scorer. It binds actual provider/model/revision, rubric, protocol,
evaluation kind and sampling transform; absent, mixed, replayed or inconsistent
answers withhold the entire external scorer. Cues and criteria remain independent
requirements of the advisory acceptance gate. Repetitions do not inflate the
calibrator's independent-source denominator. `trust.json` grants `trusted_for`
only the classes whose exact lower bound passes at every tested strength;
all other classes remain explicitly untrusted. It records calibration and score
file hashes. This trust applies to the recorded binding and constructed domain,
not a changed rubric, transform, kind, route or model, and never baseline approval.
The mandatory paid-run scorer proof includes perfect, near-perfect, wrong,
inconsistent-order, missing, replay and identity cases for the judge gate.
