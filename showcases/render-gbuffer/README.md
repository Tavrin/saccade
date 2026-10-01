# render-gbuffer

A lit image plus native 16-bit depth, RGB signed normals and RG signed motion. The capture changes a local shade and reduces depth to 32 levels, still stored in a 16-bit PNG. Normal and motion stay identical. Numerical buffers bypass FLIP and retain their own units.

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
  cd showcases/render-gbuffer
  flipdiff compare baseline capture --config flipdiff.toml --out "../../$REPORTS/render-gbuffer/compare"
)
```

Expected: 2 fail (lit and depth), 2 pass (normal and motion); depth error uses normalised units.

Exit 1 is the intentional regression verdict; exit 0 is expected for
explain and decision-request. `EXPECTED.txt` contains the actual CLI
stdout captured by `scripts/run-showcases.sh`, including diagnostics.

All images are procedural, use fixed seeds and Pillow's bundled default
font, and contain no third-party source imagery. The timestamp variation
is simulated deterministically, rather than read from the system clock.
