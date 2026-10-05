# Dense motion and renderer vectors

Build with `--features dense-motion`, then run:

```sh
saccade review motion reference.png candidate.png --out motion.json --json
saccade review motion reference.png candidate.png --vectors vectors.json \
  --sidecar convention.json --out motion.json --json
```

This bounded CPU diagnostic uses a native DIS inverse-search implementation.
It mean-normalizes 8×8 patches, searches with a fixed reference Hessian across
up to four pyramid levels, and aggregates overlapping patches with residual
weights. Two spatial passes and bounded local initialization improve convergence.
The version and all settings are in the packet. It omits variational refinement,
as in the paper's faster variant, and has no upstream implementation parity or
throughput qualification. No OpenCV runtime, weights or new dependency is used.
The independent Rust code uses the project's MIT OR Apache-2.0 licence.
[Method reference](https://arxiv.org/abs/1603.03590).

Inputs must be opaque, encoded 8-bit SDR captures of equal dimensions, each
axis 16..1024 pixels. No implicit resizing, HDR tone mapping or alpha flattening
is performed. A reduced build returns `feature_unavailable`; it can still read
all motion packet and sidecar types.

The `saccade-vector-buffer.v1` JSON contains `vectors` as row-major `[x,y]`
floats and a matching `valid` boolean array. Rows always run top to bottom.
The `saccade-motion-vectors.v1` sidecar declares:

- Exact lowercase SHA256 hashes of both encoded images and the encoded buffer
  JSON, plus shared `[width,height]` dimensions.
- `units`: `pixels`, `uv` (scale by width/height), or `ndc` (half width/height).
  Values are displacements, not absolute coordinates or velocities.
- `origin`: `top_left` means Y down; `bottom_left` means Y up. This affects
  components, not the row order of the buffer.
- `direction`: `reference_to_candidate` uses the reference grid;
  `candidate_to_reference` uses the candidate grid and the separately measured
  reverse field. Negating the forward field on the wrong grid is not equivalent.
- `reference_jitter_px` and `candidate_jitter_px`, always in top-left pixel
  units, and `includes_jitter`. Subtract candidate-minus-reference jitter from
  forward screenshot flow (the opposite difference from reverse flow).
  Subtract the same difference from supplied vectors only if they include it.
- Positive `frame_interval_ms` and a nonempty `producer` identity object.
  Dynamic resolution, packed GPU formats and unit inference are unsupported.

Each field contains vectors and a per-pixel state. Flat, aperture-limited or
locally repeated texture, image boundaries, residual appearance changes,
motion boundaries and forward/backward mismatch are excluded from renderer
statistics. The mismatch state is `possible_occlusion_or_mismatch`, not proof
of geometric occlusion. These fixed heuristic checks are not calibrated
confidence; large motion and repeated patterns can still fool them. Renderer
validity and jitter are producer assertions.

Endpoint mean, nearest-rank p95 and maximum are in unjittered original-image
pixels, only where both measured and supplied vectors are valid. Angular error
excludes either vector of length <=0.1 pixels. Support counts, the compared mask
and whole-grid coverage are retained; no support produces null statistics.
Transparency, reflections, particles, animated shading and missing skinning
can cause disagreements which do not identify a unique cause.

The packet's raw unaligned FLIP is authoritative. `--maximum-raw-mean` defaults
to 0.01 and controls exit 0/1, regardless of flow or renderer disagreement.
Exit 0 means that raw threshold passed; it does not validate renderer vectors
or establish visibility in unknown regions. No aligned score replaces raw
pixels. Synthetic translations, independently moving texture, occlusion,
appearance edits and convention errors qualify the constructed cases only;
no live renderer capture or public flow benchmark was run.
