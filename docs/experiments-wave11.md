# Imported timings, settling and repeat ablations

These interfaces operate on externally acquired evidence in any field. Capture and
benchmark tools own execution. Saccade does not run arbitrary commands.

## Timing A/B

`saccade timing ab SESSION.json --out timing-report --json` imports a
`saccade-timing-session.v1` manifest. `--format csv` accepts UTF-8 unquoted columns
`id,block,order,a_ms,b_ms,aa` (the last column is `true` or `false`). CSV uses a 2%
band, confidence 0.95, seed 1 and 2048 resamples; `--band-pct` changes the band.

A manifest declares `session`, `band_pct`, `max_pairs` (6..128 A/B pairs),
`confidence` (0.8..0.99), `seed`, `resamples` (1024..8192), and `pairs`.
Each pair has a unique `id`, contiguous `block`, `order` (`ab` or `ba`),
`a` and `b` (1..3 positive millisecond frames from that run), and optional `aa`.
At least six A/B blocks and six same-session A/A blocks are needed for a verdict.
Frames within a run are averaged; they do not become independent observations.

For example, one row from a specimen-processing experiment is:

```json
{"id":"p01","block":"block01","order":"ab",
 "a":[10.1,10.3],"b":[11.2,11.3],"aa":false}
```

A run can instead reference an external file:

```json
{"file":"benchmark.json","format":"hyperfine","index":0}
{"file":"capture/saccade-perf.json","format":"perf","unit":"ms"}
{"file":"measurement.json","format":"json","path":"observations.0.elapsed","unit":"us"}
```

Hyperfine's `results[0].times` is in seconds. Use an explicit JSON path such as
`results.1.times` for another arm and `index` to select its run. The extractor supports
object keys and numeric array indices; exact dotted keys take precedence. Referenced
arrays exceeding three frames need an explicit run index. The default imported unit
is ms; `s`, `us`, and `ns` are also supported. Paths resolve beside the manifest,
so existing captures can be used without copying or modifying them. Exact source
SHA-256 values are recorded. Unknown formats, missing paths and invalid numbers fail.

The estimator is Hodges–Lehmann over paired log ratios, converted to percent. The
percentile bootstrap resamples whole declared blocks. A/A uses the same estimator;
the noise floor is the maximum absolute endpoint of its interval. A floor wider than
the declared practical band refuses a verdict. Reports distinguish `faster`, `slower`,
`equivalent` **within the recorded ±band**, and `inconclusive`, with explicit reasons
for too few pairs/blocks, incomplete look counts, a wide floor or an interval spanning
the practical band. Constant observations can establish equivalence; zero spread does
not establish provenance or universal bootstrap coverage.

Optional `looks: [12,24,48]` must increase to `max_pairs`. Each look uses Bonferroni
confidence spending across all declared looks, including unobserved future looks.
The recorded rule permits stopping only at those counts; default is the fixed maximum.
An acquisition block must be complete before looking. This bootstrap procedure has
conditional, approximate coverage; order, independence and preregistration remain
producer assertions. An inconclusive look is not a failed experiment and must not be
repeated selectively to get a favourable answer.

Optional `captures: [["a-run","b-run"], ...]` reuses performance clock/readiness
qualification and its typed reason codes. GPU clock telemetry is required unless the
session explicitly records `gpu_clocks_not_applicable: true` for CPU work. Missing
capture qualification makes the statistical result **diagnostic-only**. Qualification
rejections make it inconclusive; imported samples alone never establish runtime/GPU
acceptance. See [performance qualification](identity-and-performance.md).
Exit codes: 0 for faster/equivalent/inconclusive, 1 for slower, 2 for invalid input/IO.
Read `verdict`, `diagnostic_only` and `reasons`, not just the process status.

### External acquisition recipes and validators

