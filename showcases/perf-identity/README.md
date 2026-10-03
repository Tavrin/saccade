# perf-identity

A synthetic optimisation proof: two images are bit-identical, one image changes a pixel. Flat sidecars pair timing.gpu_ms 4.85 to 2.71. These timings illustrate pairing and are not benchmark measurements.

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
  cd showcases/perf-identity
  saccade identity baseline capture --config saccade.toml --out "../../$REPORTS/perf-identity/identity"
)
```

Expected: identity exits 1: 1 fail, 2 pass, with bit-identical and gpu_ms timing text.

Exit 1 is the intentional regression verdict; exit 0 is expected for
inspect evidence and inspect. `EXPECTED.txt` contains the actual CLI
stdout captured by `scripts/run-showcases.sh`, including diagnostics.

All images are procedural, use fixed seeds and Pillow's bundled default
font, and contain no third-party source imagery. The timestamp variation
is simulated deterministically, rather than read from the system clock.
