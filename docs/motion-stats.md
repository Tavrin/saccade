# Motion statistics and constructed negatives

`experiment motion-stats` measures a candidate against a reference without a
fused quality score. It does not infer naturalness or human preference. Use it
for short moving textures, scrolling interfaces or other sampled animation.

```sh
saccade experiment motion-stats reference/frames.json candidate/frames.json \
  --out report.json --json
saccade experiment motion-stats reference/frames.json candidate/frames.json \
  --mask mask:region.png --out region-report.json --json
saccade experiment calibrate-degradations --positive reference/frames.json \
  --positive other-source/frames.json --strengths 0.25,0.5,1 \
  --seed 42 --threshold 0.8 --out calibration --json
```

Inputs use [frame maps](frame-map.md): `saccade-frame-map.v1`, increasing
source indices and presentation timestamps in seconds, relative frame paths,
optional encoded-file SHA256 pins. Video extraction remains external with
ffmpeg on PATH, as in the existing contract. A frame directory needs a map;
frame filenames alone cannot establish timing or detect hitches. Use original
presentation timestamps for variable-rate clips; a synthetic fixed-rate map
cannot establish source hitches. Explicit producer-side resizing must preserve
aspect ratio (including portrait inputs); no command implicitly resizes.
Limit: 4–128 frames, equal opaque 8-bit SDR dimensions of 16–256 pixels on each
axis. At 30 fps this covers approximately three seconds. These bounds keep CPU
and memory use finite; split longer clips into declared windows. Reference and
candidate need equal dimensions, but their durations and frame counts may differ.

A static binary L8 PNG inclusion mask uses the shared `mask:PATH[:MIN_PIXELS]`
grammar, white included. Both sides use the same mask, whose hash is retained.
Flow uses full-image context but only qualified included pixels contribute;
neighbour coherence also requires both neighbour pixels to be included. Masking
therefore does not make flow independent of surrounding image content.

Each side reports separate measurements:

- Per-pixel encoded luma, normalized to 0–1, with DC removed before a rectangular
  DFT; average power in [0,2), [2,6), and [6,Nyquist] Hz, in intensity squared.
  At most 4096 evenly spaced selected pixels contribute to spectral summaries.
  Bands above the sampled Nyquist contain no measurements. Nonuniform timestamps
  or missing indices make spectra unavailable, with a reason; no interpolation.
- Qualified native DIS flow magnitude in pixels/second with histogram edges
  0,15,30,60,120,240,480,infinity; eight direction bins over [-pi,pi] radians.
  Direction excludes vectors <=0.1 pixels/step. Flow spatial coherence is mean
  horizontal neighbour vector difference in pixels/second; smaller means more
  locally consistent. Coverage is always separate. Zero support stays unavailable.
  Enable `dense-motion` for flow; default builds retain temporal diagnostics.
- Mean adjacent luma change, mean squared second temporal difference (flicker),
  exact decoded pixel duplicate fraction and near-frozen fraction (mean adjacent
  luma change <=1/255), per sampled interval. Constant or low-contrast motion can
  produce frozen flags; decoded equality does not assert encoded file identity.
- Median presentation interval in seconds, interval coefficient of variation,
  hitch fraction (index-normalized step >1.5 times its median), and missing source-frame indices.
  Hitches are timestamp evidence, independent of optical flow. Index gaps alone
  do not count as hitches.

Scalar distances are absolute differences with the same units. Distribution
Jensen–Shannon distances use natural logarithms (nats); spectral L1 uses intensity
squared. Band ratios are candidate/reference power, ideal 1, omitted if reference
power is zero. Other distances have ideal 0. None has a universal pass threshold.

