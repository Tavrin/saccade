# Transitions and animation from external captures

Build with `graphics` (included by default). These commands consume images;
they do not render, load animated assets, or establish deformation correctness
from mesh data. Static [asset views](asset-lod-review.md) remain a separate check.
The same commands measure object detail transitions, progressive image loads,
zooming raster pyramids and other timestamped visual observations.

Write a `saccade-captured-sequence-plan.v1` JSON file. Every image path is
relative to the plan directory. Declare every expected frame explicitly;
missing files, unequal counts, unequal timestamps and duplicate/non-increasing
timestamps are errors, never omitted pairs. For example:

```json
{
  "schema": "saccade-captured-sequence-plan.v1",
  "candidate": [
    {"image": "candidate/frame_0.png", "timestamp_ms": 0},
    {"image": "candidate/frame_1.png", "timestamp_ms": 100},
    {"image": "candidate/frame_2.png", "timestamp_ms": 200},
    {"image": "candidate/frame_3.png", "timestamp_ms": 300},
    {"image": "candidate/frame_4.png", "timestamp_ms": 400}
  ],
  "reference": null,
  "masks": null,
  "id_buffers": null,
  "include_ids": [],
  "alignment": "none"
}
```

For animation or paired levels, supply `reference` with the same number of
records and exactly the same `timestamp_ms` values. Matching timestamps are
producer assertions: Saccade cannot verify capture clocks. This plain timestamp
contract does not depend on a frame-map implementation. A future frame-map
adapter can populate these lists without changing measurement semantics.

Each sequence has 3..512 opaque encoded 8-bit SDR images, equal dimensions,
16..1024 pixels per axis. Encoded files are bounded at 64 MiB and total decoded
capture pixels at 256 MiB. The plan is bounded at 1 MiB. Absolute paths,
parent components and symlink escapes are rejected. Input file SHA256s and the
plan hash are recorded in `sources` (`plan` and `input:<relative path>`
keys use distinct namespaces); the report retains the effective plan,
budgets, scope declarations and sampled measurements. No resampling in time,
colour conversion, exposure normalization or resizing is implicit.

## Declared switch or paired levels

```sh
saccade experiment transition plan.json --change-frame 2 \
  --maximum-pop 0.08 --maximum-duration-ms 400 --maximum-steady-error 0.01 \
  --settle-threshold 0.02 --consecutive 2 --window 2 --tile-size 16 \
  --out transition-report --json
```

This writes `saccade-transition.v1.json`. All frame indices are zero based.
The three budgets must be declared explicitly. Scores use normalized encoded
RGB mean absolute error (MAE), in 0..1; they are not FLIP or a perceptual
visibility guarantee. Duration budgets are in milliseconds, using actual
sample timestamps, including nonuniform intervals.

`motion_baseline[i]` is the reference's raw adjacent-frame MAE when supplied.
Without a reference it is the mean adjacent-frame MAE within the pre-switch
`window` observations (at least two). `pop_curve[i]` is the positive candidate adjacent-frame
MAE minus this baseline. Adjacent comparison uses the intersection of both
frames' declared scopes; an empty intersection is an error. Index zero has no
preceding edge. `popping` is the largest excess from the requested switch through
the first confirmed convergence interval, or the end when convergence fails.
`pop_frame` is its earliest maximizing index, null for zero excess.

For convergence, each scoped tile must be at or below `settle_threshold` for
`consecutive` frames. `settle_frame` is the first frame of that interval;
`transition_duration_ms` is its timestamp minus the request timestamp. A direct
switch to the target thus has sampled duration zero; a fade requested at index
3 and first converged at index 6 on a 100 ms timeline has duration 300 ms.
Unsettled sequences fail. This does not establish continuous-time duration or
stability after the recorded interval.

With a reference, it must depict the target level/content at every timestamp;
`steady_state_difference` is the final `window` frames' mean paired raw MAE,
and its budget is independently enforced. Without reference, the final
candidate frame is an inferred target. Steady level difference is the mean raw MAE between corresponding observations
in the pre-switch `window` and final `window`. `steady_state_basis` records
`pre_post_windows`; with a reference it records `matched_reference`. The steady
budget applies in both modes. Moving content or viewpoints can confound the inferred
target, pre-switch baseline and pre/post difference. Use an independent matched target for those
cases. Do not interpret final-window convergence as correctness of the target.

Omit `--change-frame` to compare two captured steady levels. This mode requires
a matched reference, measures steady-state difference and leaves popping and
duration null: the input contains no observed switch.

## Animation consistency

```sh
saccade experiment animation matched-plan.json \
  --maximum-frame-error 0.01 --maximum-local-error 0.08 --maximum-flicker 0.04 \
  --tile-size 16 --out animation-report --json
```

This writes `saccade-animation.v1.json`. `first_divergence_frame` is the first
raw whole-frame or local tile budget failure. Each `frames` row retains raw
error, worst tile error and above-budget `regions` with pixel rectangles.
These localized trajectories surface a deformation glitch without claiming
its cause. The local budget prevents a small glitch from disappearing in a
whole-frame average.

`alignment` is `none`, `translation` or `dense`. Translation reuses phase
correlation (confidence at least 0.6, displacement at most 64 pixels per axis).
Dense reuses the existing qualified dense correspondence and requires the
`dense-motion` feature. No mode relaxes the raw budgets. Nearest-sample aligned
scores and tiles are supplementary; `aligned_pixels` records supported scope
pixels, while unavailable correspondence produces null aligned error.
A raw passing sequence with incomplete alignment support fails to establish
consistency. Global translation cannot compensate independent local motion or
deformation; dense correspondence also has [qualification limits](dense-motion.md).

Temporal flicker is the maximum tile mean of the absolute change in **signed**
aligned RGB residuals between adjacent observations, divided by two to normalize
to 0..1. It catches equal-size errors alternating in sign as well as isolated
glitches. Residuals subtract the timestamp-matched reference, so its ordinary
motion is the baseline. Scope is the adjacent intersection; incomplete support
or an empty intersection makes `temporal_flicker` null and the run fails.
`first_flicker_frame` is the later index of the first over-budget pair.
Flicker is per observation interval, not a frequency-weighted human temporal
vision model; moving local errors are still measured on reference-grid tiles.

## Optional scope and results

`masks` may contain one relative binary L8 PNG path per timestamp: 255 includes,
0 excludes. `id_buffers` may similarly contain one L8 or L16 PNG per timestamp;
`include_ids` explicitly selects IDs. When both are present their intersection
is used. Scope is declared on the original reference grid, frozen for both
sides, and never regenerated from candidate content. Every frame must include
pixels. Empty ID selection, missing buffers, wrong dimensions, nonbinary masks
and unsupported formats are errors. Excluded pixels are outside the declared
claim; inspect the plan and included/aligned pixel counts before reporting a pass.

Exit 0 means the applicable declared budgets passed; 1 means regression or
unqualified alignment; 2 means invalid/unavailable inputs or features. `--json`
and the disk artifact contain the same complete versioned packet, with linked
report identity and exact source hashes. Outputs must be separate from inputs.
These commands have no MCP mirror.

Generated tests construct a detailed circular object and a progressive square
raster. Both use the same transition command: abrupt detail loss at frame 3
fails the popping budget; a four-step fade converges at frame 6 in 300 ms and
passes. Animation fixtures assert a localized one-frame glitch at frame 4 and
a correct candidate with zero error/flicker. These are constructed image proofs;
real capture acquisition, animated asset rendering and runtime performance are
not qualified by them.
