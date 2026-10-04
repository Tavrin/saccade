# Backlog decisions (2026-10)

## 1. Repeats as input

`experiment ablate` accepts labelled repeat groups through `--base` and
`--arm LABEL=GLOB`, while the positional form remains available. It records
accepted and excluded paths, and derives a performance floor from complete
base repeats. A rejected floor stays rejected. I rejected treating missing
optional gap terms as a qualified complete timing graph: only the common
terms receive a floor, and the missing terms remain explicit warnings.
Packet B also has differing `configuration_hash` values in the five base
performance sidecars. Those captures therefore cannot supply qualified
unchanged-build performance noise. Capture all repeats with the same renderer
configuration, qualifying warmup and complete timing provenance.

## 2. Nondeterministic arm output

An arm receives a repeat validity finding when its maximum within-arm FLIP
exceeds the base repeat FLIP for an image. File hashes and per-image FLIP values
remain in the full ablation artifact. I rejected treating every different file
hash as a rendering failure: encoding can differ while decoded samples match.
The finding is an additive arm field, and the bounded result marks validity
invalid with a typed repair action. A Markdown ablation export and the HTML
report display the same finding. No schema version changes because prior field
meanings remain intact.

## 3. Severe local changes under a mean pass

Keep `status: pass` for compatibility and add `pass_with_local_change: true`
to affected report entries. The bounded result counts and names these entries
and supplies an inspect action. I rejected changing `status` to a new enum
value because historical readers may reject it. The default rule is a hotspot
with max FLIP at least `0.5` and area at least `16` pixels; both cutoffs are
configurable. A zero FLIP score with differing native samples uses the additive
`zero_flip_native_difference` class instead of any identical label.

Packet B's `lit.png` peaks at `0.4268897`, so its passing mean does not meet
the specified default `0.5` rule. The other two images do. A demonstration
with `hotspot_local_max = 0.4` marks all three; changing the default to make
this packet pass would contradict the spec's stated default. This remains an
explicit acceptance shortfall for the default packet demonstration.

## 4. GPU clock and power state

Read `gpu_clock.json` as engine-neutral `saccade-gpu-clock.v1` or adapt Moss
`moss.gpu-clock.v2` sampled windows. A present sidecar with missing power state,
incomplete frames, failed queries, unestablished warm-to-boost stability,
throttling, or a clock span above 5% of its median is unqualified. Different
device, power state, window set, or median clocks between arms reject timing
comparability. I rejected accepting a producer's clock samples solely because
its performance sidecar says `qualified`; those are separate facts. Both clock
summaries and reasons remain in the performance diff. Absence on both sides
preserves historical behavior; absence on one side rejects pairing. The 5%
span is a conservative measured-window limit, not an exact-MHz requirement.

## 5. Three-verb front door

The default help shows `compare`, `prove`, and `review`, then names every
existing command under an Advanced heading. `prove identity` routes to the
existing exact identity path and `prove performance` to ablation. The old
commands remain callable and discoverable through explicit help and compiled
capabilities. I rejected renaming or removing them because existing scripts
and §17 compatibility depend on those paths. The new identity form exposes
the common options; advanced identity options remain on `identity`.

## 6. Agent skill

Reuse the generated, bounded Claude and Codex packs from one shared guide and
add an explicit one-request prompt for each environment. I rejected a new
agent service or provider dependency: the installed CLI already yields the
measurement, bounded JSON, and typed next actions. Prompts select `compare`,
`prove identity`, or `prove performance`, and never turn a baseline update into
an automatic step. The generator's 4,800-byte per-pack limit remains enforced.

## 7. Intent-first checks

Use the existing `--intent-file` path for a pre-capture
`saccade-visual-intent.v1` declaration. It requires an objective, exact entry
names, box or mask per change, expected kind, and `no_change_elsewhere: true`.
Deterministic verification writes a separate versioned artifact, a short HTML
summary, and bounded CLI counts; mismatches exit 1. The declaration is copied
into the canonical case as structured intent with a hashed source. I rejected
replacing the existing evidence intent schema or making AI review an
acceptance gate: both would alter existing authority contracts. The report
schema also stays stable. Hotspot boxes are the available location evidence,
so overlap can match a declaration, while a box extending outside its declared
region is conservatively unexpected. Tone direction uses global exposure and
a full-frame region; `none` uses exact native equality. Missing scope or mask
dimension mismatch is unmeasurable rather than a fabricated pass. The
pre-existing plain-text `--intent` remains lower-assurance review context.

## 8. Playwright adapter

Use a small Playwright reporter to record complete expected/actual screenshot
attachments and a CLI `ingest playwright` route to copy them into paired
directories, sidecars and a normal report. The reporter records project,
browser, viewport and test ID. I rejected inferring pairs from Playwright's
filenames because `snapshotPathTemplate` can change them and multiple projects
or failures can collide. The adapter refuses an empty manifest; passed tests
without actual attachments do not become invented comparisons. Rust core has
no JavaScript dependency and the CLI never executes the test suite.

## 9. Git bisect

Run Git's `bisect run` in a disposable local clone of the user's repository,
with an internal saccade step callback and per-revision capture, report and
log outside the original repository. Validate original clean state and commit
ancestry before starting, copy the baseline, and call `git bisect reset` in
the clone on every exit path after start. The original HEAD never moves,
including when a capture command dirties its checkout. I rejected silently
cleaning the original repository to rescue a failed bisect: that could discard
user data. Dirty capture steps abort and retain their logs; the disposable
clone is removed after a successful reset.
Failed captures and incomplete image/performance evidence are skips, not bad
commits. `--perf` requires qualified paired timing and repeat noise. The old
ordered-run `experiment bisect` stays available separately.

