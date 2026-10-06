# Performance sidecar kit

Fixtures and recipes for producers that feed `saccade timing ab` or `saccade perf
validate`. Saccade never runs the benchmark; the kit shows the files it reads and the
outcomes it must give.

`examples/perf-kit/` (regenerate byte for byte with `python3 scripts/gen-perf-kit.py`):

| File | Content |
| --- | --- |
| `a.json`, `a-control.json` | two baseline exports of 12 runs each (control = same-session unchanged arm) |
| `b-unchanged.json`, `b-slower.json`, `b-faster.json` | candidate exports: unchanged, 8% slower, 8% faster |
| `session-<name>.json` | `saccade-timing-session.v1`: 12 candidate pairs and 12 control pairs, alternating order, 3% band |
| `sidecar-valid.json`, `sidecar-invalid.json` | a `saccade-perf.v1` sidecar that passes and one that fails validation |

Declared outcomes, asserted by `crates/saccade/tests/measure.rs`:

| Session | `timing ab` verdict | Exit |
| --- | --- | --- |
| `session-unchanged.json` | `equivalent` | 0 |
| `session-slower.json` | `slower` | 1 |
| `session-faster.json` | `faster` | 0 |

Reports are `diagnostic_only` with reason `clock_readiness_provenance_unavailable`
because the fixtures carry no clock provenance. That is the correct label for any
hand-assembled input.

## Recipes

- **hyperfine**: `hyperfine --runs 12 --export-json a.json 'workload'`. In a session
  pair use `{"file":"a.json","format":"hyperfine","index":i}`; `index` selects run `i`.
  Acquire pairs interleaved, one run of each arm back to back, alternating `ab` and `ba`,
  and add same-session control pairs (`"aa": true`) comparing the baseline with itself.
- **Any tool that writes JSON**: `{"file":"m.json","format":"json","path":"readings.elapsed_us","unit":"us"}`.
- **A producer's own sidecar**: emit `saccade-perf.v1` or `v2`, check it with
  `saccade perf validate sidecar.json`, then reference it with `"format":"perf"`.
- **Plain table**: `saccade timing ab pairs.csv --format csv` with columns
  `id,block,order,a_ms,b_ms,aa`.

Checklist before trusting a verdict: at least 6 candidate and 6 control pairs, both
orders present, one process or run per pair, a stated `band_pct`, and a `max_pairs`
(and any `looks`) fixed before collection. Method and limits:
[paired-performance.md](paired-performance.md).
