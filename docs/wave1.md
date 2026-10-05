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

## Performance onset candidates

Record comparison reports with `saccade history record REPORT --store HISTORY`.
Then run `saccade history onset --store HISTORY --json`. The default window is
the most recent 60 distinct qualified observations per comparison identity;
`--limit` accepts 10–120. Use one independently measured nightly statistic per
record. Correlated frames from one capture are not independent nights.

The detector reads and verifies the content-addressed report objects. It requires
qualified timing and repeat noise, a recorded comparison identity, a positive
latency, and a distinct sample window. Hardware, driver, timer, configuration,
statistic, aggregation and qualification changes create separate partitions.
Older reports without the new identity field are explicitly excluded. Capture
or report time gaps are retained; no missing measurements are interpolated.

Exact dynamic programming minimizes absolute deviation from segment medians of
log timings, normalized once by robust adjacent differences and measured repeat
noise. The minimum segment is five observations and the penalty is `3 ln(n)`.
Candidates must exceed the recorded absolute, relative, repeat-noise and timer
resolution bounds, with at least five observations above the preceding level.
The output contains run/commit endpoints, effect estimates, segment boundaries,
input identities, scale, penalty and objective. Ten pre-change and five
post-change observations meet only the retrospective count recommendation.

Every onset remains a candidate. Fresh qualified repeats are required, and
intervening commits or missing nights make the interval ambiguous. The penalty
is a calibration seed: synthetic tests do not establish a 1% field false-alert
rate. Variance shifts, periodicity and general gradual drift are not qualified.