Freeze the binary, prepared inputs, timer, practical band, maximum, seed and look counts
before acquiring anything. Record one changed variable per arm and qualify readiness.
Acquire run pairs in ABBA order (pair `ab`, then pair `ba`), or randomized equal-sized
blocks with the recorded seed. Correlated adjacent pairs share a block ID. The importer
validates IDs, orders, block contiguity and presence of both orders; it cannot authenticate
an external tool's recorded order. Do not reinterpret separately batched Hyperfine A and
B runs as interleaved pairs. Export each scheduled external invocation's observations
and join them into the manifest. Acquire unchanged-arm A/A blocks interspersed in the
same session, with the same binary/workload and readiness checks. Do not borrow an old
noise floor. Mark those rows `aa: true` and use separate resampling blocks.

For a generic external hook, apply CPU pinning through `taskset -c CORES TOOL ...` on
platforms supporting it; use the platform's affinity mechanism elsewhere. Existing
`SACCADE_HEAVY_WRAPPER` wraps coordinator gate execution, not timing imports. Declare
CPU-only resource requirements through your external scheduler. Saccade has no
scheduling helper; external tools retain their scheduling and command syntax.

## Event-relative visual settling

```sh
saccade experiment settle frames --change-frame 12 --fps 30 \
  --tile-size 32 --threshold 0.02 --consecutive 3 --final-frames 3 \
  --reference independent-reference-frames --out trajectory --json
```

`--event event.json` can replace `--change-frame`; the marker contains an integer
`change_frame`. It is a zero-based position in numeric-suffix-sorted image frames,
not a filename number or execution instruction. Frame indices must be unique.
All observation/reference frames must have identical dimensions. Limits are 512 frames,
4096 tiles and 256 MiB decoded pixels per sequence. Images are read with decoder limits.
A provided reference sequence has one target for each observation; otherwise the final
window's mean RGB is the target. Exact frame and event-marker hashes bind the report.

`saccade-settling.v1.json` records per-frame normalized RGB absolute error per tile,
the first K-consecutive under-threshold frame, time-to-settle in seconds, onset lag in
frames, and residual projection onto pre-change minus reference content. The global
trajectory is the maximum tile error, and settles when every tile is below threshold
for the same K-frame interval. Missing onset/settling stays null. The report retains
worst tile indices and `index.html` shows an HTML/SVG strip chart with hover values.

Use a fixed viewpoint and exposure. Onset compares with the initial pre-change frame and is threshold-dependent; it may precede the declared marker, producing negative lag. Ghosting is a signed
least-squares correlation/projection coefficient, not causal proof; 0.25 means a residual
aligned with one quarter of the pre-change/reference contrast. Zero contrast is null.
A final-window target can hide a persistent residual: use an independent reference for
such questions. K recorded observations do not prove unobserved future settling.
Generated fade and residual proofs represent a laboratory image sequence; they require
no capture driver, engine vocabulary or native dependencies.

## Multi-arm repeat tables

```sh
saccade experiment ablate --base 'reference-*' \
  --arm 'method-a=method-a-*' --arm 'method-b=method-b-*' \
  --require-valid-arms --fingerprint-map map.toml \
  --intended-variable run.env.method --out table --json
```

Every complete repeat contributes to its arm's `timing.samples_ms`, median and IQR.
`base_timing` records the baseline distribution. The shift is the unpaired two-sample
Hodges–Lehmann median of all arm-minus-baseline differences; independent arm bootstrap
(95%, seed 11, 2048 draws) supplies the interval. **No CI with fewer than two repeats
in either arm.** This repeat table does not manufacture matched pairs or remove drift.
Use `timing ab` for interleaved paired inference. The JSON, Markdown and HTML tables
rank descriptive shifts in increasing timing order. Performance comparability and
image/repeat validity remain separate; a descriptive rank is not a qualified speed claim.
Image classes come from existing evidence-quality analysis when enabled in config.
Mapped-only fingerprints validate every repeat before comparison. Readiness or undeclared
mapped differences refuse strict analysis; image repeat instability invalidates the arm
and leaves its timing rank null. Excluded repeats never enter a distribution.

