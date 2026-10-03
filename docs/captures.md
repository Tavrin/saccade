# Capture inputs

Compare files or directories; directory entries pair by relative name.
PNG, JPEG, EXR and HDR are supported. Record resolution, color interpretation,
render mode and backend. Proof profiles also need source revision/dirty state,
binary hash/build configuration, scene/assets, camera, frame/time/seed,
warmup/sampling/cache state, GPU/driver and performance qualification.
Saccade reads supplied captures; the capture producer owns readiness and repeats.

```sh
saccade compare examples/baseline examples/capture --out capture-report --json
```

This example intentionally exits 1. Missing and new entries fail by default.
An empty scope cannot establish evidence.

Directory sidecars default to `saccade-meta.json`. Image-specific sidecars
`<stem>.saccade-meta.json` override directory values. For Moss use
`--meta-name cost-card.json`; retain the same override precedence.
`--require-matching-meta` refuses undeclared configuration differences.

Predeclare interventions with `--changes-file`; each declaration has a key,
reason and optional expected before/after values. `--declare` permits named
metadata differences with required matching metadata. Do not generate declarations
from observed differences. Declared-but-unchanged keys remain visible; a
declaration does not prove an intervention occurred.

`saccade.toml` holds thresholds, metric, HDR parameters, regions, masks and capture
requirements. Built-in defaults are overridden by project settings, the selected
profile, then explicit CLI options. User security policy constrains all of them.
Use `inspect config` to inspect values and sources. Image-noise calibration uses
FLIP units; performance noise uses milliseconds and cannot be substituted.

Put reports outside capture inputs. `.saccade-run` and historical report/view
markers keep report images out of archive discovery. Portable artifacts use
relative paths and immutable hashes. Inspect names before publishing.
