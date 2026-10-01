# webapp-ui

A drawn dashboard: three regressions, a pixel-identical page, and a timestamp-only change. The header region gates its p95; the timestamp mask has a generous margin because FLIP filters spread error beyond glyphs.

Generate from the repository root:

```sh
python3 scripts/gen-showcases.py
```

Run from the repository root with `saccade` on PATH. Reports go to a
sibling directory outside the repository; use a fresh directory or an
existing saccade report directory.

```sh
REPORTS=../saccade-showcase-reports
(
  cd showcases/webapp-ui
  saccade compare baseline capture --config saccade.toml --out "../../$REPORTS/webapp-ui/compare"
)
```

Expected: 3 fail, 2 pass; button-shift, label and token fail, identical and timestamp-only pass.

Exit 1 is the intentional regression verdict; exit 0 is expected for
explain and decision-request. `EXPECTED.txt` contains the actual CLI
stdout captured by `scripts/run-showcases.sh`, including diagnostics.

All images are procedural, use fixed seeds and Pillow's bundled default
font, and contain no third-party source imagery. The timestamp variation
is simulated deterministically, rather than read from the system clock.
