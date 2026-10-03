# cover-art

Generated gradient, geometric cover and title. Warm tone, an 8% crop zoom and decoded JPEG quality 40 demonstrate diagnostic classes. Recompression is stored as PNG after JPEG decoding so names pair exactly.

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
  cd showcases/cover-art
  saccade compare baseline capture --config saccade.toml --out "../../$REPORTS/cover-art/compare"
)
```

Expected: 3 fail: warmer is global_tone; crop and jpeg-q40 are local_structure with structural descriptions. These are measured diagnostics, not hardcoded labels.

Exit 1 is the intentional regression verdict; exit 0 is expected for
inspect evidence and inspect. `EXPECTED.txt` contains the actual CLI
stdout captured by `scripts/run-showcases.sh`, including diagnostics.

All images are procedural, use fixed seeds and Pillow's bundled default
font, and contain no third-party source imagery. The timestamp variation
is simulated deterministically, rather than read from the system clock.
