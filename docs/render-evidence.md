# Rendering evidence

These opt-in policies extend the ordinary measured pair report. They do not
qualify capture conditions or timing, and no pixel mask establishes causality.

## Required effects

`[[required_effect]]` in the comparison config declares a name, entry `glob`,
`min_pixels` (default 1), `min_fraction` (default 0), and `sides` (`both`,
`baseline`, or `candidate`). Both minima apply. Selection is an inclusion;
white/nonzero samples mean effect occupancy. A zero mask fails even at FLIP 0.

```toml
[[required_effect]]
name = "feature footprint"
glob = "frame.png"
min_pixels = 16
min_fraction = 0.001
sides = "both"
[required_effect.selection]
kind = "mask"
image = "effect.png"
```

Mask images resolve relative to the config and must match dimensions exactly.
`kind = "boxes"` accepts `boxes = [[x,y,width,height], ...]` in capture pixels.
`kind = "layer"` accepts `image = "ids.png"` relative to each capture image's
parent and a `predicate` with `kind = "ids", values = [1,2]`,
`kind = "above", threshold = 0.1`, `kind = "range", min = 0.0, max = 2.0`,
or `kind = "mask"`. Integer ID/mask samples retain native PNG values; scalar
floating layers use the R channel, without colour conversion.

Each entry records `required_effects` with its policy, counts/fractions on both
sides, `insufficient_effect_coverage` failures and mean/max FLIP plus changed
pixel counts inside the union of the two footprints and outside it. Empty
footprint metrics are null. These are full-frame FLIP samples; spatial filter
support may cross a footprint boundary.

Visual intent accepts the same policies in a top-level `required_effects` array.
They are measured by compare before intent verification, which also refuses
missing effects. Effect names must be unique. Mask paths resolve relative to
the intent file. Existing `changes` retain their existing meaning.

`localized-check BASE CANDIDATE --required-effect effect.json --out OUTPUT --json`
measures the same policy independently of its frozen-region mode. This mode
gates occupancy; it reports differences but imposes no implicit collateral or
minimum-change threshold. Freeze a region and use the ordinary localized mode
when exact collateral preservation is the claim.

## Intended experiment variables

`compare`/`prove identity` and experiment sequence/rank accept repeatable
`--intended-variable 'env.FEATURE_*'` (comma-separated patterns also accepted).
The config equivalent is `intended_variables = ["env.FEATURE_*", "binary.sha"]`.
This needs no metadata enforcement flag. Before/after values, including
unchanged or missing values, appear in `entry.intended_variables`; declared
patterns appear in `config.meta.intended`. Every other key retains the existing
validity rules. Required-key presence and explicit expected values still apply.
Missing metadata stays unknown or invalid; a declaration supplies no provenance.

Ablation accepts those global patterns and repeatable
`--arm-variable 'variant=env.FEATURE_A'`. Config can use
`[arm_variables]` with `variant = ["env.FEATURE_A"]`. Arm declarations only
apply to that label. Each arm records `intended_keys` and `intended_variables`
separately from `config_differs`. MCP compare uses `intended_variables` and
supports config declarations for experiments.

## Structure and texture

Add `[spatial]` to the compare config. `scales = [2,4,8]` runs FLIP on
area-downsampled pairs at 1/2, 1/4 and 1/8 as well as full resolution.
`ssimulacra2 = true` requires the compression feature; scores are absent for
transparent inputs or dimensions below 8x8. RGB MAD retains floating area
means; perceptual scales round these means to RGB8. All edge pixels contribute.

The default `version = "spatial-1"` records a 32-pixel tile grid, signed mean
normalized sRGB luminance shift and pixel CI, relative shift (denominator
floored at 0.02), before/after contrast, variance ratio, absolute Laplacian
fine-detail-energy ratio, optional background gap fractions, and mean full and
coarsest FLIP. Edge tiles remain in the grid. Zero baseline variance/energy with
nonzero candidate values yields a null ratio, never an artificial finite ratio.

Thresholds are all configurable and recorded in each entry's `spatial.policy`.
Defaults: absolute and relative shifts >0.02, CI multiplier 1.96, minimum
three connected significant tiles, coarse RGB MAD <=0.002, at most two
absolute outlier tiles, relative shifted-tile share <=0.25, detail ratio within
10%, coverage/gap tolerance 0.05, and coarse/full FLIP ratio <=0.3. The small
coarse-error allowance targets half an 8-bit code; sparse tiles alone cannot
hide broad relative shifts, detail loss or silhouette changes. These defaults
are generated-fixture policies, not calibrated human perceptual thresholds.
Pixel CIs assume independence; spatial correlation limits that interpretation.

