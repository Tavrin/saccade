# Wave 2 workflows

## Snapshot variation and drift

Record genuine unchanged-build captures with unique producer run identities:

```sh
saccade history record report/report.json --store .saccade-history --run-id capture-001 --environment-id chromium-fonts-v1-1280x720-warm --unchanged-build --json
saccade history analyze --store .saccade-history --entry article.png --drift --json
```

Use the same environment identity only for the same capture protocol. Omit `--unchanged-build` when measuring later revisions against the retained baseline. Ten declared unchanged-build runs are needed for normal-variation advice. `run_analysis` separates independent runs from distinct images and retains run/report/image witnesses. Drift candidates require fresh matched repeats. Suggestions and quarantine advice never change a threshold or grant approval. Older hash-only history remains provisional.

## Compression sweep

`quality-sweep` measures outputs from your actual encoder. Use `cargo build -p saccade --features compression` and `saccade quality-sweep sweep.json --out quality-report.json --json`. It creates a new JSON file and exits 1 when candidates are missing/unusable or none meets policy.

```json
{
  "schema": "saccade-quality-sweep.v1",
  "original": {"path": "original.png", "sha256": "LOWERCASE_FILE_HASH"},
  "reference": {"path": "resized-reference.png", "sha256": "LOWERCASE_FILE_HASH"},
  "minimum_score": 90.0,
  "maximum_bytes": 100000,
  "viewing_conditions": "sRGB display, 100% scale, 60 cm, white background; human review pending",
  "candidates": [{
    "id": "jpeg-q85",
    "stages": [{
      "id": "drupal", "encoder": "GD VERSION; fixed settings", "quality": 85,
      "subsampling": "4:4:4", "pixel_policy": "normalized_srgb_opaque",
      "dimensions": [640, 480],
      "output": {"path": "q85.jpg", "sha256": "LOWERCASE_FILE_HASH"}
    }]
  }]
}
```

For two-stage processing, add the delivered image as a second stage. Every output must have the resized reference dimensions. Normalize EXIF orientation and ICC profiles externally; this version accepts opaque RGB8 sRGB only. Keep originals immutable. Inspect incremental and cumulative scores and bytes for each stage. The smallest file meeting the declared budget may differ from the lowest encoder quality. The score and declared viewing conditions do not establish invisible loss. Inspect protected details separately and perform human review.
