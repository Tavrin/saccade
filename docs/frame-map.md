# Frame maps for extracted video

saccade does not decode video. A tool you choose extracts frames; a frame map says
which frame is which. `saccade frame-map check MAP` reports what that sequence can
support and never resamples, interpolates or guesses.

`saccade-frame-map.v1` lists `frames` in increasing order, each with `index` (the
source frame number), `timestamp_s` (presentation time, seconds) and `file` (a path
relative to the map, never absolute or containing `..`), plus an optional `sha256`.
An optional `nominal_fps` records what the producer believes it extracted.

```sh
saccade frame-map check frames/map.json --json
saccade frame-map check frames/map.json --settling settle/saccade-settling.v1.json
```

## States

`state` is the first that applies: `invalid_files` (a file is missing or its hash
differs; skip with `--skip-files`), `missing_frames` (indices absent; the gaps are
listed), `variable_frame_rate` (per-frame step differs from the median by more than
`--rate-tolerance-pct`, default 1), `single_frame`, `constant_frame_rate`. The step
is the timestamp difference divided by the index difference, so a dropped frame is a
gap, not a rate change. Duplicate or out-of-order indices and timestamps, non-finite
times and unsafe paths are errors.

`usable_for.index_aligned` is true for a complete sequence with verified files.
`usable_for.uniform_time` is true only for a constant rate that agrees with
`nominal_fps` when one is given. Exit 0 means constant rate (or one frame); 1 means
any other state. The reasons are listed in plain words.

## Settling in map time

With `--settling` (a `saccade-settling.v1` report for the same frames, same count),
`settle` restates the result with the map's timestamps: `settled` with `elapsed_s`,
`never_settled` (no settling time exists; `elapsed_s` is `null`), or
`change_outside_sequence`. `fixed_rate_assumption_holds` says whether the report's
own seconds, which assume one frame rate, match the map; when false, use `elapsed_s`.

Output schema: `saccade-frame-map-check.v1`.