`texture_noise_only` means error is confined to texture under this policy;
`systematic_shift` includes connected regions and their bias/coverage/detail
reasons. Other cases retain the existing diagnostic class. `absolute_shifted_share`
and `relative_shifted_share` describe the whole nonempty grid. The original
FLIP score and threshold remain recorded. `decide = true` explicitly uses
structural classification for the comparison gate; required occupancy and
explicit region thresholds still apply. Defaults retain historical acceptance.

Optional `[spatial.background]` uses `kind = "luminance", max = 0.001`,
`kind = "colour", rgb = [0,0,0], tolerance = 0.001`, or
`kind = "selection"` with the same mask/layer/box selection model as effects.
Background share is gap fraction; non-background share is coverage. The report
also records whole-frame coverage ratio and silhouette XOR/union.

## Automatic regions

`[spatial] gallery_top = 5` selects up to five regions from existing FLIP
hotspots, connected structural findings and shifted tiles, ranked by declared
bias/gap severity and error. Exact duplicate boxes are removed. Passing entries
are included. The JSON `entry.gallery` lists boxes, reasons, mean/max FLIP,
signed luminance, detail-energy ratio, gap change and relative artifact paths.
`regions/` holds base | candidate | heatmap strips and exact 2x nearest-neighbour
zooms. The comparison HTML includes a gallery with those statistics and both
views. Selection is diagnostic; it neither creates masks nor suppresses errors.

## Capture layers and scope

`[layers]` enables image-adjacent `<filename>.layers.json` manifests.
`manifest = "layers.json"` can select a different plain sidecar filename.
The `saccade-capture-layers.v1` manifest supplies `image_sha256`,
`dimensions = [width,height]`, and named layers with `image`, `kind`
(`id`, `depth`, `mask`, `scalar`, `colour`), optional `sha256`, `scale` (1),
`colour_space` (`linear` or `srgb`), and `additive` (false). The hash binds the
encoded final image; named layer bytes are hashed and decoded from the same
retained buffer. Paths remain below the image parent, dimensions must match,
and nonfinite samples are refused. Exact layer/manifest bytes and witness
paths/hashes are bundled beside the report images.

```toml
[layers]
attribution = true
[layers.scope]
layer = "surface"
mode = "union"
[layers.scope.predicate]
kind = "ids"
values = [513, 514]
```

Scope modes are `union` (default, preserving new/disappearing footprints),
`intersection`, `baseline`, and `candidate`. Empty scopes fail. Predicates
use native values including 16-bit IDs, scaled depth ranges or nonzero masks.
Scope combines with ordinary exclusions and restricts region/hotspot/tile
statistics. `mask_mode = "neutralize"` also prevents excluded differences
from bleeding into full-resolution FLIP filtering. Native equality remains
a full-image finding; identity never becomes scoped equality.

A `[[buffer]]` can put the same policy under `[buffer.capture_layers]`,
including its `scope` and `predicate`. It restricts numerical buffer metrics
in their native units. Global `[layers]` is used if that buffer has no local
policy. FLIP/structural policies require colour-image comparisons; supplying
them to a native buffer yields an explicit error rather than silent omission.

Declared additive colour layers are compared in linear light. Each component
reports signed RGB mean delta, absolute delta and projection onto the final
image delta. The residual is `final_delta - sum(component_deltas)`, computed
per pixel within the scope. Signed projections plus the residual sum to one
when final energy is nonzero; absolute deltas do not sum because components
can cancel. This is algebraic attribution, not a causal explanation. Missing
components on either side fail; no components means attribution is absent.

Effects can use `kind = "named_layer"`, a `name`, native `predicate` and an
optional `manifest` filename, sharing source-bound layer loading. HDR effect
coverage stays native; difference statistics use the declared display tone
mapper and HDR-FLIP map. Spatial statistics for HDR describe this display
mapping, not radiometric energy. Scoped SSIMULACRA2 is absent because its
whole-image metric cannot represent arbitrary inclusion masks.

## Fixed-camera temporal stability

`experiment sequence BASE CANDIDATE --fixed-camera --out REPORT --json`
adds per-tile temporal evidence. Config uses `[temporal_tiles]` with
`fixed_camera = true`, `tile_size = 32`, `fps = 60`, absolute variance/energy
increase thresholds 0.00005, relative increase ratio 1.2, motion threshold
0.5 pixels and phase-coherence confidence 0.6. All are recorded and configurable.

`tile_stability` reports baseline/candidate variance of consecutive-frame
luminance differences, high-frequency energy (mean squared second temporal
difference / 4), their increases, and connected shimmer boxes. Global motion
uses the existing phase-correlation estimator. Verdicts are `stable`,
`shimmer_increase`, `camera_not_fixed`, or `unqualified_motion` when texture
cannot support a motion estimate. A non-stable result fails the sequence gate.
At least three equal-size paired SDR frames are required. Frames are not
resampled in time; energy units are normalized sRGB squared per frame.
Object movement and rotation can confound the translation-based camera check;
the producer's fixed-camera declaration alone never establishes that condition.
