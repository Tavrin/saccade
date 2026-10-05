# Deterministic analysis evidence

Optional analysis records distinguish checks that ran from unknown, unsupported,
excluded and rejected checks. An omitted field in an older report means that its
producer did not record the evidence. It does not mean the check passed.

Analysis provenance records the implementation revision, tool version, code
licence, resource SHA-256 identities and execution settings. A null resource hash
or licence means unavailable information. Code licensing does not establish a
model weight licence. These records do not change comparison or approval policy.

## Exclusion audit

Every new `compare` report records `exclusion_audit`; each measured colour pair
also records `pixel_exclusions`. Use `saccade inspect exclusions REPORT --json`
for the recorded audit and resolved mask runs, or omit `--json` for text. The
portable HTML report has an Exclusion audit section.

The audit lists selected and ignored capture scope, supplied names excluded by
policy, incomplete pairs, metadata gaps and ignored differences, performance
qualification, region thresholds and headroom, and decoder channel/precision
losses. This includes discarded HDR alpha, 16-bit to 8-bit SDR reduction, and HDR
to SDR display previews (the latter do not replace HDR-FLIP). Nondeciding regions,
including fully masked thresholded regions, appear in JSON, text and HTML.
Sequence reports retain this audit and all supplied names excluded by their
pattern or ignore policy. Rejected repeat calibration stays rejected. It cannot
list captures that were never supplied and were not declared in an expected list.

Excluded pixel error comes from the original FLIP map. Mask SHA-256 hashes bind
the resolved bitmap, including resized image masks. `without_masks` recomputes
the diagnostic entry/region/hotspot decision with masks removed. Capture validity
is separate. The configured result and exit code are unchanged. A negative
headroom exceeds a threshold; hotspot failure also includes equality. Historical
reports without audit fields show unavailable evidence, not zero exclusions.

## Performance onset candidates

Record comparison reports with `saccade history record REPORT --store HISTORY`.
Then run `saccade history onset --store HISTORY --json`. The default window is
the most recent 60 distinct qualified observations per comparison identity;
`--limit` accepts 10–120. Use one independently measured nightly statistic per
record. Correlated frames from one capture are not independent nights.

The detector reads and verifies the content-addressed report objects. It requires
qualified timing and repeat noise, a recorded comparison identity, a positive
latency, and a distinct sample window. Hardware, driver, timer, configuration,
statistic, aggregation and qualification changes create separate partitions.
Older reports without the new identity field are excluded. Capture
timestamps (numeric Unix time in `capture.timestamp`) order a complete partition.
Otherwise the history store's append sequence orders the measurements. Equal
capture timestamps retain append order. Report creation times and hashes never
order observations, and missing measurements are not interpolated.

Exact dynamic programming minimizes absolute deviation from segment medians of
log timings, normalized once by robust adjacent differences and measured repeat
noise. The minimum segment is five observations. A pilot segmentation estimates
positive lag-one correlation `rho` from within-segment residuals, avoiding
correlation across planted steps. The penalty is `3 ln(n)` multiplied by
`(1+rho)/(1-rho)`, bounded between one and `n/5`, to account conservatively for
serial noise. The output records the correlation and inflation factor.
Candidates must exceed the recorded absolute, relative, repeat-noise and timer
resolution bounds, with at least five observations above the preceding level.
The output contains run/commit endpoints, effect estimates, segment boundaries,
input identities, scale, penalty and objective. Ten pre-change and five
post-change observations meet only the retrospective count recommendation.

Every onset remains a candidate. Fresh qualified repeats are required, and
intervening commits or missing nights make the interval ambiguous. The penalty
is a calibration seed: synthetic tests do not establish a 1% field false-alert
rate. Variance shifts, periodicity and general gradual drift are not qualified.

## Static mesh geometry

Build with `cargo build -p saccade --features geometry`. Geometry is optional and
is not in the default feature set.

```
saccade experiment geometry original.obj candidate.gltf --unit m --samples 4096 --json
saccade prove mesh-identity original.obj candidate.obj --unit m --json
```

Inputs are static triangle OBJ, glTF or GLB. Declare their common coordinate
unit with `--unit`; Saccade does not convert units or align the meshes. glTF node
transforms are applied in f64. glTF POSITION samples are natively f32 and are
promoted to f64. OBJ positions are read as f64. The default glTF scene is used,
or its sole scene when unambiguous. Triangle-only OBJ avoids implicitly choosing
an ambiguous polygon triangulation. Degenerate/nonfinite geometry, animation,
skinning, morph targets and required glTF extensions are rejected.

`experiment geometry` reports directed and symmetric sampled Hausdorff, mean and
RMS distances, directed p95/p99, and oriented geometric face-normal deviations.
It exits zero when measurement completes, regardless of distance. Normal flips
remain visible; correspondence on edges or vertices is reported as ambiguous.
Every triangle receives area samples, including tiny components. Vertex and edge
midpoint probes improve maximum-distance coverage without biasing mean/RMS.
The requested area-sample budget is approximate: actual samples per direction
are at most the budget plus the triangle count. The sampler has no random stream,
and records its version, sample counts and sequential reduction order.

Sampled Hausdorff is a lower estimate subject to floating-point error. It is not
a certified upper bound and can miss defects between probes. No adaptive bound
or domain-specific pass tolerance is supplied.

`prove mesh-identity` exits zero only for exact ordered f64 world vertices and
u32 oriented triangle indices under the unit declaration. Different orderings
or triangulations can have zero sampled distance but fail this identity proof.
Document/buffer hashes separately identify input bytes; a UV-only change can
change those hashes while geometric identity remains true. Materials, UVs,
shading normals, textures and renderer appearance are outside the geometric
proof. OBJ MTL and glTF image resources are not loaded.

Each resource is limited to 64 MiB; glTF document and buffers share a 64 MiB
aggregate limit. Meshes are limited to one million vertices and triangles.
Buffers may be embedded or local relative files. Parent paths, URL escapes,
remote URLs and symlinks escaping the mesh directory are rejected.

## Motion diagnostics

Ordinary comparisons with diagnostics enabled include `diagnostics.motion`.
The portable report's Motion diagnostics section shows the same evidence.
The existing `[diagnostics] shift_detection = false` setting excludes this work;
disabling diagnostics omits it entirely. Missing historical fields mean unknown.

A global phase-correlation estimate comes first. Qualification requires sufficient
two-dimensional texture, phase coherence, a distinct correlation peak, and
forward/backward consistency. Per-hotspot labels are `stable`, `moved`, `changed`,
`moved_and_changed`, or `unknown`, conditional on that one global translation.
They do not establish independent-object flow or the cause of a change.

The record includes raw and aligned FLIP, displacement, phase coherence, peak
ratio, forward/backward error, exact excluded border runs and per-hotspot valid
coverage. Alignment uses one encoded-RGBA8 Catmull-Rom warp. Its valid interior
excludes interpolation support plus the pinned FLIP filter radius. Hotspots with
insufficient texture or valid coverage remain unknown. Both residual mean and
peak matter, so a small defect cannot disappear into a large moving hotspot's
mean. Displacements below `shift_min_px` remain in the estimate but do not count
as appreciable movement. Only the configured original hotspots receive labels.

The original unaligned FLIP map, thresholds, entry verdict and exit code remain
authoritative. Moving content may itself be a regression. These labels support
opaque SDR pairs; HDR and transparency are not supported. Dense DIS flow,
independent motion, interior disocclusion and field TAA qualification are not
covered yet. No OpenCV or learned model is added.