## One-flag native scopes and occupancy

`compare` and `render-evidence` accept:

- `--mask-layer 'labels=id=12,13'`: a named layer and integer IDs.
- `--mask-layer 'labels=label=cell*'`: generic producer label glob;
  `material=PATTERN` is a syntax alias with identical semantics.
- `--mask-layer 'depth=range=0.1,2.0'`, `above=0.5`, or `mask`.
- `--mask-from-dump objects.json`: alias of `--mask-dump`, using the existing generic
  screen-space ID dump rasterizer. The path is relative to each capture/image parent.
- `--require-effect 'labels=id=12:64'`: at least 64 occupied pixels on both sides;
  default minimum is one. Repeat for multiple effects. `mask:FILE[:MIN_PIXELS]` selects
  a binary inclusion image. This is an occupancy gate, independent of perceptual equality.

Scope shortcuts expand to existing layer policy, union selection and neutralization.
The effective config and effect policies are recorded. Native manifest `labels` is an
optional JSON ID-to-string dictionary (for example `"12":"cell-nucleus"`). Dump names
supply the dictionary for `derived_instances`. No application-specific vocabulary is
interpreted. Required named-layer effects inherit the declared manifest/dump source.
Empty scopes and insufficient occupancy fail even when the two final images are equal.

## External report links and indexes

Add repeatable `--source-ref URI_OR_KEY` to any report-producing CLI command. Persisted
measurement reports have a content-addressed `report_id` (`sha256:` plus 64 hex digits)
and `source_refs`. IDs use the existing `Measurement::report_identity` semantic projection.
For pair reports, this is exactly the existing evidence-case semantic SHA-256: generation
timestamps, baseline/capture locations, entry presentation paths and link fields are omitted.
Other report types use its shared value projection, omitting only link fields and explicit
generation timestamps; their input/configuration paths and source hashes remain bound.
Canonical object keys are sorted and arrays retain order. Exact artifact-byte hashes remain
separate. Compact CLI/MCP envelopes retain the referenced measurement's ID. Historical
reports without links remain readable. Acquisition plans, capture/policy inputs and human
approval/evidence documents retain their separate immutable identity contracts.

CLI reports append locked JSONL rows to `reports/index.jsonl` beside each report (inside `--out`);
`--report-index FILE` selects another destination. Core library writers use the same default. Each `saccade-report-index-row.v1` row records report ID, source refs, verdict
class, Unix insertion timestamp, report schema and report path. Multiple rows for the same ID
are allowed; hubs may deduplicate by `(report_id, source_refs, report_path)`.

```sh
saccade index export --index reports/index.jsonl --format jsonl --out hub.jsonl
saccade index export --index reports/index.jsonl --format json
```

Export is bounded to 16 MiB and read-only; optional output uses create-new semantics.
A hub stores its capture card's URI/key as a source ref, joins exported rows on that ref,
and links its card to `report_path` (or a hosted copy preserving the same report ID). Verify the report ID after dereferencing: regenerating an owned output path can replace its contents, while old index rows remain historical.
Never execute source refs; URI/key values are data. Different-domain cards use the same
contract without plugins, producer-specific joins or names built into saccade.

MCP `saccade_measure` mirrors `timing_ab`, `settle`, and `index_export`, with root-contained
inputs/outputs. Timing and settling accept `source_refs`. Existing comparison tools
mirror native mask shortcuts. Full trajectories stay in artifacts; tool responses are bounded.

## Subtree fingerprint maps

```toml
compare = "mapped_only"
absent = "value"
[subtrees."run.env"]
path = "config.options"
# file = "producer.json" # optional sibling source
exclude = ["output.*"]
```

