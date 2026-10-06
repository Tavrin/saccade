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
      "id": "resize", "encoder": "GD VERSION; fixed settings", "quality": 85,
      "subsampling": "4:4:4", "pixel_policy": "normalized_srgb_opaque",
      "dimensions": [640, 480],
      "output": {"path": "q85.jpg", "sha256": "LOWERCASE_FILE_HASH"}
    }]
  }]
}
```

For two-stage processing, add the delivered image as a second stage. Every output must have the resized reference dimensions. Normalize EXIF orientation and ICC profiles externally; this version accepts opaque RGB8 sRGB only. Keep originals immutable. Inspect incremental and cumulative scores and bytes for each stage. The smallest file meeting the declared budget may differ from the lowest encoder quality. The score and declared viewing conditions do not establish invisible loss. Inspect protected details separately and perform human review.

## Capture inventory

After comparing generic capture layouts, reconcile stable cases:

```sh
saccade inventory --manifest suite.json --report report/saccade-report.v1.json --out inventory.json --json
```

```json
{
  "schema": "saccade-inventory.v1",
  "expected": [{"case_id": "par/article/mobile", "entry": "article.png", "required": true}],
  "supplied": [{"case_id": "par/article/mobile", "entry": "article.png", "state": "captured", "capture_sha256": "LOWERCASE_FILE_HASH"}]
}
```

Producer states are `captured`, `missing`, `unusable`, `stale`, `skipped`, and `quarantined`. Use null entry/hash when there is no artifact. Stable IDs survive filename changes; hashes bind the actual capture to its comparison. Missing, skipped or quarantined required cases prevent complete coverage. Equal bytes across two named cases do not establish equivalence. The JSON accounts for every expected case; a failed image measurement is still a compared case.

The Playwright reporter now inventories every test from `onBegin`, including passed tests without attachments. To compare passing snapshots, attach both expected and actual images with role-suffixed attachment names, for example `article-expected` and `article-actual`. Quarantine annotations remain visible. Retry attempts remain duplicate identities. `saccade ingest playwright manifest.json --out suite-report` writes `inventory.json` as well as the usual comparison and returns 1 for incomplete required coverage. Older manifests lack suite coverage evidence. For multiple predeclared page states, supply a generic suite manifest.

## Localized changes

```sh
saccade localized-check reference.png candidate.png --box 80,120,300,200 --out localized-report --json
saccade localized-check reference.png candidate.png --mask inclusion.png --out masked-report
saccade localized-check reference.png candidate.png --selector 'main .card' --metadata dom.json --out selector-report
```

Masks are binary grayscale PNGs, with 255 inside and 0 outside. Both the intended region and complement must contain pixels. The command writes `frozen-region.json` before measuring and `localized.json` with complete evidence. Use `--region frozen-region.json` to reproduce the scope. Exact native outside preservation is the default; `--perceptual-outside --maximum-outside-flip 0.01 --ppd 67` declares a different complement policy. Exit 1 means collateral change or missing intended spatial change. No result establishes semantic success of the edit. HDR/float inputs are unsupported.

DOM metadata is supplied by the capture producer:

```json
{
  "schema": "saccade-dom-regions.v1", "reference_sha256": "LOWERCASE_FILE_HASH", "dimensions": [800, 600],
  "selectors": [{"selector": "main .card", "boxes": [[80, 120, 300, 200]]}]
}
```

Bind geometry to the reference screenshot bytes, accounting for scroll, screenshot crop and device scale. A missing or multiply matched selector is unmeasurable. For Playwright, attach this JSON as `saccade-dom-regions-0` (body or path); the reporter adds `dom_regions`, and ingest retains it in `dom-regions/0000.json`. Use the ingested baseline PNG and that metadata with `localized-check`. The reference region is retained even if the candidate object disappears. Configuration masks cannot hide collateral from this diagnostic.

## Grounded explanations

```sh
saccade explain-grounded --report report/saccade-report.v1.json --out explanation.json
saccade explain-grounded --report localized-report/localized.json --proposals atomic-proposals.json --out verified.json --json
```

The default uses deterministic atomic templates. An optional proposal file is a JSON array, for example:

```json
[{"claim_kind": "mean_flip", "region_ids": ["e0.full"], "evidence_ids": ["e0.full.mean_flip"], "value": 0.01}]
```

Use exact IDs and numbers from the catalog. Supported kinds are `mean_flip`, `max_flip`, `thresholded_hotspot_pixels`, and localized `changed_pixels`. Unsupported semantic/causal observations, compound citations and inconsistent quantities are dropped. Free-form wording is not accepted. A model may supply this finite contract offline; the application performs the same verification and inserts the final wording. Source hashes and JSON pointers let reviewers resolve each fact. Pixel counts in hotspots depend on thresholds/exclusions; localized counts use native samples.

MCP: call `saccade_inspect` with `operation: "grounded"`, `artifact` pointing to a comparison report, and optional `limit` (1–5) and numeric `cursor`. Each page carries accepted claims and the matching region/fact/source citations. Numerical support does not establish semantic success, causation, or a human review decision.

## Regions described in words

Text-to-mask inference remains unavailable in this lane; `saccade regions status` states this explicitly. Freeze a reviewed phrase mask without model dependencies:

```sh
saccade regions import --reference reference.png --mask left-sphere.png --phrase 'the left sphere' --out left-sphere.json
saccade localized-check reference.png candidate.png --region left-sphere.json --out measured-region
```

The mask must be binary grayscale, reference-sized, 255 inside. The original phrase, explicit import, source-mask hash and reference hash are saved before measurement. Use the same reference region if the candidate object disappears. Spatial measurement does not verify that the phrase selected the correct object.

Optional plumbing: `cargo build -p saccade --features semantic-regions`. The pinned checkpoint manifest is [semantic-regions.json](../crates/saccade-core/models/semantic-regions.json). No weights or ONNX Runtime binaries ship with the repository.

```sh
saccade regions cache --manifest crates/saccade-core/models/semantic-regions.json --cache ~/.cache/saccade/models
saccade regions runtime-probe --manifest qualified-exports.json --cache ~/.cache/saccade/models --library /path/to/libonnxruntime.so
```

Only `cache` performs explicit downloads; it verifies hashes and exact byte counts before storing artifacts. Cache hits are verified again. The shipped manifest contains checkpoints, so runtime probing rejects it until detector/SAM encoder/SAM decoder self-contained ONNX artifacts are supplied. Runtime 1.22, CPU f32, pinned preprocessing/tokenizer and checkpoint/export/license provenance must be recorded. Graph loading is diagnostic plumbing and does not establish source-model parity or successful text segmentation. The export, inference and accuracy residuals are recorded in the design decisions.

## RenderDoc divergence localization

Run the optional worker explicitly on a qualified Vulkan replay host with RenderDoc 1.34 official Python bindings available. Replay uses the GPU/driver even without the GUI.

```sh
python3 scripts/renderdoc-worker.py base.rdc --out baseline-replay
python3 scripts/renderdoc-worker.py candidate.rdc --out candidate-replay
saccade renderdoc-localize baseline-replay/extraction.json candidate-replay/extraction.json --out replay-localization.json --json
```

Worker output contains native payloads, event/resource witnesses and exact capture/worker hashes. It requires a new directory, does two independent replays by default, and records unavailable capability with exit 2 when bindings/version/device/replay are unavailable. `--single-replay` retains unqualified repeatability. `--max-bytes` bounds extraction (default 512 MiB, maximum 2 GiB); individual resources are bounded to 64 MiB and actions to 2048. Only one bound texture mip/layer/sample is extracted; buffers are read in full. Other resources remain unobserved.

Rust verifies the payloads and aligns unique marker/action signatures with gaps. Repeated markers, duplicate action signatures and unmarked actions remain ambiguous. It compares native bytes under matching resource roles/format/dimensions/subresources, including compute writable resources. It scans every aligned event, so overwritten intermediate differences remain visible. Exit 2 means incomplete correspondence or evidence, exit 1 means complete evidence with an observed divergence, and exit 0 means complete extracted evidence with equal bytes under this declared scope. These exits grant no acceptance or root-cause authority.

Read `first_observed_divergence`, all observations, gaps and `candidate_inputs`. A bad upload may precede its first visible effect, and a later clear may erase a difference. Confirm final-output relevance and cause with additional resource tracing and a controlled intervention. The current host has no official bindings; only synthetic Rust-side fixtures and the worker's unavailable-capability path were validated. No Vulkan, D3D12, GL or browser replay qualification is claimed.

For long histories, retain the full selected-group witness with `history analyze --store .saccade-history --entry article.png --drift --out history-analysis.json --json`. Standard output is a bounded preview with explicit group/run omission counts; the new file retains every run of the selected groups. Declared trials contribute only to `run_analysis`; artifact-only tolerance advice remains confined to older rows without independent-run declarations.
