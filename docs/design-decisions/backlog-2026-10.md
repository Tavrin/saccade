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