Every leaf below the source object maps to the destination prefix automatically, including
newly recorded optional flags. Relative leaf glob exclusions are removed before comparison.
Source values must be objects. Flat dotted keys below the source prefix are also discovered as a virtual object when the exact root object is absent; an exact object takes precedence. Normal field mappings keep their existing behaviour.
Duplicate destinations, unsupported destination names and malformed paths fail. `mapped_keys`
lists the full sorted union of effective keys covered on either arm; excluded leaves do not
appear. With `absent = "value"`, a newly present flag differs from absence (exit 3), two
absences compare equal, and present null stays distinct. Required readiness evidence retains
its strict missing/false semantics. A new field need not be enumerated manually to be checked.

## Linked-report contract migration

Strict legacy JSON schemas (`additionalProperties: false`) reject an added `report_id`.
Their shipped schema files remain unchanged. Linked writers emit distinct successors:
`saccade-report.v1` → `saccade-report.v2`, `saccade-grounded.v1` → `saccade-grounded.v2`,
`saccade-localized.v1` → `saccade-localized.v2`, and `saccade-onset.v1` → `saccade-onset.v2`.
The two historical result layouts have distinct successors: `saccade-result.v1` →
`saccade-result.v3` and the current `saccade-result.v2` → `saccade-result.v4`. This avoids
colliding with the already existing result-v2 contract. All strict report successors require
`report_id` and `source_refs`; their complete mapping is the public
`saccade_core::report_links::SCHEMA_MIGRATIONS` table. `schema list` and `schema path`
include both versions. Permissive legacy report schemas accept optional links without
needing a version bump. Acquisition inputs and approval authority records keep their contracts.

In particular, `saccade-crop-check.v1` → `saccade-crop-check.v2`,
`saccade-faces.v1` → `saccade-faces.v2`, and `saccade-ui-review.v1` →
`saccade-ui-review.v2` are intentional strict-report migrations. Readers accept both
versions, including nested reports, while rejecting unknown successors. Crop receipts
can retain a legacy face receipt inside the linked crop report. Replay readers project
only known linkage fields into the legacy data model; other unknown fields still fail.

Upgrade strict consumers to the successor schema, selected by the payload's `schema`
discriminator. New readers accept legacy and linked versions and still reject unknown newer
versions. Existing report filenames remain stable (for example `saccade-report.v1.json` may
contain the v2 contract); dispatch by the payload rather than inferring schema from a filename.
IDs normalize a linked version to its original measurement contract before applying
`Measurement::report_identity`, so migrating a record never changes its semantic measurement
identity. Existing authority bindings and exact-byte hashes are not rewritten.

The stable library grammar is `saccade_core::mask_spec::parse_layer_spec` and
`parse_effect_spec`, returning validated native predicates and `EffectSpec`. CLI and MCP
use these same parsers. They perform no IO; consumers resolve mask images and producer
manifest/dump dictionaries before evaluating samples. `material=` aliases the generic
`label=` dictionary predicate. A final `:MIN_PIXELS` suffix is a positive `u64`, default one.

## Fingerprint record size

`arms check A B` accepts each fingerprint JSON record up to 16 MiB by default,
including telemetry unrelated to mapped identity. A map-level `max_record_bytes`
sets a byte limit; `--max-record-bytes N` overrides both the map and configuration.
The effective limit is echoed as `max_record_bytes` in the arm-check output.
For example, a laboratory map may set `max_record_bytes = 33554432`, or run
`saccade arms check baseline candidate --max-record-bytes 33554432 --json`.
The hard ceiling is 64 MiB; zero and larger limits are rejected. MCP `arms_check`
also accepts `max_record_bytes`. Limits cover primary, inherited, record_files and
mapped sibling JSON records. Mapping files retain their separate 1 MiB bound.
Oversize errors name the record, observed bytes, active limit and override syntax.
Parsing retains the full bounded JSON object, preserving existing all-field checks;
streaming and pruning unmapped subtrees are deferred.
