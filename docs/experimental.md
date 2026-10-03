# Experimental checks

Safety/accessibility prechecks require a build with `--features prechecks`.
They supply detector evidence, not certification or formal compliance.

```sh
saccade experiment safety showcases/photosensitivity/frames --fps 32 --standard itu-bt1702 --out safety-report
```

This synthetic 4 Hz fixture intentionally exits 1. Reports use static previews;
do not play its flashing frames. No GPU or provider is called.

`experiment a11y` exports color-vision simulations and checks declared regions.
Thresholds and inferred contrast clusters have limits; confirmed semantics and
formal testing remain human responsibilities. See [detector details](safety-a11y.md).
The six-tool MCP interface exposes enabled experiments through `saccade_measure`.
Missing computation reports the required feature.
