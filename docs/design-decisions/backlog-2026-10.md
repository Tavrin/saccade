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
