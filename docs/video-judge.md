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

Pricing uses the inherited `openrouter-price-allowlist/2026-10-07-v1` source pin
([models API](https://openrouter.ai/api/v1/models), supplied 2026-10-07): 750
nanodollars per prompt/image token and 3,750 per completion token for the admitted
model. These are **frame-image prices**, not an invented native-video price.
Each reservation is `input_bound × 750 + 1024 × 3750` integer nanodollars.
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
