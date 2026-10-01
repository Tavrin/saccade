# ml-image-model

Six fixed procedural seeds stand in for shared prompts across checkpoints A and B. B adds a colour cast to seeds 101 and 104, moves one shape for seed 105 and preserves the other three outputs. Compare, explain strips and a bounded decision-request provide judge-ready evidence; no model or external judge is invoked.

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
  cd showcases/ml-image-model
  saccade compare baseline capture --config saccade.toml --out "../../$REPORTS/ml-image-model/compare"
  saccade explain "../../$REPORTS/ml-image-model/compare/saccade-report.v1.json" --out "../../$REPORTS/ml-image-model/explain"
  saccade decision-request "../../$REPORTS/ml-image-model/compare/saccade-report.v1.json" --all-failing --question accept --intent 'Checkpoint B must preserve colour and structure.'
)
```

Expected: compare exits 1 with 3 fail and 3 pass; explain and decision-request exit 0.

Exit 1 is the intentional regression verdict; exit 0 is expected for
explain and decision-request. `EXPECTED.txt` contains the actual CLI
stdout captured by `scripts/run-showcases.sh`, including diagnostics.

All images are procedural, use fixed seeds and Pillow's bundled default
font, and contain no third-party source imagery. The timestamp variation
is simulated deterministically, rather than read from the system clock.
