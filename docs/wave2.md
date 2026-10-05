# Wave 2 workflows

## Snapshot variation and drift

Record genuine unchanged-build captures with unique producer run identities:

```sh
saccade history record report/report.json --store .saccade-history --run-id capture-001 --environment-id chromium-fonts-v1-1280x720-warm --unchanged-build --json
saccade history analyze --store .saccade-history --entry article.png --drift --json
```

Use the same environment identity only for the same capture protocol. Omit `--unchanged-build` when measuring later revisions against the retained baseline. Ten declared unchanged-build runs are needed for normal-variation advice. `run_analysis` separates independent runs from distinct images and retains run/report/image witnesses. Drift candidates require fresh matched repeats. Suggestions and quarantine advice never change a threshold or grant approval. Older hash-only history remains provisional.
