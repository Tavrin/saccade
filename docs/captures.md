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
`<stem>.saccade-meta.json` override directory values. Captures from the Moss
engine use `--meta-name cost-card.json`, with the same override precedence.
Its `binary.sha` and `build.commit` keys supply the binary hash and source
revision; see [agents](agents.md#moss-cost-cards) for every accepted key.
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

## Opt-in flaky-test history

Record each comparison report into a local store, then inspect variation:

```sh
saccade history record capture-report/saccade-report.v1.json --store .saccade-history --json
saccade history analyze --store .saccade-history --entry ui/button.png --json
```

The store keeps report snapshots at `objects/<sha256>.json` and a JSON lines
`index.jsonl`. Recording the same report twice has no effect. `analyze` shows at
most 10 groups by default (20 maximum); `--entry` selects one exact name. A
group needs the same baseline bytes and effective run configuration, and at
least three distinct capture hashes. An invalid capture is excluded. If the
observed values cross the threshold, or their range is as wide as the
threshold, the command suggests quarantine and a fresh repeat campaign. A
suggested threshold is the observed maximum plus one observed range, capped at
1; it is provisional and is never applied automatically. Capture validity
marked unknown, renderer or hardware differences not represented by report
configuration, and too few repeats limit any noise claim. Keep the store
outside capture inputs and review its reports before sharing it.

Put reports outside capture inputs. `.saccade-run` and historical report/view
markers keep report images out of archive discovery. Portable artifacts use
relative paths and immutable hashes. Inspect names before publishing.
