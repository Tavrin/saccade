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
