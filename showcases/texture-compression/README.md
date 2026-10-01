# texture-compression

Seeded brick/noise texture with normal-map-like blue detail. Block candidates use 4x4 line palettes (4 colours/RGB565 or 8/RGB888): these approximate two block-compression qualities and are simulations. Real BC/ASTC encoders, format bitrates and encoder speed are out of scope.

Generate from the repository root:

```sh
python3 scripts/gen-showcases.py
```

Run from the repository root with `flipdiff` on PATH. Reports go to a
sibling directory outside the repository; use a fresh directory or an
existing flipdiff report directory.

```sh
REPORTS=../flipdiff-showcase-reports
(
  cd showcases/texture-compression
  flipdiff rank baseline candidates/block-low candidates/block-high candidates/jpeg-q40 candidates/jpeg-q85 candidates/palette-256 --labels block-low,block-high,jpeg-q40,jpeg-q85,palette-256 --metric mean --threshold 0.001 --out "../../$REPORTS/texture-compression/rank"
)
```

Expected: rank exits 1 because all lossy candidates exceed mean 0.001; lower FLIP ranks first.

Exit 1 is the intentional regression verdict; exit 0 is expected for
explain and decision-request. `EXPECTED.txt` contains the actual CLI
stdout captured by `scripts/run-showcases.sh`, including diagnostics.

All images are procedural, use fixed seeds and Pillow's bundled default
font, and contain no third-party source imagery. The timestamp variation
is simulated deterministically, rather than read from the system clock.
