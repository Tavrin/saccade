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

## Temporal comparison

For numbered SDR frames, run the optional graphics command:

```sh
saccade experiment temporal baseline-frames/ capture-frames/ --fps 30 \
  --display standard_4k --out temporal-report --json
```

It pairs frames by sorted trailing number through `experiment sequence`, then
computes ColorVideoVDP video JOD and one image JOD per frame. The versioned
`saccade-temporal.v1.json` contains all per-frame scores, video-map hotspots,
and flicker/ghosting findings; stdout holds at most five of each. `--min-jod`
adds an explicit JOD gate. Existing per-frame FLIP failures also exit 1.
PNG and JPEG are interpreted as sRGB; alpha is dropped. Video files and HDR
frame color transforms are not accepted by this command.

The default `standard_4k` display is the reference model of a 30-inch
3840×2160 monitor at 0.7472 m, 200 cd/m² peak, 1000:1 contrast and 250 lux
ambient. The image occupies its native pixel size on that display. ColorVideoVDP
uses replicate-first-frame temporal padding. Its raw video distortion map is
reported as one bounding box per frame over pixels above 0.2 (per-pixel JOD
below 8); disconnected regions can share a box. Flicker means alternating
signed sRGB luminance residuals of at least 0.02 across three or more frames.
Ghosting means a capture frame is closer in mean squared RGB error to the
previous reference frame than the current one, with reference motion. These
two labels are deterministic screening heuristics; ColorVideoVDP supplies the
JOD score and distortion map, not artifact classification. Specify the actual
frame rate and display conditions before using the score as acceptance
evidence. See the [crate's model documentation](https://github.com/Tavrin/colorvideovdp-rs#display-models)
and the [original ColorVideoVDP implementation](https://github.com/gfxdisp/ColorVideoVDP).

## Object and material attribution

Place optional ID sidecars next to a capture image, using its stem:

```text
captures/scene.png
captures/scene.object-id.png
captures/scene.object-id.json
captures/scene.material-id.exr
captures/scene.material-id.json
```

The JSON legend is `{"schema":"saccade-object-ids.v1","kind":"object","ids":{"1":"tree","2":"rock"}}`
(use `"material"` for material IDs). A PNG stores an unsigned 24-bit ID in
RGB, most significant byte first. An EXR stores an exactly integral ID in the
red channel, from 0 through 16,777,215. ID zero is `<unlabeled>` unless named.
The ID image must match the compared image dimensions. Exactly one PNG or EXR
and one legend is required for each supplied kind; malformed sidecars make
that entry an error. These reserved sidecar images are excluded from ordinary
image pairing.

Each report hotspot lists the share of above-cutoff FLIP error contributed by
each ID, plus its hot-pixel count. Attribution uses the capture-side ID at
each unmasked pixel within the hotspot bounding box; nearby disconnected
hot regions inside that box can contribute too. The copied sidecars and their
hashes remain in the report and portable evidence case. The HTML report shows
the same percentages in its attribution table.

Put reports outside capture inputs. `.saccade-run` and historical report/view
markers keep report images out of archive discovery. Portable artifacts use
relative paths and immutable hashes. Inspect names before publishing.
