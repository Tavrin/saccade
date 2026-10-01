# upscaler

A native procedural scene with thin lines, text and a checkerboard floor, compared with half-resolution reconstructions. Bicubic and Lanczos are separate filters. The 12-frame camera pan uses a Lanczos reconstruction with alternating floor highlights to simulate temporal shimmer. Temporal instability is informational and is not a motion-compensated metric.

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
  cd showcases/upscaler
  saccade rank baseline candidates/nearest candidates/bilinear candidates/bicubic candidates/lanczos candidates/sharpened-bicubic --labels nearest,bilinear,bicubic,lanczos,sharpened-bicubic --metric mean --threshold 0.001 --out "../../$REPORTS/upscaler/rank"
  saccade sequence sequence/baseline sequence/capture --pattern 'frame_*.png' --threshold 0.005 --out "../../$REPORTS/upscaler/sequence"
)
```

Expected: rank and sequence exit 1; the shimmer sequence adds positive temporal instability.

Exit 1 is the intentional regression verdict; exit 0 is expected for
explain and decision-request. `EXPECTED.txt` contains the actual CLI
stdout captured by `scripts/run-showcases.sh`, including diagnostics.

All images are procedural, use fixed seeds and Pillow's bundled default
font, and contain no third-party source imagery. The timestamp variation
is simulated deterministically, rather than read from the system clock.
