# Asset and LOD views with geometry evidence

Build with `--features geometry,graphics`. Supply paired renders from a declared
finite camera set, then run the existing geometry measurement workflow:

```sh
saccade experiment geometry source.obj lod.obj --unit metres \
  --views cameras.json --out asset-review.json --json
```

The combined `saccade-geometry.v1` document keeps the sampled surface measurements
and exact ordered geometry identity. Its additive `views` field contains the
`saccade-asset-view-report.v1` packet. Historical documents without this field
still deserialize. Without `--views`, geometry behaviour is unchanged. The
geometry measurement remains distinct from `prove mesh-identity`, which excludes
appearance. This command does not run a renderer or measure runtime cost.

The [camera manifest schema](../crates/saccade-core/schemas/saccade-asset-views.v1.schema.json)
defines `saccade-asset-views.v1`. Declare the common length unit and both assets'
source document SHA256, ordered decoded geometry SHA256 and LOD/cut labels.
Obtain geometry identities from the ordinary geometry report. For glTF, loaded
external geometry resources remain recorded in the geometry provenance; decoded
geometry hashes bind their effect on the rendered geometry. Compare selected
cuts or actual LOD surfaces, rather than overlapping levels of a cluster DAG.

Declare every required viewpoint in `views`, even when acquisition fails. Each
has a unique ID, a distinct camera, and optional reference/candidate render
receipts. Cameras use finite, nonsingular row-major 4×4 `model_to_world`,
`world_to_view` and `projection` matrices acting on column vectors. The fixed
convention is `row_major_column_vector_right_handed_clip_z_zero_to_one`.
World/placement matrices must be affine. Matrices describe the supplied capture;
Saccade does not establish their physical accuracy or recapture it.

The common `context` records resolution, renderer name/version, source and
executable hashes, settings, lighting, background and colour pipeline hashes.
Use `asset_materials` to compare material or texture changes, retaining each
receipt's material and ordered texture hashes. Use `override_materials` for
controlled geometry views and declare the exact common override material hash.
Keep temporal sampling, exposure and tone mapping in the pinned render settings.

Each receipt binds its encoded image to the exact camera/context objects and the
appropriate asset document/decoded geometry. Camera/context hashes are SHA256
of the typed objects using Saccade's canonical JSON contract; the library exposes
`asset_views::hash`. Matrix values serialize as f64, including values such as
`0.0`. Encoded image pins are ordinary lowercase SHA256 of exact file bytes.
The report retains the full manifest, its canonical body hash, all receipts and
common context hash. Producer identities and render-to-asset assertions are
recorded assertions, not executable attestation.

Images and optional masks must be relative paths inside the manifest directory;
parent/absolute paths and symlink escapes are rejected. Captures must be encoded
8-bit SDR at the declared resolution. There is no resizing, tone mapping,
registration or image alignment. Limits are 64 views, 2048 pixels per axis,
four million pixels across declared views, and 64 MiB per encoded file.

Every measured view has raw full-frame FLIP, a row-major error map and bounds
of errors above `maximum_mean_flip`. The worst view is chosen by mean FLIP,
with lexical view-ID tie breaking. Inspect maxima and local maps as well: a
small defect can have a low full-frame mean. The report does not average away a
bad view. Missing receipts/files or rejected hashes, cameras, context, dimensions
or masks remain explicit rows with null measurements.

Optional paired `silhouette` pins refer to L8 PNG object masks. Values 0..255
represent fractional object coverage; they are not automatically inferred from
a background image. The report retains each side's mean coverage, mean absolute
coverage change and changed bounds. `maximum_silhouette_change` controls that
separate check. Without paired masks, silhouette coverage is unknown. FLIP's
existing alpha handling still applies to render-image alpha.

`coverage` counts all declared views, supplied receipt pairs, measured views,
missing views and rejected views. Its fraction is measured/declared camera views,
not angular or visible-surface coverage. Exit 0 with `--views` requires complete
coverage and no per-view mean FLIP or supplied silhouette threshold failure;
exit 1 includes incomplete coverage. Invalid plans or mismatched compared mesh
identities exit 2. These policies concern supplied render observations, not a
certified geometry distance bound or a human judgement of LOD quality.

Synthetic supplied images test a front-only set missing a rear-localized defect,
material/normal-map-like appearance changes, fractional coverage loss and missing
or stale evidence. They do not prove that a renderer produced the images from
the declared asset. Finite views establish sampled visibility only. Other camera
angles, temporal popping, cracks, motion, runtime cost and renderer integration
remain separate qualification work.
