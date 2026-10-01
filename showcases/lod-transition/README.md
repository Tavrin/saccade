# lod-transition

A smoothly moving procedural gear. At frame 6 (zero-based), the capture loses its fine spokes for one frame, simulating LOD threshold jitter; the reference preserves detail. This creates an isolated error spike and adds temporal variation at the pop and recovery.

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
  cd showcases/lod-transition
  saccade sequence baseline capture --pattern 'frame_*.png' --threshold 0.005 --out "../../$REPORTS/lod-transition/sequence"
)
```

Expected: sequence exits 1 with 1 frame over threshold; worst frame is frame 6 and temporal instability is positive.

Exit 1 is the intentional regression verdict; exit 0 is expected for
explain and decision-request. `EXPECTED.txt` contains the actual CLI
stdout captured by `scripts/run-showcases.sh`, including diagnostics.

All images are procedural, use fixed seeds and Pillow's bundled default
font, and contain no third-party source imagery. The timestamp variation
is simulated deterministically, rather than read from the system clock.
