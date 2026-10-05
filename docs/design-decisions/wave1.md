# Wave 1 decisions

## Item 0: shared evidence contracts

Read FEATURE-RESEARCH-2026-10-05 sections 2, 5, 6 and build order, and
ASTRA-VISION-2026-10-05 candidate 22 before implementation. This lane follows
the lane's explicit order: contracts, exclusion audit, onset, geometry, motion.

Add `evidence::analysis` alongside existing evidence contracts. Availability has
explicit available, unknown, unsupported, excluded and rejected states. Historical
absence defaults to unknown, never available. Measurements carry implementation
revision, package version, code licence, resource identities and execution settings.
Missing resource hashes and licences remain null. No model or runtime download is
introduced. Existing schema identifiers and exit authorities remain unchanged;
new report fields will be optional. Unknown fields remain reader errors, preserving
the existing version-skew handling required by DESIGN-1.0 section 17.

No new dependencies. These records provide provenance, not reproducibility proof
across arbitrary hardware, nor capture validity or approval authority.

## Item 1: exclusion audit

Keep the pre-mask FLIP map authoritative for excluded-error accounting. Record
resolved mask runs and their byte hash; overlap counts once. Evaluate a copy of
the region policy without masks for the counterfactual, leaving the original
entry and verdict intact. Exact identity ignores thresholds in both paths.
Store capture-selection exclusions before filtering names. Absent expected-capture
manifests mean coverage beyond supplied names is unknown. Buffer/HDR/SDR scope is
stated explicitly, and historical reports do not manufacture audit evidence.
Expose JSON, portable HTML/Markdown and `inspect exclusions`. No new dependencies.

## Item 2: performance onset

Use the existing content-addressed history objects and verify their hashes on
read. Add the producer's existing comparison identity to PerfDiff; it includes
the frame statistic, which cannot be reconstructed from historical PerfDiff
records. Historical absence stays excluded rather than guessing p50. Read
capture-side timings only, deduplicate sample-window hashes, and partition by
comparison identity. No separate database or dependency is introduced.

Implement unpruned exact dynamic programming, log-latency L1 segment cost,
minimum segment 5 and seed penalty 3 ln(n). Inflate the final penalty by positive
lag-one pilot-residual correlation, using `(1+rho)/(1-rho)` capped at n/5.
Capture timestamps order complete partitions; otherwise preserve the store's
append sequence, including ties. Never use report creation time or hashes.
Normalize once using the maximum of robust
adjacent log differences, qualified repeat range and a documented numerical
floor. Bound the window to 120 (default 60) for predictable exact computation.
Require five individual post-segment observations beyond the materiality floor,
so a suffix shorter than five cannot masquerade as a sustained segment by
absorbing pre-change observations. Preserve all numerical witnesses and gaps.

Candidates never become confirmed on counts alone. Synthetic step/noise/spike,
materiality, identity and exhaustive-objective tests establish mechanics only;
field false-alert calibration and fresh independent repeats remain residuals.

## Item 3: geometry

`experiment geometry` is the measurement front door: finite samples cannot prove
continuous Hausdorff bounds. `prove mesh-identity` makes the narrower exact claim
about ordered world vertices, oriented triangle indices and declared units.
Appearance attributes remain unsupported in that proof; input document/buffer
hashes are retained separately so UV-only edits remain visible as source changes.
No renderer acceptance or asset-wide texture/material equality is inferred.

Use pinned parry3d-f64 0.21.1 (Apache-2.0), tobj 4.0.3 with use_f64 (MIT),
gltf 1.4.1 (MIT OR Apache-2.0), and existing base64 0.22 (MIT OR Apache-2.0).
The lockfile and geometry-dependencies.md record transitive additions. Pin Parry
rather than taking the newest major so the workspace Rust 1.88 contract remains
applicable. glTF convenience transforms are f32 in the inspected loader source;
read transform JSON as f64 to preserve large-world translation precision.

Use Parry surface queries with solid=false, deterministic per-triangle area
strata and mandatory vertex/edge probes. Area weights are reduced sequentially;
probe oversampling affects only maxima. Edge/vertex normal correspondence stays
unknown; face-normal orientation is preserved. Static triangle-only input rejects
unsupported animation, skins, morphs and required extensions. No automatic ICP,
unit conversion or polygon triangulation. Residuals: adaptive certified bounds,
attribute-level comparison, overlapping-surface tie resolution and renderer quality.

## Item 4: motion-aware diagnostics

The code already had Hann-window RustFFT phase correlation, phase-slope subpixel
refinement, inverse bicubic resampling and diagnostic FLIP reruns. Extend that
implementation instead of introducing a competing estimator. Preserve raw FLIP
and every configured verdict. Add per-hotspot original-mask evidence, peak
uniqueness, two-dimensional texture, reverse-shift consistency, recorded
thresholds, valid-border masks and shared input/provenance manifests. Both mean
and peak aligned error participate: a synthetic shifted image with a small
appearance defect showed that mean-only classification could wrongly say moved.

Opaque SDR is the qualified initial scope. HDR/transparency, local deformation
and interior disocclusion remain unknown/unsupported rather than being inferred
from a small aligned residual. Global forward/backward consistency is only one
check; the global model cannot verify independent local object motion.
Constructed translations cover both signs at 0.125, 0.25, 0.5, 1 and 2 pixels;
passing these fixtures does not qualify arbitrary TAA or rendered scenes.

DIS assessment (2026-10-05): the [standalone author implementation](https://github.com/tikroeger/OF_DIS)
is GPL-3.0 and is excluded. The bounded search found Rust Lucas-Kanade and
Farneback implementations, not a suitable pure-Rust DIS implementation.
[OpenCV's DIS](https://github.com/opencv/opencv/blob/4.x/modules/video/src/dis_flow.cpp)
and its [Rust binding](https://github.com/twistedfall/opencv-rust/blob/master/INSTALL.md)
require a native OpenCV build. `pkg-config --modversion opencv4` exits 1 here
(package not found), and the repository release workflows contain no OpenCV
installation/packaging path. A clean supported-platform build was not established.
Therefore ship phase correlation and record DIS as a residual under the lane's
explicit fallback; do not import GPL code or silently add an unqualified native
build dependency. RustFFT 6.4.1 (MIT OR Apache-2.0) was already present. No new
motion dependencies or model weights.

Final integration found that the existing notice generator used default-feature
metadata and omitted all three optional geometry crates. Request all features
when generating release notices so the shipped all-features binary has its
complete dependency inventory and upstream licence texts. The default metadata
probe contained none of parry3d-f64/tobj/gltf; the corrected inventory includes all.
