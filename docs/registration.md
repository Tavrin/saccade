# Registration

Explicit registration changes the question to similarity over overlapping geometry.
Default `compare` continues to measure unregistered differences.

```sh
saccade compare reference.png capture.png --align similarity --resample reference --out registered --json
saccade compare references/ captures/ --align homography --resample common --out registered --json
```

`--align none|translation|similarity|affine|homography|auto` selects the geometric
model. `none` only resamples across declared resolutions; `auto` tries translation,
similarity, affine, then homography and takes the least flexible successful model.
The translation mode uses feature consensus; the existing phase-correlation
diagnostic still runs in ordinary unregistered comparisons. `reference` retains
reference dimensions; `common` uses the smaller input pixel count while retaining
reference aspect ratio. Both policies use bilinear capture warping and triangle
reference resizing, with premultiplied alpha interpolation.

The pure Rust detector uses FAST-9 with oriented BRIEF-256 and five pyramid
levels, at most 1200 features per image. Matches must be reciprocal and pass
Hamming ratio <0.8 and distance <=80. RANSAC uses a fixed seed and 768 trials,
then least-squares refitting. A fit needs six inliers (eight for homography),
at least 25% consensus and RMS residual <=3 reference pixels. These are
engineering bounds, not a universal registration success guarantee. Blank,
repetitive or unrelated images may fail with `insufficient_inliers`.

The report is `saccade-registration.v1.json`, with input byte hashes, original
and output dimensions, model matrix, inlier count/ratio and residual. Matrix
direction is reference to original capture pixels. Non-overlap is explicitly
excluded by geometry, hatched in the heatmap and black in
`geometry-inclusion.png`; white means included. The HTML shows these artifacts.
FLIP is measured on overlapping pixels only. Spatial filter neighbourhoods at
coverage boundaries depend on border interpolation. This is not native identity,
and successful registration does not approve a baseline.

This initial explicit route accepts threshold/metric/resample options only. It
rejects config, intent, metadata, performance, selection and JUnit options rather
than silently omitting those contracts. Both inputs must be files or directories.
Directories pair by relative path; new, missing, unreadable and unregistrable
entries fail. Exit 0 means every supplied pair meets its threshold, 1 means a
failed/incomplete comparison, 2 means invalid arguments or unavailable inputs.
The JSON stdout envelope is `saccade-general-result.v1`, with report path and
counts; full measurements are in the report. Inputs are bounded at 64 MiB and
16 million pixels, 8-bit SDR only; use the existing HDR pipeline for HDR.

MCP `saccade_general` operation `registered_compare` mirrors the explicit route:
`reference`, `capture`, `out`, `align`, optional `resample`, `threshold`, `metric`.
Root/out-root authorization applies, and no images are returned by default.