Calibration creates every class at each declared strength, with deterministic
LCG noise, seed, versioned transform identity, source hashes, generated frame
hashes and timestamp maps in `manifest.json`. Output must be a new directory. Calibration positives require at least eight
frames. Ineffective transforms (unchanged samples and timestamps) are refused
instead of being labelled known negatives.
`frozen` holds the final strength fraction; `flicker` alternates +/-64 times
strength channel levels; `speed_up/down` scale presentation times about the first sample by speed factors
1+strength and 1/(1+strength), without wrapping, dropping or duplicating samples;
`looped_hitch` holds 1–4 samples every eight frames and expands the release
interval by 1+4 times strength. `overlay_blobs` inserts a moving disk of radius
strength times the smaller dimension /4. `temporal_blur` blends the preceding
frame at weight strength/2. `frame_drops` removes every ceil(1/strength)+1 sample
while retaining source indices/timestamps. Spatial Gaussian blur (sigma 3 times
strength) and seeded noise (+/-64 times strength) are controls. These transforms
are constructed defects, not models of every real-world degradation.

Every built-in distance is its own scorer: higher is better, scores are negative
distances (ratios use -abs(ratio-1)). Built-in score magnitudes <=1e-12
in each distance's native units are numerical ties; raw measurements remain
unchanged. This prevents roundoff from qualifying a separator. External scores
retain their supplied values. The positive is compared with itself as a
known anchor, so this tests separation of constructed defects, not generalization
to unseen good motion. Results group independent source clips by class and
strength. Ties count half for pairwise accuracy, but as failures for trust.
Exact Clopper–Pearson two-sided 95% bounds cover strict-win probability, assuming
independent sources. Paired accuracy additionally has conservative exact 95%
bounds: the strict-win lower bound and the non-loss upper bound jointly cover
the half-tie accuracy (each offending tail is bounded by 2.5%). All ties therefore
give accuracy 0.5 with bounds [0,1], rather than a misleading narrow interval.
Variants/windows of one clip are not independent sources.
No repetitions inflate sample counts. `trusted_for` requires the lower bound to
meet `--threshold` at every evaluated strength of that class; no trust is claimed
at untested strengths. Unavailable metrics do not enter ranking. Small generated
proofs normally leave all scorers untrusted at the default threshold 0.8.

An external higher-is-better scorer can supply a `saccade-motion-scores.v1` file:
`manifest_sha256` binds exact generated evidence; `scores` contains objects with
`id`, `scorer`, `positive`, `negative`. Every external scorer must cover every
negative exactly once, with finite scores. Generate evidence first, score it,
then rerun with the same maps/parameters into a new output directory and pass
`--scores FILE`. No AI/provider calls occur. External scores are always labelled
untrusted outside the classes where their exact bound passes.

Run the generated moving-pattern and scrolling-interface proofs in bounded
stages through both commands (one corpus, no regenerated negatives):

```sh
scripts/qualify-motion-stats.py --bin PATH --out SCRATCH --phase generate
scripts/qualify-motion-stats.py --bin PATH --out SCRATCH --phase verify --domain texture
scripts/qualify-motion-stats.py --bin PATH --out SCRATCH --phase verify --domain scrolling
```

Verification checks manifest/map hashes and binds domain markers to the same
manifest and executable. Only both completed domains produce `acceptance.json`. The script
asserts frozen, flicker, hitch, and time-scaling sensitivity and records per-metric
class separation at strengths 0.5 and 1. It reports trust separately from point
separation. Time-scaling changes presentation intervals and qualified flow speed; broad spectral
bands can miss frequency changes that stay inside one band. Sensitivity depends
on content, sampling, exposure, flow coverage,
and transform strength; this is a constructed proof, not perceptual qualification.
MCP mirrors are a recorded follow-up, outside this lane.

The generated proof uses 90 frames at 30 fps, 64 by 48 pixels, in both domains.
At strengths 0.5 and 1 it requires duplicate fraction to rise by more than 0.2
for freezing, flicker second-difference energy to exceed 1.5 times the positive,
and a nonzero normalized timestamp hitch fraction with unavailable spectrum for
looped hitches. Time-scaling must change median interval by more than 1e-6 seconds;
where positive qualified flow speed is nonzero, its speed ratio must match the
constructed factor within 1e-6. These are regression sensitivity assertions,
not universal content or preference thresholds.
