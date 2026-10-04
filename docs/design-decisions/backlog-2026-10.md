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
