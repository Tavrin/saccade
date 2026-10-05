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
