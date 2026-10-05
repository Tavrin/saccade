# Deterministic analysis evidence

Optional analysis records distinguish checks that ran from unknown, unsupported,
excluded and rejected checks. An omitted field in an older report means that its
producer did not record the evidence. It does not mean the check passed.

Analysis provenance records the implementation revision, tool version, code
licence, resource SHA-256 identities and execution settings. A null resource hash
or licence means unavailable information. Code licensing does not establish a
model weight licence. These records do not change comparison or approval policy.

## Exclusion audit

Every new `compare` report records `exclusion_audit`; each measured colour pair
also records `pixel_exclusions`. Use `saccade inspect exclusions REPORT --json`
for the recorded audit and resolved mask runs, or omit `--json` for text. The
portable HTML report has an Exclusion audit section.

The audit lists selected and ignored capture scope, supplied names excluded by
policy, incomplete pairs, metadata gaps and ignored differences, performance
qualification, region thresholds and headroom, and channel/HDR limits. It cannot
list captures that were never supplied and were not declared in an expected list.

Excluded pixel error comes from the original FLIP map. Mask SHA-256 hashes bind
the resolved bitmap, including resized image masks. `without_masks` recomputes
the diagnostic entry/region/hotspot decision with masks removed. Capture validity
is separate. The configured result and exit code are unchanged. A negative
headroom exceeds a threshold; hotspot failure also includes equality. Historical
reports without audit fields show unavailable evidence, not zero exclusions.
