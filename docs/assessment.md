# No-reference quality assessment

```sh
saccade assess image.png --out assessment --json
saccade assess after.jpg --compare-to before.jpg --out assessment --json
```

`saccade-assess.v1.json` and HTML report each measure and its meaning:

| Measure | Interpretation and limit |
| --- | --- |
| Laplacian variance | Four-neighbour luminance Laplacian variance; lower can indicate blur or flat content. |
| Edge width | Local edge contrast / maximum central slope in pixels; larger can indicate broader edges. No qualifying edge gives null. |
| Noise sigma | MAD high-pass estimate over low-gradient neighbourhoods, in 8-bit luma units; texture and smooth signal can contribute. |
| Block boundary excess | Mean adjacent luma difference at 8-pixel boundaries minus interior difference, clipped to zero; structure can mimic JPEG blocks. |
| Occupied luma fraction | Occupied 8-bit bins / 256; few bins can indicate posterisation or intended flat content. |
| Shallow plateau steps | Shallow 3..24 luma jumps between plateaus; a banding candidate, not a banding verdict. |
| Black/white clipping | Fraction of all-black/all-white pixels, plus per-channel samples at 0/255. Intended black or white content also counts. |
| Transparency | Fraction of alpha below 255; all measures composite over white and can therefore change with alpha. |

Thresholds are content-dependent. The command reports `verdict: unknown`; exit
0 means measurement executed, not that an image is good enough. Invalid inputs
exit 2. The optional learned score is explicitly unavailable: no reviewed
permissive model/export and pinned artifact was supplied. No quality model or
native dependency is added to the default build.

With `--compare-to`, deltas are current image minus reference for each measure;
null remains null when either observation is unavailable. Resolution changes
and intended content changes affect interpretation. Noise storage samples at
most about 262144 values and edge width about 16384 candidates. Tiny features
can be missed. The shared 64 MiB/16M-pixel, 8-bit SDR bounds apply; rasters must
be at least 3x3. HDR uses the existing paired HDR-FLIP pipeline.

JSON stdout is a bounded artifact receipt. MCP `saccade_general` / `assess`
mirrors `image`, optional `compare_to`, and `out`, with local root containment.
Generated blur, noise, quantised ramp and clipping tests are focused unit tests;
CLI paired delta/schema tests are opt-in.
