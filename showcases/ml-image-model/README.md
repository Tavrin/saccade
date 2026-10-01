# ml-image-model

Six fixed procedural seeds stand in for shared prompts across checkpoints A and B. B adds a colour cast to seeds 101 and 104, moves one shape for seed 105 and preserves the other three outputs. Compare, explain strips and a bounded decision-request provide judge-ready evidence; no model or external judge is invoked.

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
  cd showcases/ml-image-model
  flipdiff compare baseline capture --config flipdiff.toml --out "../../$REPORTS/ml-image-model/compare"
  flipdiff explain "../../$REPORTS/ml-image-model/compare/flipdiff-report.v1.json" --out "../../$REPORTS/ml-image-model/explain"
  flipdiff decision-request "../../$REPORTS/ml-image-model/compare/flipdiff-report.v1.json" --all-failing --question accept --intent 'Checkpoint B must preserve colour and structure.'
)
```

Expected: compare exits 1 with 3 fail and 3 pass; explain and decision-request exit 0.

Exit 1 is the intentional regression verdict; exit 0 is expected for
explain and decision-request. `EXPECTED.txt` contains the actual CLI
stdout captured by `scripts/run-showcases.sh`, including diagnostics.

All images are procedural, use fixed seeds and Pillow's bundled default
font, and contain no third-party source imagery. The timestamp variation
is simulated deterministically, rather than read from the system clock.