## 10. Engine layouts

Read existing PNG test outputs through additive `ingest blender`, `bevy`,
`unity` and `unreal` paths, using each producer's documented structure or
serialized comparison paths. Pair only named, documented files, keep source
diffs as provenance, and run the normal saccade measurement after copying.
I rejected guessing an on-disk Godot screenshot-test layout from its Viewport
capture API: the project documents capture, but not a standard golden/actual
directory or result format. `ingest godot` remains an explicit residual until
a project-level layout is specified. Rust never executes producer code.

## 11. HDR and EXR comparison

Keep the existing `image` crate decoders for OpenEXR and Radiance HDR and the
BSD-3-Clause `flip-rs` HDR-FLIP path. `flip-rs` 0.1.2 already implements the
reference exposure sweep and reports its resolved range; a second HDR-FLIP
implementation would add drift risk. A synthetic Radiance fixture verifies
decoding and the Reinhard exposure endpoints against the NVIDIA FLIP v1.7
`image::computeExposures` reference formula to 1e-5 stop. The existing EXR
round trip and highlight-change tests cover float input and multi-exposure
behavior. The dependency's published C++ parity harness reports exact results
on its measured corpus with 1e-5 per-pixel and 1e-6 pooled/exposure acceptance
limits; this lane does not run or download that third-party suite. We constrain
explicit exposure counts to the library's 128 maximum before comparison.
Automatic exposures on an all-black reference remain undefined by the
reference and require explicit endpoints. Sources: [NVIDIA's FLIP v1.7
implementation](https://github.com/NVlabs/flip/blob/main/src/cpp/FLIP.h),
[NVIDIA HDR-FLIP paper](https://research.nvidia.com/publication/2021-05_HDR-FLIP),
and [flip-rs parity results](https://github.com/Tavrin/flip-rs#parity-with-the-c-reference).

## 12. Flaky-test memory

Use an explicit `history record` step on existing reports, then `history
analyze` for bounded local advice. Store full report bytes under their SHA-256
and one JSON line per unique report in an index. I rejected silent recording
from every `compare`: users must choose a retention location, and routine
comparison must stay stateless. Variation groups require the same baseline
hash, effective report configuration, entry metric and threshold. Duplicate
capture hashes do not add evidence, and invalid captures are excluded.
Three distinct captures are the minimum to flag a threshold crossed by
observations or a variation span at least as wide as the threshold. The
suggested threshold is a provisional observed maximum plus one span (bounded
at 1), never a config mutation. This is a deterministic screening rule, not a
confidence interval or a claim that unknown capture provenance is qualified.

## 13. Temporal checks with ColorVideoVDP

Add a graphics-only `experiment temporal` path that reuses numbered-frame
pairing and the existing per-frame FLIP report, then runs `colorvideovdp`
0.1.1 on the same PNG/JPEG sRGB frames. I rejected implicit display or frame
rate inference: the user supplies FPS, and the selected display model is
recorded. The default display is `standard_4k`. The independent temporal
artifact contains video JOD, each frame's still-image JOD, above-threshold
raw-map boxes and typed flicker/ghosting screening findings. An explicit
`--min-jod` gates JOD; ordinary per-frame FLIP failures still gate as before.
The transient map is a perceptual difference map, not a semantic classifier,
so flicker and lagging-frame labels come from declared deterministic rules.
I rejected an FFmpeg dependency or guessed HDR display transform for this
first path. The MIT Rust crate's published parity is scoped to its own
measured corpus; this lane's synthetic tests cover the adapter and findings.

## 14. Per-object attribution

Use capture-side `<stem>.object-id.{png,exr}` and
`<stem>.material-id.{png,exr}` with a versioned JSON legend of numeric IDs to
names. PNG RGB encodes a u24 ID; EXR red stores an exact integral u24 value.
I rejected palette-index PNG because image decoders can expand it, changing
the numeric identity. ID buffers are excluded from ordinary image pairing.
Invalid or partial sidecars turn the compared entry into an error. Attribution
weights each unmasked above-cutoff FLIP pixel inside the hotspot bounding box
by its error, rather than counting pixels equally; copied sidecar bytes and
hashes are bound into the report and evidence case. The HTML table uses the
same report data. The hotspot detector keeps component areas but not component
membership, so an attribution box can include nearby distinct components;
`measured_hot_pixels` makes that limitation visible. I rejected claiming exact
component membership without changing the existing hotspot algorithm and ABI.

## 15. Inline PR images

Make inline images an explicit `inline-images: 'true'` companion to the
existing sticky `comment`. A small Python standard-library helper renders up
to three failing entries to bounded PNG snapshots and heatmap views, then
uses GitHub's Git Database API to create a no-parent root commit on the
dedicated `saccade-assets` branch or a non-force descendant commit. A marker
prevents reusing an unrelated branch. Comment image URLs pin the resulting
commit; the original immutable artifact link remains. I rejected pushing from
the checkout because it could alter the PR branch or require persisted Git
credentials. Fork PRs perform no writes; API denial or any image-rendering
failure leaves the artifact-only summary. The action cannot raise its own
token permissions, so a trusted caller must grant `contents:write` and
`pull-requests:write`. Offline API mocks establish the root/non-force calls
and fallback behavior; live GitHub permissions and private raw-image rendering
remain unqualified without an authorized repository run.
