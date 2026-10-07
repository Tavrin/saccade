# Command reference

Generated from compiled capabilities and `--help`; do not edit by hand.

Generation: `cargo build --release -p saccade --all-features`, then `python3 scripts/gen-docs.py --saccade target/release/saccade`.
The all-features binary includes every supported operation.

Compiled features: `ai`, `assist`, `compression`, `credentials`, `dense-motion`, `documents`, `embeddings`, `evaluation`, `geometry`, `graphics`, `imgtune-avif`, `local-models`, `local-vlm`, `mcp`, `media-http`, `ocr`, `ocr-provider`, `parallel`, `prechecks`, `print`, `products`, `schema`, `semantic-regions`, `text-quality`, `vision-providers`, `workbench`.

Exit 1 means a failed image measurement/evaluation gate or located divergence.
Exit 0 for compare/identity means no image regression; inspect `performance` for qualification.
Inspection, review, rank and ablation completion grant no acceptance authority.
Exit 2 means the operation cannot run. Demo intentionally exits 1.

## saccade

```text
Tell when visual or performance evidence is not good enough to support a claim

Usage: saccade [OPTIONS] <COMMAND>

Commands:
  compare             Compare a directory of captures against a directory of baselines
  prove               Check whether image identity or performance evidence proves a claim
  review              Preview a review plan or handle a local closed decision request
  manifest            Find, link and re-check the outputs of a report directory
  export-regions      Crop the worst regions of a report, with coordinates
  print               ICC-managed CMYK raster comparison (first-party print extension)
  timing              Verdicts over timings acquired by external tools
  mask-metrics        Overlap and boundary metrics between two integer label images
  boxes               Export, import and transform bounding boxes as COCO or YOLO
  frame-map           Check the index, timestamp and file map of externally extracted frames
  render-evidence     Compare structural rendering evidence with explicit scope and ID attribution
  schema              Discover JSON Schemas without a source checkout
  perf                Validate producer performance sidecars
  arms                Validate producer identity before comparing pixels
  sweep               Plan and compare deterministic page sweeps
  imgtune             Audit delivery formats and search perceptual-target encodings
  design              Pull design-source frames and compare implementation captures
  notify              Send a generic report summary to a user-configured webhook
  capabilities        List comparison questions, inputs, features and honest availability
  inspect-image       Inspect provenance/integrity indicators without a real/fake verdict
  assess              Measure content-dependent no-reference quality indicators
  text                Compare image-bound OCR/text observations and literal expected strings
  tofu                Triage pixel shapes resembling missing glyphs (requires text-quality)
  text-legibility     Measure text legibility across supplied variants (requires text-quality)
  similar             Cosine similarity with an explicitly pinned optional ONNX export
  index               Build or query a streaming exact flat embedding index
  hash                Compute perceptual hashes without changing originals
  dedupe              Cluster near-duplicates with bounded Hamming search; never delete images
  analyze-media       Analyze an image into a versioned media record (no model downloads by default)
  keyframes           Extract shot representatives with timestamps, without linking a video decoder
  find-usage          Match an image or media record against generic target images
  models              List or explicitly pull pinned local models
  locate              Locate a phrase with boxes, optional masks, and an overlay PNG
  quality-score       Measure a separately named learned quality score
  watermark           Decode explicitly compatible watermark schemes without an origin verdict
  faces               Detect faces and optionally create a privacy-redacted PNG
  crop-check          Assess declared crops against detected faces, without identity recognition
  observe-local       Bounded advisory observations from an explicitly configured local VLM
  provider-map        Map provider requests or decode recorded vision responses; no live calls
  renderdoc-localize  Align optional Vulkan replay evidence and locate native-resource divergence
  regions             Import and freeze phrase regions, or inspect optional model plumbing
  explain-grounded    Render verified atomic numerical claims with region and evidence citations
  localized-check     Measure intended-region, boundary and protected-complement changes independently
  inventory           Reconcile expected and supplied stable capture cases against a comparison report
  quality-sweep       Measure externally encoded quality candidates under a frozen score and byte budget

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help
  -V, --version                      Print version

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output


Start here:
  saccade compare baseline/ captures/ --out report
  saccade prove identity parent/ candidate/ --out proof
  saccade prove performance --base 'base_r*' --arm 'candidate=candidate_r*'
  saccade review report/saccade-report.v1.json --out review

Tasks (full map and guides: docs/quickstart.md, docs/guides/):
  Did a render or screenshot change?      saccade compare
  Is a refactor pixel-identical?          saccade prove identity
  Did it get faster, accounting noise?    saccade prove performance
  Is a timing from another tool real?     saccade timing
  Are two capture setups comparable?      saccade arms check
  Which images are near-duplicates?       saccade dedupe
  What does a finished report say?        saccade inspect, saccade review
Advanced: demo, identity, noise, view, inspect, experiment (incl. settle), timing, approve, init,
serve, mcp, ingest, bisect, history, doctor. Existing commands keep working; use `saccade COMMAND --help`.
`saccade doctor` lists what this build and machine can run.

Exit codes (a command that cannot produce a measurement never exits 0):
  0  success: no regression, claim proven, or the requested output was written
  1  regression found, or the claim was not proven (differs, missing, new, unreadable)
  2  the command could not run: usage, config, input or unavailable feature/model
  3  strict producer check refused: an undeclared difference (--require-valid-arms)
  4  strict producer check refused: a required key is missing (--require-valid-arms)
Units: --threshold on FLIP scores is a 0-1 score (lower = more alike); hash thresholds count bits.
```

## saccade manifest

```text
Find, link and re-check the outputs of a report directory

Usage: saccade manifest [OPTIONS] <COMMAND>

Commands:
  build     Write saccade-manifest.json for a report directory: artifacts by hash, reports by report_id, duplicates listed once. Passing is never recorded as approval
  verify    Re-hash everything a manifest or link names; fails with `link_missing` or `stale_link` when a recorded file moved or changed
  link      Write a stable link to one report of a directory, by report_id
  classify  Say whether a path is a report directory, a JSON document or an API response

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade manifest build

```text
Write saccade-manifest.json for a report directory: artifacts by hash, reports by report_id, duplicates listed once. Passing is never recorded as approval

Usage: saccade manifest build [OPTIONS] <DIR>

Arguments:
  <DIR>  The output directory to describe

Options:
      --approved-anchor <APPROVED_ANCHOR>
          Record a baseline a human approved (separate from last-good)
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --last-good <LAST_GOOD>
          Record the last passing run (a different role; not approval)
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --json
          Print a JSON result
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade manifest verify

```text
Re-hash everything a manifest or link names; fails with `link_missing` or `stale_link` when a recorded file moved or changed

Usage: saccade manifest verify [OPTIONS] <TARGET>

Arguments:
  <TARGET>  A report directory, a saccade-manifest.json or a saccade-link.json

Options:
      --json                         Print a JSON result
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade manifest link

```text
Write a stable link to one report of a directory, by report_id

Usage: saccade manifest link [OPTIONS] --report-id <REPORT_ID> --out <OUT> <DIR>

Arguments:
  <DIR>  A report directory that has a manifest

Options:
      --report-id <REPORT_ID>        The report_id to link (see `reports` in the manifest)
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>                    Where to write the link document
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --json                         Print a JSON result
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade manifest classify

```text
Say whether a path is a report directory, a JSON document or an API response

Usage: saccade manifest classify [OPTIONS] <PATH>

Arguments:
  <PATH>  Path to inspect

Options:
      --json                         Print JSON (the default output is already one line of JSON)
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade export-regions

```text
Crop the worst regions of a report, with coordinates

Usage: saccade export-regions [OPTIONS] <REPORT>

Arguments:
  <REPORT>  A saccade report JSON (saccade-report.v1 or its linked successor)

Options:
      --out <OUT>                    Output directory for the crops and the coordinates document [default: regions-export]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --top <TOP>                    How many regions to export, worst first (1 to 200) [default: 5]
      --padding <PADDING>            Context pixels added around each hotspot box [default: 8]
      --json                         Print a JSON result
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade print

```text
ICC-managed CMYK raster comparison (first-party print extension)

Usage: saccade print [OPTIONS] --out <OUT> --tac-limit <TAC_LIMIT> --dpi <DPI> <REFERENCE> <CANDIDATE>

Arguments:
  <REFERENCE>  Reference CMYK TIFF/JPEG or supported single-image PDF raster
  <CANDIDATE>  Candidate CMYK TIFF/JPEG or supported single-image PDF raster

Options:
      --out <OUT>
          Empty artifact output directory
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --input-profile <INPUT_PROFILE>
          Override both embedded input ICC profiles with this CMYK ICC
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --output-profile <OUTPUT_PROFILE>
          Target CMYK output ICC for gamut diagnostics
      --tac-limit <TAC_LIMIT>
          Declared total area coverage limit, percent (0..400)
      --dpi <DPI>
          Declared physical raster density for small-mark detection
      --small-text-points <SMALL_TEXT_POINTS>
          Maximum text-like component height, points [default: 12]
      --json
          Emit the versioned, linked measurement as JSON
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade timing

```text
Verdicts over timings acquired by external tools

Usage: saccade timing [OPTIONS] <COMMAND>

Commands:
  ab  Analyze external paired timings; no commands are executed

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade timing ab

```text
Analyze external paired timings; no commands are executed

Usage: saccade timing ab [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Session manifest or paired CSV

Options:
      --format <FORMAT>              session or csv; referenced runs support hyperfine, perf and JSON paths [default: session]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>                    [default: timing-ab]
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --json
      --band-pct <BAND_PCT>          Override practical band for CSV imports (manifest policies otherwise retained)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade mask-metrics

```text
Overlap and boundary metrics between two integer label images

Usage: saccade mask-metrics [OPTIONS] <PREDICTED> <REFERENCE>

Arguments:
  <PREDICTED>  Predicted label image (single-channel native labels, packed RGB or integer EXR)
  <REFERENCE>  Reference label image of the same size

Options:
      --class <CLASSES>              Class as NAME=PREDICATE (id=, range=, above=, mask); repeatable. Default: one `foreground` class, label not zero
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --each-label                   Score every distinct non-void label as its own class (at most 256)
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --void <VOID>                  Reference labels to exclude everywhere, as NAME=PREDICATE
      --boundary-px <BOUNDARY_PX>    Boundary match tolerance in pixels (0-64, Euclidean, inclusive) [default: 2]
      --out <OUT>                    Write saccade-mask-metrics.v1.json into this new or empty directory
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade boxes

```text
Export, import and transform bounding boxes as COCO or YOLO

Usage: saccade boxes [OPTIONS] <COMMAND>

Commands:
  export     Export a saccade-boxes.v1 document as COCO JSON or YOLO text
  import     Import COCO JSON or YOLO text into a saccade-boxes.v1 document
  transform  Re-express boxes for a cropped or resized copy of the image

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade boxes export

```text
Export a saccade-boxes.v1 document as COCO JSON or YOLO text

Usage: saccade boxes export [OPTIONS] --format <FORMAT> --out <OUT> <DOC>

Arguments:
  <DOC>  saccade-boxes.v1 document

Options:
      --format <FORMAT>              [possible values: coco, yolo]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>                    New or empty output directory
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --clip                         Clip boxes that leave the image (counted) instead of refusing them
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade boxes import

```text
Import COCO JSON or YOLO text into a saccade-boxes.v1 document

Usage: saccade boxes import [OPTIONS] --format <FORMAT> --out <OUT> <INPUT>

Arguments:
  <INPUT>

Options:
      --format <FORMAT>              [possible values: coco, yolo]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --width <WIDTH>                YOLO only: image width in pixels
      --height <HEIGHT>              YOLO only: image height in pixels
      --image-file <IMAGE_FILE>      YOLO only: image file name to record
      --classes <CLASSES>            YOLO only: classes.txt, one name per line
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade boxes transform

```text
Re-express boxes for a cropped or resized copy of the image

Usage: saccade boxes transform [OPTIONS] --out <OUT> <DOC>

Arguments:
  <DOC>

Options:
      --crop <CROP>                  Crop window X,Y,W,H in source pixels
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --resize <RESIZE>              Resized image size W,H (each axis scaled independently)
      --image-file <IMAGE_FILE>      File name of the derived image to record
      --out <OUT>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade frame-map

```text
Check the index, timestamp and file map of externally extracted frames

Usage: saccade frame-map [OPTIONS] <COMMAND>

Commands:
  check  Report gaps, constant or variable rate, file integrity and settling in the map's own timestamps

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade frame-map check

```text
Report gaps, constant or variable rate, file integrity and settling in the map's own timestamps

Usage: saccade frame-map check [OPTIONS] <MAP>

Arguments:
  <MAP>  saccade-frame-map.v1 document

Options:
      --root <ROOT>
          Directory the map's relative paths resolve against (default: the map's directory)
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --skip-files
          Do not look at frame files; check the index and timestamps only
      --rate-tolerance-pct <RATE_TOLERANCE_PCT>
          Relative spread of the per-frame step, in percent, still called constant [default: 1]
      --settling <SETTLING>
          saccade-settling.v1 report for the same frames, to restate settling in map time
      --out <OUT>
          Write saccade-frame-map-check.v1.json into this new or empty directory
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade render-evidence

```text
Compare structural rendering evidence with explicit scope and ID attribution

Usage: saccade render-evidence [OPTIONS] <BASELINE> <CANDIDATE>

Arguments:
  <BASELINE>
  <CANDIDATE>

Options:
      --out <OUT>
          [default: render-evidence]
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --config <CONFIG>

      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --json

      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED>

      --export-maps
          Export native FLIP and tile grids as float32 NPY/EXR with a JSON index
      --require-scope
          Require an actual ID layer or nonempty mask scope
      --noise-from <REPEAT> <REPEAT>...
          Same-arm repeat files or run directories (2..32); enables noise-aware deciding evidence
      --mask-dump <MASK_DUMP>
          Generic screen-space dump filename relative to each capture; used when no layer manifest exists
      --mask-layer <MASK_LAYER>
          Named layer and native predicate, NAME=id=1,2 or NAME=label=pattern
      --require-effect <REQUIRE_EFFECT>
          Required occupancy from NAME=predicate[:MIN_PIXELS] or mask:FILE[:MIN_PIXELS]
      --id-top <ID_TOP>
          Per-ID rows and diagnostic crops retained, at most 32
      --id-threshold <ID_THRESHOLD>
          Declared normalized luminance threshold for colour per-ID statistics
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade schema

```text
Discover JSON Schemas without a source checkout

Usage: saccade schema [OPTIONS] <COMMAND>

Commands:
  list  List all schema IDs shipped in this binary
  get   Print the exact embedded JSON Schema (or create a new file)
  path  Locate an installed copy, if available; use get for portable discovery

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade schema list

```text
List all schema IDs shipped in this binary

Usage: saccade schema list [OPTIONS]

Options:
      --json
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade schema get

```text
Print the exact embedded JSON Schema (or create a new file)

Usage: saccade schema get [OPTIONS] <ID>

Arguments:
  <ID>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade schema path

```text
Locate an installed copy, if available; use get for portable discovery

Usage: saccade schema path [OPTIONS] <ID>

Arguments:
  <ID>

Options:
      --json
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade perf

```text
Validate producer performance sidecars

Usage: saccade perf [OPTIONS] <COMMAND>

Commands:
  validate  Validate a perf v1 or v2 sidecar against the exact embedded JSON Schema

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade perf validate

```text
Validate a perf v1 or v2 sidecar against the exact embedded JSON Schema

Usage: saccade perf validate [OPTIONS] <SIDECAR>

Arguments:
  <SIDECAR>

Options:
      --json
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade arms

```text
Validate producer identity before comparing pixels

Usage: saccade arms [OPTIONS] <COMMAND>

Commands:
  check  Check two capture records, sidecars, images or capture directories

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade arms check

```text
Check two capture records, sidecars, images or capture directories

Usage: saccade arms check [OPTIONS] <A> <B>

Arguments:
  <A>
  <B>

Options:
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --vary <VARY>
          Allowed difference matching destination or mapped source: exact key, dotted prefix, suffix or glob; repeat or comma-separate
      --allow-unreached <ALLOW_UNREACHED>
          Permit intentionally unreached captures with exactly matching observations
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --ignore <IGNORE>
          Explicit exception matching destination or mapped source, echoed even if unmatched; repeat or comma-separate
      --fingerprint-map <FINGERPRINT_MAP>

      --max-record-bytes <MAX_RECORD_BYTES>
          Fingerprint record limit in bytes (default 16 MiB; hard ceiling 64 MiB); overrides map
      --compare <COMPARE>
          Override the map's field selection (mapped-only requires a map) [possible values: mapped-only, all]
      --config <CONFIG>

      --meta-name <META_NAME>

      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade sweep

```text
Plan and compare deterministic page sweeps

Usage: saccade sweep [OPTIONS] <COMMAND>

Commands:
  plan     Sample a URL list or bounded sitemap tree into a driver-neutral manifest
  compare  Compare every planned capture, including explicit failure receipts

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade sweep plan

```text
Sample a URL list or bounded sitemap tree into a driver-neutral manifest

Usage: saccade sweep plan [OPTIONS] --before-origin <BEFORE_ORIGIN> --after-origin <AFTER_ORIGIN> --out <OUT>

Options:
      --source-ref <SOURCE_REF>        External capture URI/key (repeatable); recorded in generated reports
      --urls <URLS>
      --report-index <REPORT_INDEX>    Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --sitemap <SITEMAP>
      --before-origin <BEFORE_ORIGIN>
      --after-origin <AFTER_ORIGIN>
      --samples <SAMPLES>              Samples per candidate setting, 1-100 (default 3) [default: 3]
      --seed <SEED>                    Integer seed for sample order (default 42) [default: 42]
      --viewport <VIEWPORT>            Repeat WIDTHxHEIGHT. Default: 1280x720
      --out <OUT>
      --json
  -h, --help                           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade sweep compare

```text
Compare every planned capture, including explicit failure receipts

Usage: saccade sweep compare [OPTIONS] --captures <CAPTURES> --out <OUT> <MANIFEST>

Arguments:
  <MANIFEST>

Options:
      --captures <CAPTURES>
      --source-ref <SOURCE_REF>        External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>
      --report-index <REPORT_INDEX>    Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --config <CONFIG>
      --baseline <BASELINE>            [possible values: last-good]
      --history-store <HISTORY_STORE>
      --align <ALIGN>                  Explicit registration; incompatible with ordinary comparison config [possible values: none, translation, similarity, affine, homography, auto]
      --resample <RESAMPLE>            [possible values: reference, common]
      --json
  -h, --help                           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade imgtune

```text
Audit delivery formats and search perceptual-target encodings

Usage: saccade imgtune [OPTIONS] <COMMAND>

Commands:
  audit   Record actual HTTP content negotiation, bytes and decoded dimensions
  search  Search a finite quality/format grid, retaining source and delivery evidence

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade imgtune audit

```text
Record actual HTTP content negotiation, bytes and decoded dimensions

Usage: saccade imgtune audit [OPTIONS] --urls <URLS> --accept <ACCEPT> --out <OUT>

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --urls <URLS>
      --accept <ACCEPT>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --out <OUT>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade imgtune search

```text
Search a finite quality/format grid, retaining source and delivery evidence

Usage: saccade imgtune search [OPTIONS] --out <OUT> <MANIFEST>

Arguments:
  <MANIFEST>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade design

```text
Pull design-source frames and compare implementation captures

Usage: saccade design [OPTIONS] <COMMAND>

Commands:
  pull     Export mapped design frames and tokens, cached by file version
  compare  Compare mapped frames and implementation captures; retain expected layout differences

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade design pull

```text
Export mapped design frames and tokens, cached by file version

Usage: saccade design pull [OPTIONS] --out <OUT> <MAPPING>

Arguments:
  <MAPPING>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --cache <CACHE>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --fixture-dir <FIXTURE_DIR>
      --scale <SCALE>                Export scale factor, 0.01-4 (default 1; 2 = twice the pixel size) [default: 1]
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade design compare

```text
Compare mapped frames and implementation captures; retain expected layout differences

Usage: saccade design compare [OPTIONS] --pull <PULL> --captures <CAPTURES> --out <OUT> <MAPPING>

Arguments:
  <MAPPING>

Options:
      --pull <PULL>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --captures <CAPTURES>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --out <OUT>
      --config <CONFIG>
      --align <ALIGN>                [default: translation] [possible values: none, translation]
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade notify

```text
Send a generic report summary to a user-configured webhook

Usage: saccade notify [OPTIONS] <REPORT>

Arguments:
  <REPORT>  Full report, sweep report or design report, read locally

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --template <TEMPLATE>          [default: generic] [possible values: generic, slack, teams]
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --report-link <REPORT_LINK>    Display link; defaults to the report path. Never used as the webhook endpoint
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade capabilities

```text
List comparison questions, inputs, features and honest availability

Usage: saccade capabilities [OPTIONS]

Options:
      --json
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect-image

```text
Inspect provenance/integrity indicators without a real/fake verdict

Usage: saccade inspect-image [OPTIONS] <IMAGE>

Arguments:
  <IMAGE>

Options:
      --faces
          Run local face detection; never downloads models implicitly
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --face-observations <FACE_OBSERVATIONS>
          Image-bound face receipt; explicitly labelled replay
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --model-registry <MODEL_REGISTRY>
          Shared model registry (vision, embedding and OCR pins)
      --model-cache <MODEL_CACHE>
          Deprecated: set SACCADE_MODELS_DIR or [models].dir
      --runtime-library <RUNTIME_LIBRARY>
          Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library
      --watermark
          Inspect named watermark decoders; unavailable decoders stay explicit
      --watermark-payload <WATERMARK_PAYLOAD>
          Known legacy DWT message bytes in hex; arbitrary bits are not detection
      --face-crop <FACE_CROP>
          Declared face-protection crop in original pixels: X,Y,W,H
      --include-gps
          Explicitly include unsigned EXIF GPS coordinates in the report
      --hash-index <HASH_INDEX>
          Local saccade-hash.v1 / saccade-dedupe.v1 archive for candidate lookup
      --output-size <OUTPUT_SIZE>
          Declared publication output size, WIDTHxHEIGHT; repeatable
      --crop <CROP>
          Crop x,y,width,height in raw raster pixels
      --text-source <TEXT_SOURCE>
          Optional image-bound source/OCR observations for legibility evidence
      --ocr
          Extract legibility observations with the default PP-OCRv5 engine
      --ocr-contract <OCR_CONTRACT>
          Explicit pinned OCR contract override
      --out <OUT>
          [default: image-inspection]
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade assess

```text
Measure content-dependent no-reference quality indicators

Usage: saccade assess [OPTIONS] <IMAGE>

Arguments:
  <IMAGE>

Options:
      --faces
          Run local face detection; never downloads models implicitly
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --face-observations <FACE_OBSERVATIONS>
          Image-bound face receipt; explicitly labelled replay
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --model-registry <MODEL_REGISTRY>
          Shared model registry (vision, embedding and OCR pins)
      --model-cache <MODEL_CACHE>
          Deprecated: set SACCADE_MODELS_DIR or [models].dir
      --runtime-library <RUNTIME_LIBRARY>
          Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library
      --watermark
          Inspect named watermark decoders; unavailable decoders stay explicit
      --watermark-payload <WATERMARK_PAYLOAD>
          Known legacy DWT message bytes in hex; arbitrary bits are not detection
      --face-crop <FACE_CROP>
          Declared face-protection crop in original pixels: X,Y,W,H
      --compare-to <COMPARE_TO>
          Reference quality measurements; deltas are image minus compare-to
      --out <OUT>
          [default: assessment-report]
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade text

```text
Compare image-bound OCR/text observations and literal expected strings

Usage: saccade text [OPTIONS] <A> <B>

Arguments:
  <A>
  <B>

Options:
      --ocr-provider <OCR_PROVIDER>
          Optional document OCR provider; selecting it exports images/PDFs only with --ocr-run [possible values: mistral]
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --ocr-model <OCR_MODEL>
          Explicit dated OCR model (aliases refused)
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --ocr-pages <OCR_PAGES>
          Zero-based pages selected explicitly; images accept only 0 [default: 0]
      --ocr-responses <OCR_RESPONSES> <OCR_RESPONSES>
          Constructed/recorded request-bound fixture envelopes for both inputs; no network
      --ocr-run
          Explicitly authorize live document export under existing root policy
      --ocr-max-spend-usd <OCR_MAX_SPEND_USD>
          Finite overall monetary cap; unestablished usage keeps the full reservation
      --ocr-price-per-page-usd <OCR_PRICE_PER_PAGE_USD>
          User-confirmed conservative per-selected-page billing ceiling
      --ocr-price-policy <OCR_PRICE_POLICY>
          User-owned price-policy revision; no built-in unverified pricing
      --ocr-user-config <OCR_USER_CONFIG>
          Existing human-owned user.toml with explicit egress roots and Mistral credential binding
      --a-source <A_SOURCE>
          Image-bound imported saccade-ui-source.v1 observations for the reference
      --b-source <B_SOURCE>
          Image-bound imported observations for the candidate
      --ocr-contract <OCR_CONTRACT>
          Override the default pinned PP-OCRv5 contract (or select external Tesseract). Deprecated for registries: a configured registry (SACCADE_MODELS_REGISTRY / [models].registry) with one OCR contract is used automatically
      --download-model
          Deprecated: provision with `saccade models pull ocr`. Still fetches the SHA-pinned Rust OCR models into the contract cache
      --expect-text <EXPECT_TEXT>
          Literal Unicode strings expected in the candidate (repeatable); always inert data
      --readable-confidence <READABLE_CONFIDENCE>
          OCR confidence cutoff for the readability observation; not a calibrated probability [default: 80]
      --moved-px <MOVED_PX>
          Movement threshold in reference pixels after dimension normalization [default: 3]
      --out <OUT>
          [default: text-report]
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade tofu

```text
Triage pixel shapes resembling missing glyphs (requires text-quality)

Usage: saccade tofu [OPTIONS] <IMAGE>

Arguments:
  <IMAGE>

Options:
      --mask <MASK>                    Binary text-region mask: nonzero red includes; dimensions must match
      --source-ref <SOURCE_REF>        External capture URI/key (repeatable); recorded in generated reports
      --expected-text <EXPECTED_TEXT>  Declared expected Unicode text, never interpreted as instructions
      --report-index <REPORT_INDEX>    Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --source <SOURCE>                Image-bound imported OCR observations (saccade-ui-source.v1)
      --ocr                            Use cached default PaddleOCR; never downloads
      --out <OUT>                      Optional report directory; --json always emits the full versioned report
      --json
  -h, --help                           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade text-legibility

```text
Measure text legibility across supplied variants (requires text-quality)

Usage: saccade text-legibility [OPTIONS] --region <REGION> <BASELINE> <VARIANTS>...

Arguments:
  <BASELINE>
  <VARIANTS>...  One or more variant captures, in report order

Options:
      --region <REGION>
          Baseline capture-pixel rectangle x,y,width,height (repeatable, max 64)
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --minimum-contrast <MINIMUM_CONTRAST>
          [default: 4.5]
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --minimum-x-height-px <MINIMUM_X_HEIGHT_PX>
          [default: 8]
      --minimum-sharpness <MINIMUM_SHARPNESS>
          [default: 0.35]
      --minimum-stroke-px <MINIMUM_STROKE_PX>
          [default: 1]
      --ocr
          Use cached default PaddleOCR on baseline and variants; never downloads
      --baseline-source <BASELINE_SOURCE>
          Imported OCR for baseline, paired with one --variant-source per variant
      --variant-source <VARIANT_SOURCE>

      --out <OUT>

      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade similar

```text
Cosine similarity with an explicitly pinned optional ONNX export

Usage: saccade similar [OPTIONS] <A> <B>

Arguments:
  <A>
  <B>

Options:
      --model <MODEL>                Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing. Default: SACCADE_MODELS_EMBEDDING_CONTRACT or [models].embedding_contract
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --cache <CACHE>                Deprecated: set SACCADE_MODELS_DIR or [models].dir. Content-addressed model cache
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --library <LIBRARY>            Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. Explicit ONNX Runtime 1.22 dynamic library, CPU execution only
      --download-model               Deprecated: provision with `saccade models pull embedding`. Still downloads the pinned export to the cache
      --out <OUT>                    [default: similar-report]
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index

```text
Build or query a streaming exact flat embedding index

Usage: saccade index [OPTIONS] <COMMAND>

Commands:
  export         Export external report cross-links
  export-inputs  Write exact Rust-preprocessed tensors for independent checkpoint/export parity
  calibrate      Run pinned export parity and fit/holdout calibration over a frozen corpus (heavy)
  build          Build an exact index; --segmented supports larger archives and incremental updates
  update         Add/replace changed sources in an existing index, optionally pruning missing paths
  query          Search an existing index; model/preprocessing must exactly match the index

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index export

```text
Export external report cross-links

Usage: saccade index export [OPTIONS]

Options:
      --index <INDEX>                [default: reports/index.jsonl]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --format <FORMAT>              [default: jsonl]
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --out <OUT>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index export-inputs

```text
Write exact Rust-preprocessed tensors for independent checkpoint/export parity

Usage: saccade index export-inputs [OPTIONS] --model <MODEL> --out <OUT> <DIR>

Arguments:
  <DIR>

Options:
      --model <MODEL>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index calibrate

```text
Run pinned export parity and fit/holdout calibration over a frozen corpus (heavy)

Usage: saccade index calibrate [OPTIONS] --out <OUT> <CORPUS>

Arguments:
  <CORPUS>

Options:
      --model <MODEL>                Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing. Default: SACCADE_MODELS_EMBEDDING_CONTRACT or [models].embedding_contract
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --cache <CACHE>                Deprecated: set SACCADE_MODELS_DIR or [models].dir. Content-addressed model cache
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --library <LIBRARY>            Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. Explicit ONNX Runtime 1.22 dynamic library, CPU execution only
      --download-model               Deprecated: provision with `saccade models pull embedding`. Still downloads the pinned export to the cache
      --out <OUT>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index build

```text
Build an exact index; --segmented supports larger archives and incremental updates

Usage: saccade index build [OPTIONS] --out <OUT> <DIR>

Arguments:
  <DIR>

Options:
      --segmented                    Use durable v2 segments (up to 1000000 images and 16 GiB vectors)
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --model <MODEL>                Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing. Default: SACCADE_MODELS_EMBEDDING_CONTRACT or [models].embedding_contract
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --cache <CACHE>                Deprecated: set SACCADE_MODELS_DIR or [models].dir. Content-addressed model cache
      --library <LIBRARY>            Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. Explicit ONNX Runtime 1.22 dynamic library, CPU execution only
      --download-model               Deprecated: provision with `saccade models pull embedding`. Still downloads the pinned export to the cache
      --out <OUT>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index update

```text
Add/replace changed sources in an existing index, optionally pruning missing paths

Usage: saccade index update [OPTIONS] <INDEX> <DIR>

Arguments:
  <INDEX>
  <DIR>

Options:
      --prune                        Treat dir as the complete archive and remove absent sources
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --model <MODEL>                Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing. Default: SACCADE_MODELS_EMBEDDING_CONTRACT or [models].embedding_contract
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --cache <CACHE>                Deprecated: set SACCADE_MODELS_DIR or [models].dir. Content-addressed model cache
      --library <LIBRARY>            Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. Explicit ONNX Runtime 1.22 dynamic library, CPU execution only
      --download-model               Deprecated: provision with `saccade models pull embedding`. Still downloads the pinned export to the cache
      --out <OUT>                    [default: index-update-report]
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index query

```text
Search an existing index; model/preprocessing must exactly match the index

Usage: saccade index query [OPTIONS] <INDEX> [IMAGE]

Arguments:
  <INDEX>
  [IMAGE]

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --text <TEXT>                  Text query requires a pinned SigLIP 2 joint text/image model
      --model <MODEL>                Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing. Default: SACCADE_MODELS_EMBEDDING_CONTRACT or [models].embedding_contract
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --cache <CACHE>                Deprecated: set SACCADE_MODELS_DIR or [models].dir. Content-addressed model cache
      --library <LIBRARY>            Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. Explicit ONNX Runtime 1.22 dynamic library, CPU execution only
      --download-model               Deprecated: provision with `saccade models pull embedding`. Still downloads the pinned export to the cache
      --top <TOP>                    Number of nearest matches to return (default 10) [default: 10]
      --out <OUT>                    [default: query-report]
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade hash

```text
Compute perceptual hashes without changing originals

Usage: saccade hash [OPTIONS] <FILES>...

Arguments:
  <FILES>...  Files or directories; each unique input is decoded once

Options:
      --out <OUT>                    New or empty output directory [default: hash-report]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json                         Emit a bounded JSON artifact receipt
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade dedupe

```text
Cluster near-duplicates with bounded Hamming search; never delete images

Usage: saccade dedupe [OPTIONS] <DIR>

Arguments:
  <DIR>  Directory of images; never deletes originals

Options:
      --algorithm <ALGORITHM>        Algorithm for the Hamming index [default: phash] [possible values: ahash, dhash, phash]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --threshold <THRESHOLD>        Largest perceptual-hash distance in bits, 0-64 (0 = identical hashes; larger = looser); clusters use transitive connectivity [default: 6]
      --out <OUT>                    New or empty output directory [default: dedupe-report]
      --json                         Emit a bounded JSON artifact receipt
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade analyze-media

```text
Analyze an image into a versioned media record (no model downloads by default)

Usage: saccade analyze-media [OPTIONS] <SOURCE>

Arguments:
  <SOURCE>

Options:
      --profile <PROFILE>            [default: cpu-lite] [possible values: cpu-lite, cpu-full, gpu]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --model-dir <MODEL_DIR>        Deprecated: set SACCADE_MODELS_DIR or [models].dir (see `saccade models config`)
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --registry <REGISTRY>          Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry
      --options <OPTIONS>            Per-section options JSON file
      --strict
      --output-size <OUTPUT_SIZE>    Repeat output size WxH
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade keyframes

```text
Extract shot representatives with timestamps, without linking a video decoder

Usage: saccade keyframes [OPTIONS] --out <OUT> <SOURCE>

Arguments:
  <SOURCE>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --sample-fps <SAMPLE_FPS>      Requested samples per second, 0.1-10 (default 1; the decoder may lower it) [default: 1]
      --shot-penalty <SHOT_PENALTY>  Change-point penalty, 0.001-10 (default 0.15; higher = fewer, longer shots; content-dependent) [default: 0.15]
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade find-usage

```text
Match an image or media record against generic target images

Usage: saccade find-usage [OPTIONS] <SOURCE> <TARGETS>...

Arguments:
  <SOURCE>
  <TARGETS>...

Options:
      --json
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade models

```text
List or explicitly pull pinned local models

Usage: saccade models [OPTIONS] <COMMAND>

Commands:
  list    Inspect selections, real pins, cache integrity and source-parity status
  config  Show the resolved model configuration and where each value came from
  pull    The one provisioning verb: download and verify the named pinned artifacts

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade models list

```text
Inspect selections, real pins, cache integrity and source-parity status

Usage: saccade models list [OPTIONS]

Options:
      --registry <REGISTRY>          Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --cache <CACHE>                Deprecated: set SACCADE_MODELS_DIR or [models].dir
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade models config

```text
Show the resolved model configuration and where each value came from

Usage: saccade models config [OPTIONS]

Options:
      --json
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade models pull

```text
The one provisioning verb: download and verify the named pinned artifacts.

ID is a registry model, `runtime` (ONNX Runtime), `ocr` (the pinned OCR contract, or --contract FILE) or `embedding` (--contract FILE or the configured embedding contract). Nothing else downloads on request.

Usage: saccade models pull [OPTIONS] <ID>

Arguments:
  <ID>


Options:
      --contract <CONTRACT>
          Contract file for `ocr` / `embedding`

      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports

      --registry <REGISTRY>
          Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry

      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)

      --cache <CACHE>
          Deprecated: set SACCADE_MODELS_DIR or [models].dir

      --json


  -h, --help
          Print help (see a summary with '-h')

Global options:
      --allow-out-near-captures
          Silence warnings when --out is next to capture metadata

      --record-absolute-paths
          Opt in to absolute local paths in reports and machine-readable output
```

## saccade locate

```text
Locate a phrase with boxes, optional masks, and an overlay PNG

Usage: saccade locate [OPTIONS] <IMAGE> <PHRASE>

Arguments:
  <IMAGE>
  <PHRASE>

Options:
      --segment

      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --detector <DETECTOR>
          [default: grounding-dino-tiny]
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --segmenter <SEGMENTER>
          [default: sam-2.1-tiny]
      --observations <OBSERVATIONS>
          Explicit generated/frozen observation receipt; output is labelled replay
      --overlay <OVERLAY>
          New overlay PNG; existing files are never overwritten
      --registry <REGISTRY>
          Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry
      --cache <CACHE>
          Deprecated: set SACCADE_MODELS_DIR or [models].dir
      --runtime-library <RUNTIME_LIBRARY>
          Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download
          Deprecated: provision with `saccade models pull <id>` instead
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade quality-score

```text
Measure a separately named learned quality score

Usage: saccade quality-score [OPTIONS] <IMAGE>

Arguments:
  <IMAGE>

Options:
      --reference <REFERENCE>
          Full-reference metric command needs an explicit reference
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --metric <METRIC>
          [default: musiq]
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --observations <OBSERVATIONS>
          Explicit stand-in/frozen measurement receipt, always labelled replay
      --registry <REGISTRY>
          Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry
      --cache <CACHE>
          Deprecated: set SACCADE_MODELS_DIR or [models].dir
      --runtime-library <RUNTIME_LIBRARY>
          Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download
          Deprecated: provision with `saccade models pull <id>` instead
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade watermark

```text
Decode explicitly compatible watermark schemes without an origin verdict

Usage: saccade watermark [OPTIONS] <IMAGE>

Arguments:
  <IMAGE>

Options:
      --expected-payload <EXPECTED_PAYLOAD>
          Known legacy message bytes in hex; arbitrary recovered bits are not detection
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --quantization-step <QUANTIZATION_STEP>
          Coefficient quantization step of the embedding workflow (default 36; must be above 0) [default: 36]
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --minimum-agreement <MINIMUM_AGREEMENT>
          Required fraction of block votes supporting the expected bits, 0.75-1 (default 0.9) [default: 0.9]
      --observations <OBSERVATIONS>
          Explicit frozen/generated primary-decoder observation report
      --trustmark
          Decode the pinned TrustMark Q model and its BCH payload/schema; never downloads
      --registry <REGISTRY>
          Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry
      --cache <CACHE>
          Deprecated: set SACCADE_MODELS_DIR or [models].dir
      --runtime-library <RUNTIME_LIBRARY>
          Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download
          Deprecated: provision with `saccade models pull <id>` instead
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade faces

```text
Detect faces and optionally create a privacy-redacted PNG

Usage: saccade faces [OPTIONS] <IMAGE>

Arguments:
  <IMAGE>

Options:
      --detector <DETECTOR>
          [default: yunet-2026may]
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --observations <OBSERVATIONS>

      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --blur-faces <BLUR_FACES>
          Write a new strongly redacted PNG; never overwrite an original
      --registry <REGISTRY>
          Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry
      --cache <CACHE>
          Deprecated: set SACCADE_MODELS_DIR or [models].dir
      --runtime-library <RUNTIME_LIBRARY>
          Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download
          Deprecated: provision with `saccade models pull <id>` instead
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade crop-check

```text
Assess declared crops against detected faces, without identity recognition

Usage: saccade crop-check [OPTIONS] --crop <CROP> <IMAGE>

Arguments:
  <IMAGE>

Options:
      --detector <DETECTOR>
          [default: yunet-2026may]
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --observations <OBSERVATIONS>

      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --blur-faces <BLUR_FACES>
          Write a new strongly redacted PNG; never overwrite an original
      --registry <REGISTRY>
          Deprecated: set SACCADE_MODELS_REGISTRY or [models].registry
      --cache <CACHE>
          Deprecated: set SACCADE_MODELS_DIR or [models].dir
      --runtime-library <RUNTIME_LIBRARY>
          Deprecated: set SACCADE_MODELS_RUNTIME_LIBRARY or [models].runtime_library. ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download
          Deprecated: provision with `saccade models pull <id>` instead
      --json

      --crop <CROP>
          Repeat aspect ratio W:H or original-pixel rectangle X,Y,W,H
      --focal-point <FOCAL_POINT>
          Original-pixel X,Y, optional
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade observe-local

```text
Bounded advisory observations from an explicitly configured local VLM

Usage: saccade observe-local [OPTIONS] --endpoint <ENDPOINT> --runtime-revision <RUNTIME_REVISION> <REQUEST>

Arguments:
  <REQUEST>  Bounded saccade observation request JSON with exact encoded images/transforms

Options:
      --endpoint <ENDPOINT>

      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --runtime-revision <RUNTIME_REVISION>

      --response <RESPONSE>
          Decode an explicitly recorded response without making any HTTP request
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade provider-map

```text
Map provider requests or decode recorded vision responses; no live calls

Usage: saccade provider-map [OPTIONS] --provider <PROVIDER> <REQUEST>

Arguments:
  <REQUEST>

Options:
      --provider <PROVIDER>

      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --endpoint-profile <ENDPOINT_PROFILE>
          Startup env-file mapping for generic OpenAI-compatible or Azure deployment endpoints [possible values: openai-compatible, azure-openai]
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --response <RESPONSE>
          Explicit recorded response; omit to show request mapping only (no credentials)
      --coordinates <COORDINATES>
          [default: pixels]
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade renderdoc-localize

```text
Align optional Vulkan replay evidence and locate native-resource divergence

Usage: saccade renderdoc-localize [OPTIONS] --out <OUT> <BASELINE> <CANDIDATE>

Arguments:
  <BASELINE>   Baseline worker extraction.json; raw payloads must stay beneath its directory
  <CANDIDATE>

Options:
      --out <OUT>                    New JSON localization report
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade regions

```text
Import and freeze phrase regions, or inspect optional model plumbing

Usage: saccade regions [OPTIONS] <COMMAND>

Commands:
  import         Freeze a manually accepted phrase region from an imported inclusion mask
  status         Report honest text-to-mask and import capabilities without loading models
  cache          Explicitly download hash-pinned model artifacts into a local cache
  runtime-probe  Load self-contained ONNX graphs; graph loading does not qualify inference/parity

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade regions import

```text
Freeze a manually accepted phrase region from an imported inclusion mask

Usage: saccade regions import [OPTIONS] --reference <REFERENCE> --mask <MASK> --phrase <PHRASE> --out <OUT>

Options:
      --reference <REFERENCE>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --mask <MASK>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --phrase <PHRASE>
      --out <OUT>                    New frozen-region JSON file; use it with localized-check --region
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade regions status

```text
Report honest text-to-mask and import capabilities without loading models

Usage: saccade regions status [OPTIONS]

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade regions cache

```text
Explicitly download hash-pinned model artifacts into a local cache

Usage: saccade regions cache [OPTIONS] --manifest <MANIFEST> --cache <CACHE>

Options:
      --manifest <MANIFEST>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --cache <CACHE>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade regions runtime-probe

```text
Load self-contained ONNX graphs; graph loading does not qualify inference/parity

Usage: saccade regions runtime-probe [OPTIONS] --manifest <MANIFEST> --cache <CACHE> --library <LIBRARY>

Options:
      --manifest <MANIFEST>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --cache <CACHE>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --library <LIBRARY>
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade explain-grounded

```text
Render verified atomic numerical claims with region and evidence citations

Usage: saccade explain-grounded [OPTIONS] --report <REPORT> --out <OUT>

Options:
      --report <REPORT>              Immutable comparison or localized measurement JSON
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --proposals <PROPOSALS>        Optional JSON array of atomic proposals; no provider calls are made
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --out <OUT>                    New explanation JSON file
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade localized-check

```text
Measure intended-region, boundary and protected-complement changes independently

Usage: saccade localized-check [OPTIONS] --out <OUT> <--box <BBOX>|--mask <MASK>|--selector <SELECTOR>|--region <REGION>|--required-effect <REQUIRED_EFFECT>> <REFERENCE> <CANDIDATE>

Arguments:
  <REFERENCE>  Reference screenshot, retaining the intended region if candidate content disappears
  <CANDIDATE>

Options:
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>

      --config <CONFIG>

      --required-effect <REQUIRED_EFFECT>
          Required-effect policy JSON; records occupancy, including an empty mask
      --box <BBOX>
          Pixel box x,y,width,height
      --mask <MASK>
          Binary grayscale inclusion PNG: 255 inside, 0 outside
      --selector <SELECTOR>
          Exact selector from producer metadata; one match required
      --metadata <METADATA>
          Capture-bound DOM geometry JSON
      --region <REGION>
          Previously frozen inclusion-region JSON
      --out <OUT>
          New directory for the frozen region and measurements
      --perceptual-outside
          Use maximum full-frame complement FLIP instead of exact native preservation
      --maximum-outside-flip <MAXIMUM_OUTSIDE_FLIP>
          Largest FLIP score allowed outside the intended region, 0-1 (default 0.01; above it fails) [default: 0.01]
      --ppd <PPD>
          Viewing condition in pixels per degree of visual angle (default 67) [default: 67]
      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inventory

```text
Reconcile expected and supplied stable capture cases against a comparison report

Usage: saccade inventory [OPTIONS] --manifest <MANIFEST> --report <REPORT> --out <OUT>

Options:
      --manifest <MANIFEST>          Expected suite and supplied capture attempts, with stable case IDs
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report <REPORT>              Existing comparison report
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --out <OUT>                    New inventory JSON file
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade quality-sweep

```text
Measure externally encoded quality candidates under a frozen score and byte budget

Usage: saccade quality-sweep [OPTIONS] --out <OUT> <MANIFEST>

Arguments:
  <MANIFEST>  Frozen sweep manifest. All artifacts must be beneath its directory

Options:
      --out <OUT>                    New JSON report file; existing files are preserved
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade history

```text
Record and inspect local visual-test variation across runs

Usage: saccade history [OPTIONS] <COMMAND>

Commands:
  onset    Find candidate performance onsets in qualified, comparable history observations
  record   Add one existing comparison report to the local history store
  analyze  Show measured variation and threshold advice for comparable entries

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade history onset

```text
Find candidate performance onsets in qualified, comparable history observations

Usage: saccade history onset [OPTIONS] --store <STORE>

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --store <STORE>
      --limit <LIMIT>                Most recent distinct observations per partition; exact DP is bounded to 120 [default: 60]
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade history record

```text
Add one existing comparison report to the local history store

Usage: saccade history record [OPTIONS] --store <STORE> <REPORT>

Arguments:
  <REPORT>

Options:
      --run-id <RUN_ID>
          Producer-assigned independent capture run, never an image or report hash
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --environment-id <ENVIRONMENT_ID>
          Frozen browser/device, fonts, viewport, warmup and temporal protocol identity
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --unchanged-build
          Declare an unchanged-build repeat eligible for normal-variation advice
      --store <STORE>

      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade history analyze

```text
Show measured variation and threshold advice for comparable entries

Usage: saccade history analyze [OPTIONS] --store <STORE>

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --store <STORE>
      --entry <ENTRY>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --drift                        Diagnose sustained anchor-relative drift in recorded run order
      --out <OUT>                    New file containing the complete witness for the selected groups
      --limit <LIMIT>                Maximum runs to list, 1-20 (default 10) [default: 10]
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade bisect

```text
Locate the first commit whose fresh capture fails its baseline

Usage: saccade bisect [OPTIONS] --capture <CAPTURE> --baseline <BASELINE>

Options:
      --good <GOOD>                  Known good revision in the current repository
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --bad <BAD>                    Known bad revision descended from --good
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --capture <CAPTURE>            Shell capture command; write images to SACCADE_CAPTURE_DIR (sh on Unix, cmd on Windows)
      --baseline <BASELINE>          Stable baseline directory, copied before Git changes revisions
      --perf                         Require qualified performance evidence and count a slower frame as bad
      --out <OUT>                    Evidence directory outside the repository; defaults to a new sibling
      --json                         Print bounded JSON
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest

```text
Convert a test runner's screenshot artifacts into compared image pairs

Usage: saccade ingest [OPTIONS] <COMMAND>

Commands:
  blender     Pair Blender render report category/ref images with category renders
  bevy        Pair Bevy screenshot-N.png files from two runs
  unity       Pair Unity Graphics Test Framework ReferenceImages and ActualImages
  unreal      Read Unreal screenshot comparison result paths from JSON
  playwright  Compare expected and actual Playwright screenshot attachments

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest blender

```text
Pair Blender render report category/ref images with category renders

Usage: saccade ingest blender [OPTIONS] --out <OUT> <ROOT>

Arguments:
  <ROOT>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest bevy

```text
Pair Bevy screenshot-N.png files from two runs

Usage: saccade ingest bevy [OPTIONS] --out <OUT> <REFERENCE> <CAPTURE>

Arguments:
  <REFERENCE>
  <CAPTURE>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest unity

```text
Pair Unity Graphics Test Framework ReferenceImages and ActualImages

Usage: saccade ingest unity [OPTIONS] --out <OUT> <ASSETS>

Arguments:
  <ASSETS>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest unreal

```text
Read Unreal screenshot comparison result paths from JSON

Usage: saccade ingest unreal [OPTIONS] --out <OUT> <RESULTS>

Arguments:
  <RESULTS>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest playwright

```text
Compare expected and actual Playwright screenshot attachments

Usage: saccade ingest playwright [OPTIONS] --out <OUT> <MANIFEST>

Arguments:
  <MANIFEST>  Manifest written by integrations/playwright/reporter.cjs

Options:
      --out <OUT>                    New directory for paired inputs and the comparison report
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json                         Print the bounded comparison result
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --threshold <THRESHOLD>        FLIP threshold for the comparison
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade doctor

```text
Print installed version, features and supported evidence schemas

Usage: saccade doctor [OPTIONS]

Options:
      --json                         Print machine-readable JSON
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade init

```text
Bootstrap a commented configuration and print baseline adoption steps

Usage: saccade init [OPTIONS]

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --template <TEMPLATE>          [default: renderer] [possible values: renderer, ui, identity, ml, producer-strict, ci, nightly, lookdev]
      --dir <DIR>                    [default: .]
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --force
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade demo

```text
Run the bundled example and explain its expected regression

Usage: saccade demo [OPTIONS]



Example:
  saccade demo --out saccade-demo
  saccade view saccade-demo          Print where the demo report is

The demo exits 1 on purpose: it contains a regression and a missing capture.

Options:
      --out <OUT>                    Directory for the demo images and reports (default: a new temporary directory)
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade compare

```text
Compare a directory of captures against a directory of baselines

Usage: saccade compare [OPTIONS] [BASELINE_DIR] <CAPTURE_DIR>



Images are paired by relative path. Each pair gets a FLIP score; a pair fails when
its deciding metric is above the threshold. The report directory holds index.html
(open it in a browser) and saccade-report.v1.json.

Examples:
  saccade compare baseline/ captures/ --out report
  saccade compare baseline/ captures/ --threshold 0.02 --metric p95
  saccade compare baseline/ captures/ --entry 'ui/*' --junit report/junit.xml
  saccade compare baseline/ captures/ --json        One bounded JSON result on stdout

Exit codes: 0 no regression, 1 regression found, 2 the command could not run.

Arguments:
  [BASELINE_DIR]  Directory of approved baseline images
  <CAPTURE_DIR>   Directory of fresh captures

Options:
      --dpi <DPI>
          Declared document raster density, 36..600 DPI (default 96)
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --question <QUESTION>
          Explicit comparison question; no automatic model fallback [possible values: same-render, same-content, same-text, near-duplicate, quality]
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --model <MODEL>
          Supplied embedding export contract for same-content
      --cache <CACHE>
          Content-addressed model cache for same-content
      --library <LIBRARY>
          Explicit ONNX Runtime library for same-content
      --reference-source <REFERENCE_SOURCE>
          Image-bound reference text observations for same-text
      --capture-source <CAPTURE_SOURCE>
          Image-bound candidate text observations for same-text
      --ocr-contract <OCR_CONTRACT>
          Existing pinned OCR contract for same-text; requires ocr feature
      --align <ALIGN>
          Explicit registration; defaults to the existing unregistered pipeline [possible values: none, translation, similarity, affine, homography, auto]
      --resample <RESAMPLE>
          Explicit cross-resolution comparison scale; registration evidence records it [possible values: reference, common]
      --export-maps
          Export native FLIP and tile grids as float32 NPY/EXR with a JSON index
      --require-scope
          Require an actual ID layer or nonempty mask scope
      --noise-from <REPEAT> <REPEAT>...
          Same-arm repeat files or run directories (2..32); enables noise-aware deciding evidence
      --mask-dump <MASK_DUMP>
          Generic screen-space dump filename relative to each capture; used when no layer manifest exists
      --mask-layer <MASK_LAYER>
          Named layer and native predicate, NAME=id=1,2 or NAME=label=pattern
      --require-effect <REQUIRE_EFFECT>
          Required occupancy from NAME=predicate[:MIN_PIXELS] or mask:FILE[:MIN_PIXELS]
      --id-top <ID_TOP>
          Per-ID rows and diagnostic crops retained, at most 32
      --id-threshold <ID_THRESHOLD>
          Declared normalized luminance threshold for colour per-ID statistics
      --baseline <BASELINE>
          Resolve the latest complete passing history run as an immutable baseline [possible values: last-good]
      --history-store <HISTORY_STORE>
          Local history store for --baseline last-good
  -h, --help
          Print help

Output:
      --out <OUT>         Report output directory [default: report]
      --json              Print a bounded machine-readable result
      --labels <A,B>      Display names of the two sides, `baseline,capture`
      --junit <FILE.xml>  Write one JUnit testcase per entry

Gate:
      --threshold <THRESHOLD>  FLIP score limit in 0-1 (0 = identical): a pair fails when its --metric value is above it. Overrides the config file
      --metric <METRIC>        Default deciding metric (overrides the config file's top level) [possible values: mean, p95, p99, max]
      --config <CONFIG>        Config file; defaults to ./saccade.toml when it exists
      --fail-on-new            Treat new images (no baseline) as a regression
      --allow-empty            Accept a run that compared no pair (for example the first run, with an empty baseline directory). Without it, nothing compared exits 1
      --ppd <PPD>              Viewing condition in pixels per degree of visual angle (default 67; larger = finer detail is visible)

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)

Selection:
      --entry <GLOB>  Include only matching names (repeatable; union of globs)

Metadata sidecars:
      --meta-name <NAME>
          Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>
          Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>
          Intended experiment metadata variables (exact keys or globs)
      --require-matching-meta
          Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>
          Sidecar keys (or globs) that may differ with --require-matching-meta

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clock-map <FILE>       TOML/JSON map from producer telemetry to GPU clock evidence
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Review context:
      --intent <TEXT>        What the change is meant to do, in one sentence, recorded in the evidence
      --intent-file <FILE>   Structured evidence intent or visual declaration JSON, written before capture
      --changes-file <FILE>  JSON list of expected changes; needs --intent or --intent-file

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade identity

```text
Establish exact native decoded-sample equality in the selected scope

Usage: saccade identity [OPTIONS] <PARENT_DIR> <CANDIDATE_DIR>



Use it to prove a refactor or optimization renders the same pixels. There is no
threshold: any differing sample fails. Different file encodings of equal pixels pass.

Examples:
  saccade identity parent/ candidate/ --out report
  saccade identity parent/ candidate/ --json      One bounded JSON result on stdout

Exit codes: 0 every pair identical, 1 identity not proven (a pair differs, is missing,
new or unreadable), 2 the command could not run.

Arguments:
  <PARENT_DIR>     Directory of images from the parent build
  <CANDIDATE_DIR>  Directory of images from the candidate build

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Output:
      --out <OUT>         Report output directory [default: report]
      --json              Print a bounded machine-readable result
      --labels <A,B>      Display names of the two sides, `parent,candidate`
      --junit <FILE.xml>  Write one JUnit testcase per entry

Gate:
      --allow-empty      Accept a run that compared no pair. Without it, nothing compared exits 1
      --config <CONFIG>  Config file; defaults to ./saccade.toml when it exists
      --ppd <PPD>        Viewing condition in pixels per degree of visual angle (default 67), used only to describe differences

Selection:
      --entry <GLOB>  Include only matching names (repeatable; union of globs)

Metadata sidecars:
      --meta-name <NAME>
          Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>
          Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>
          Intended experiment metadata variables (exact keys or globs)
      --require-matching-meta
          Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>
          Sidecar keys (or globs) that may differ with --require-matching-meta

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clock-map <FILE>       TOML/JSON map from producer telemetry to GPU clock evidence
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Review context:
      --intent <TEXT>        What the change is meant to do, in one sentence, recorded in the evidence
      --intent-file <FILE>   Structured evidence intent or visual declaration JSON, written before capture
      --changes-file <FILE>  JSON list of expected changes; needs --intent or --intent-file

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade prove

```text
Check whether image identity or performance evidence proves a claim

Usage: saccade prove [OPTIONS] <COMMAND>

Commands:
  mesh-identity  Prove exact ordered static mesh geometry identity (appearance excluded)
  identity       Prove exact native decoded-sample equality over the selected images
  performance    Evaluate performance claims from ablation arms and repeat noise

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade prove mesh-identity

```text
Prove exact ordered static mesh geometry identity (appearance excluded)

Usage: saccade prove mesh-identity [OPTIONS] --unit <UNIT> <BASELINE> <CAPTURE>

Arguments:
  <BASELINE>
  <CAPTURE>

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --unit <UNIT>                  Declared common coordinate unit; no conversion or registration is performed
      --json
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade prove identity

```text
Prove exact native decoded-sample equality over the selected images

Usage: saccade prove identity [OPTIONS] <PARENT_DIR> <CANDIDATE_DIR>

Arguments:
  <PARENT_DIR>
  <CANDIDATE_DIR>

Options:
      --out <OUT>                    [default: report]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --config <CONFIG>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --json
      --allow-empty
      --ppd <PPD>                    Viewing condition in pixels per degree of visual angle (default 67)
      --labels <A,B>
      --junit <FILE.xml>
      --entry <GLOB>
  -h, --help                         Print help

Metadata sidecars:
      --meta-name <NAME>
          Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>
          Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>
          Intended experiment metadata variables (exact keys or globs)
      --require-matching-meta
          Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>
          Sidecar keys (or globs) that may differ with --require-matching-meta

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clock-map <FILE>       TOML/JSON map from producer telemetry to GPU clock evidence
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Review context:
      --intent <TEXT>        What the change is meant to do, in one sentence, recorded in the evidence
      --intent-file <FILE>   Structured evidence intent or visual declaration JSON, written before capture
      --changes-file <FILE>  JSON list of expected changes; needs --intent or --intent-file

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade prove performance

```text
Evaluate performance claims from ablation arms and repeat noise

Usage: saccade prove performance [OPTIONS] [BASE] [ARMS]...

Arguments:
  [BASE]
  [ARMS]...

Options:
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>
          Intended metadata variable for every arm
      --arm-variable <ARM_VARIABLES>
          Arm-specific variable, LABEL=KEY (repeatable; KEY may be a glob)
      --base <RUN_DIR>...
          Base repeat directories. Accepts a directory or a quoted glob; repeatable
      --arm <LABEL=RUN_GLOB>
          Labelled arm repeats, e.g. --arm 's2=s2_r*'; repeatable
      --out <OUT>
          [default: ablation]
      --config <CONFIG>

      --json

      --top <TOP>
          Per-term deltas beyond noise to show per arm [default: 5]
  -h, --help
          Print help

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clock-map <FILE>       TOML/JSON map from producer telemetry to GPU clock evidence
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade noise

```text
Calibrate thresholds from repeated captures of an unchanged build

Usage: saccade noise [OPTIONS] <DIRS> <DIRS>...
       saccade noise [OPTIONS] [DIRS] [DIRS]... <COMMAND>

Commands:
  build  Build per-tile empirical noise envelopes from same-arm repeats

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --kind <KIND>                  Image calibration (default) or qualified performance noise in ms [default: image] [possible values: image, performance]
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clock-map <FILE>       TOML/JSON map from producer telemetry to GPU clock evidence
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)
      --config <CONFIG>
      --margin <MARGIN>            Multiplier on the largest observed metric value when setting the noise floor (default 1.5) [default: 1.5]
      --metric <METRIC>            [default: p95] [possible values: mean, p95, p99, max]
      --out <OUT>                  [default: saccade.noise.toml]
      --json
  <DIRS> <DIRS>...

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade noise build

```text
Build per-tile empirical noise envelopes from same-arm repeats

Usage: saccade noise build [OPTIONS] <REPEATS> <REPEATS>...

Arguments:
  <REPEATS> <REPEATS>...  2..32 repeats from the same arm, files or run directories

Options:
      --out <OUT>
          [default: repeat-noise.json]
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --tile-size <TILE_SIZE>
          [default: 32]
      --json

      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade view

```text
Write a self-contained review viewer for 2 to 6 image directories

Usage: saccade view [OPTIONS] [DIRS]...



Examples:
  saccade view before/ after/ --out view          Swipe, flicker and heatmap viewer
  saccade view a/ b/ c/ --labels a,b,c --reference a
  saccade view my-report                          Print where an existing report's page is
  saccade view a/ b/ --blind --key-out ../key.json --out judge-view

Arguments:
  [DIRS]...  Directories to compare, paired by relative image path (2 to 6)

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Blind judging:
      --unblind <UNBLIND>  Resolve recorded anonymous choices after review
      --key <KEY>          The key written by --blind, used with --unblind
      --blind              Pairwise judging: shuffle panes and hide labels until "Reveal"
      --seed <SEED>        Seed for the blind shuffle (default: random). A blind page never embeds it; it is recorded in the key
      --key-out <PATH>     Where a blind view's key goes. Required with --blind; keep it outside --out so the judge never receives it

Output:
      --labels <LABELS>  Comma-separated labels, one per directory (default: directory names)
      --out <OUT>        Output directory [default: view]
      --json             Print a JSON summary (`saccade-view-summary.v1`) instead of text
      --open             Open the page in the default browser after writing or locating it

Comparison:
      --reference <REFERENCE>  FLIP reference: a label or one of the directories (default: the first)
      --ppd <PPD>              Viewing condition in pixels per degree of visual angle (default 67; larger = finer detail is visible)
      --config <CONFIG>        Config file whose `[[region]]` tables become preset ROIs (default: `./saccade.toml` when present)

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)

Selection:
      --entry <GLOB>  Include only matching names (repeatable; union of globs)

Metadata sidecars:
      --meta-name <NAME>        Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>  Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clock-map <FILE>       TOML/JSON map from producer telemetry to GPU clock evidence
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade approve

```text
Copy reviewed captures over baselines

Usage: saccade approve [OPTIONS] [CAPTURE_DIR] [BASELINE_DIR] [NAMES]...



Example (two steps: plan, then apply the reviewed decision):
  saccade approve --report report/saccade-report.v1.json --entry ui.png --dry-run --out plan
  saccade approve --report report/saccade-report.v1.json --decisions plan/decision.json --out receipt

Review the report, plan/manifest.json and plan/decision.json between the two steps.
The dry run writes no baseline; content hashes must still match when applying.

Arguments:
  [CAPTURE_DIR]   Directory of fresh captures
  [BASELINE_DIR]  Baseline directory to update
  [NAMES]...      Image names (relative paths) to approve

Options:
      --report <REPORT>              Derive the input directories from this report
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --entry <NAME>                 Select a report entry without positional directories; repeatable
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --all-failing [<REPORT_JSON>]  Also approve every fail and new entry of this report JSON
      --decisions <DECISIONS_JSON>   Explicit canonical CLI decision bound to this report, inputs and scope
      --include-errors               With --all-failing: also approve `error` entries (for example a size change) whose capture exists and decodes
      --prune-missing                With --all-failing: delete the baselines of every `missing` entry of the report (capture absent). Only files inside the baseline directory are removed; each removal is printed
      --json                         Print `{"schema":"saccade-approve.v1","copied":[...],"pruned":[...]}` instead of one line per file
      --dry-run                      Prepare a selected update manifest and unattested CLI decision draft
      --out <OUT>                    Empty directory for the plan, decision and applied receipt
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade serve

```text
Browse report and image archives in a local web workbench

Usage: saccade serve [OPTIONS] [ROOTS]...



The server listens on 127.0.0.1 only. Archive roots are read-only: sessions,
thumbnails and uploads go to the cache directory, decisions to the decisions directory.

Examples:
  saccade serve captures/ --open               Browse and compare runs in the browser
  saccade serve captures/ reports/ --port 0    Several roots; pick a free port

Arguments:
  [ROOTS]...  Archive roots to browse (read-only). With several, each is a top-level entry named after its directory

Options:
      --api
          Serve the local versioned media API instead of the archive viewer
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --api-max-bytes <API_MAX_BYTES>
          Largest accepted request body in bytes (default 16777216 = 16 MiB) [default: 16777216]
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --api-bind <API_BIND>
          [default: 127.0.0.1]
      --api-token-file <API_TOKEN_FILE>
          Optional bearer-token env file; default ~/.config/saccade/api.env if present
      --api-model-dir <API_MODEL_DIR>

      --api-registry <API_REGISTRY>

      --root <REGISTERED_ROOTS>
          Additional read-only archive roots (repeatable)
      --out-root <OUT_ROOT>
          Explicit generated-artifact root
      --follow-symlinks-within-roots
          Let a symlink that resolves inside any of the roots be browsed and served; a symlink to anywhere else stays refused
      --symlink-target <SYMLINK_TARGETS>
          Allow symlinks reached below a root to resolve into DIR (repeatable)
      --fs-timeout-ms <FS_TIMEOUT_MS>
          Storage deadline in milliseconds, at least 1 (default: 3000)
      --port <PORT>
          Port on 127.0.0.1 (0 picks a free one) [default: 7878]
      --cache-dir <CACHE_DIR>
          Cache directory for sessions, thumbnails and uploads (default: `$XDG_CACHE_HOME/saccade`)
      --decisions-dir <DECISIONS_DIR>
          Directory the viewer's decisions are written to (default: `$XDG_DATA_HOME/saccade/decisions`)
      --config <CONFIG>
          Config file for sidecar settings and preset regions (default: `./saccade.toml` when present)
      --ppd <PPD>
          Viewing condition in pixels per degree of visual angle (default 67; larger = finer detail is visible). Viewing condition in pixels per degree of visual angle (default 67)
      --open
          Open the page in the default browser
  -h, --help
          Print help

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)

Metadata sidecars:
      --meta-name <NAME>        Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>  Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clock-map <FILE>       TOML/JSON map from producer telemetry to GPU clock evidence
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade mcp

```text
Serve the agent tools over MCP on stdio, confined to the given roots

Usage: saccade mcp [OPTIONS] --root <ROOTS>



Every path a client passes must resolve under a --root. Generated reports go under
--out-root, which must be separate from the read-only roots.

Example:
  saccade mcp --root examples --out-root agent-reports

Options:
      --root <ROOTS>                  Read-only roots (repeatable)
      --source-ref <SOURCE_REF>       External capture URI/key (repeatable); recorded in generated reports
      --out-root <OUT_ROOT>           Generated artifacts require this separate root
      --report-index <REPORT_INDEX>   Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --follow-symlinks-within-roots  Let a symlink that resolves inside any of the roots be read
      --symlink-target <DIR>          Allow symlinks reached below a root to resolve into DIR (repeatable)
      --allow-provider-calls          Explicitly authorize provider calls for this MCP server lifetime
      --budget-calls <BUDGET_CALLS>   Finite startup attempt cap; no implicit MCP allowance
      --user-config <USER_CONFIG>     Human-owned endpoints, credential bindings and root egress policy
      --allow-product-network         Authorize product HTTP operations from registered roots
      --allow-webhook-notifications   Authorize explicit webhook tool calls using user configuration
  -h, --help                          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect

```text
Read, explain, prepare or export existing evidence

Usage: saccade inspect [OPTIONS] [ARTIFACT] [COMMAND]

Commands:
  exclusions    Show what a comparison excluded and the remaining threshold headroom
  evidence      Prepare context, crops, facts and references without a provider
  export        Export an existing artifact or selected entry
  config        Explain effective measurement settings and their sources
  capabilities  List compiled modules, operations and contracts

Arguments:
  [ARTIFACT]

Options:
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --entry <ENTRY>

      --validity-reasons
          List every capture-validity reason, with pagination
      --status <STATUS>

      --limit <LIMIT>
          Maximum rows to show (default 10) [default: 10]
      --cursor <CURSOR>

      --expected-case-id <EXPECTED_CASE_ID>

      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect exclusions

```text
Show what a comparison excluded and the remaining threshold headroom

Usage: saccade inspect exclusions [OPTIONS] <REPORT>

Arguments:
  <REPORT>

Options:
      --json
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect evidence

```text
Prepare context, crops, facts and references without a provider

Usage: saccade inspect evidence [OPTIONS] --out <OUT> <REPORT>

Arguments:
  <REPORT>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --entry <ENTRIES>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --top <TOP>                    Number of entries to include in the evidence pack (default 5) [default: 5]
      --stretch
      --blind
      --key-out <KEY_OUT>
      --seed <SEED>                  Integer seed for the blind shuffle (default: random)
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect export

```text
Export an existing artifact or selected entry

Usage: saccade inspect export [OPTIONS] --format <FORMAT> --out <OUT> <ARTIFACT>

Arguments:
  <ARTIFACT>

Options:
      --format <FORMAT>              [possible values: json, markdown, junit, png, labels]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --entry <ENTRY>
      --state <STATE>
      --width <WIDTH>                Exported image width in pixels (default 1024) [default: 1024]
      --artifact-url <ARTIFACT_URL>
      --comment-key <COMMENT_KEY>
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect config

```text
Explain effective measurement settings and their sources

Usage: saccade inspect config [OPTIONS]

Options:
      --config <CONFIG>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --entry <PATH_OR_NAME>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect capabilities

```text
List compiled modules, operations and contracts

Usage: saccade inspect capabilities [OPTIONS]

Options:
      --json
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review

```text
Preview a review plan or handle a local closed decision request

Usage: saccade review [OPTIONS] [REPORT] [COMMAND]

Commands:
  trial       Preregister and present offline blind visual trials
  assist      Experimental assist lifecycle operations
  explain     Experimental localized visible explanations, advisory only
  audit-mask  Experimental individual-mask audit, advisory only
  check-ui    Experimental bounded visible condition, never behavioral success
  brand       Review brand colours, theme contrast, CVD and source typography together
  ui          Review source text/layout and localized UI changes in one packet
  motion      Diagnose dense correspondence and validate supplied renderer vectors
  request     Prepare a closed request from an existing canonical case, locally
  propose     Validate and record proposed answers against the exact request
  ask         Create or retrieve a local human review item for an unresolved request
  eval        Plan or run a resumable evaluation manifest

Arguments:
  [REPORT]

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --run
      --budget-calls <BUDGET_CALLS>  Maximum provider calls, at least 1; counts calls, not money
      --out <OUT>
      --user-config <USER_CONFIG>
      --intent-file <INTENT_FILE>
      --intent <INTENT>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review trial

```text
Preregister and present offline blind visual trials

Usage: saccade review trial [OPTIONS] <COMMAND>

Commands:
  register  Hash a plan and all inputs before decoding or showing images
  start     Lock inspection state and write the blind HTML gallery
  vote      Persist one explicit judgment through the core vote store
  import    Import the exported blind gallery judgments for an explicit voter

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review trial register

```text
Hash a plan and all inputs before decoding or showing images

Usage: saccade review trial register [OPTIONS] --out <OUT> <PLAN>

Arguments:
  <PLAN>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review trial start

```text
Lock inspection state and write the blind HTML gallery

Usage: saccade review trial start [OPTIONS] --out <OUT> <PLAN>

Arguments:
  <PLAN>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review trial vote

```text
Persist one explicit judgment through the core vote store

Usage: saccade review trial vote [OPTIONS] --out <OUT> --voter <VOTER> --item <ITEM> --answer <ANSWER> <PLAN>

Arguments:
  <PLAN>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --voter <VOTER>
      --item <ITEM>
      --answer <ANSWER>
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review trial import

```text
Import the exported blind gallery judgments for an explicit voter

Usage: saccade review trial import [OPTIONS] --out <OUT> --voter <VOTER> <PLAN> <JUDGMENTS>

Arguments:
  <PLAN>
  <JUDGMENTS>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --voter <VOTER>
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review assist

```text
Experimental assist lifecycle operations

Usage: saccade review assist [OPTIONS] <COMMAND>

Commands:
  batch  Asynchronous frozen evaluation jobs; never used by interactive advice

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review assist batch

```text
Asynchronous frozen evaluation jobs; never used by interactive advice

Usage: saccade review assist batch [OPTIONS] <COMMAND>

Commands:
  submit   Verify and submit once; ambiguous submissions cannot repeat
  status   Read local status, or poll once with --run
  collect  Collect once and settle terminal known usage; never wait

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review assist batch submit

```text
Verify and submit once; ambiguous submissions cannot repeat

Usage: saccade review assist batch submit [OPTIONS] --plan <PLAN> --job <JOB>

Options:
      --allow-spend-above-25-usd       Explicitly acknowledge a plan allowance above the default 25 USD ceiling
      --source-ref <SOURCE_REF>        External capture URI/key (repeatable); recorded in generated reports
      --plan <PLAN>                    Source-bound saccade-assist-batch-plan.v1 artifact
      --report-index <REPORT_INDEX>    Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --job <JOB>                      Durable receipt under the output root
      --experimental
      --run                            Authorize one live submission or one poll; default local only
      --response <RESPONSE>            Recorded collection fixture; cannot settle a live reservation
      --budget-calls <BUDGET_CALLS>    Maximum provider calls, 1-128 (default 8); the run stops when reached [default: 8]
      --deadline-secs <DEADLINE_SECS>  Wall-clock limit in seconds, 1-300 (default 300) [default: 300]
      --user-config <USER_CONFIG>
      --json
  -h, --help                           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review assist batch status

```text
Read local status, or poll once with --run

Usage: saccade review assist batch status [OPTIONS] --plan <PLAN> --job <JOB>

Options:
      --allow-spend-above-25-usd       Explicitly acknowledge a plan allowance above the default 25 USD ceiling
      --source-ref <SOURCE_REF>        External capture URI/key (repeatable); recorded in generated reports
      --plan <PLAN>                    Source-bound saccade-assist-batch-plan.v1 artifact
      --report-index <REPORT_INDEX>    Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --job <JOB>                      Durable receipt under the output root
      --experimental
      --run                            Authorize one live submission or one poll; default local only
      --response <RESPONSE>            Recorded collection fixture; cannot settle a live reservation
      --budget-calls <BUDGET_CALLS>    Maximum provider calls, 1-128 (default 8); the run stops when reached [default: 8]
      --deadline-secs <DEADLINE_SECS>  Wall-clock limit in seconds, 1-300 (default 300) [default: 300]
      --user-config <USER_CONFIG>
      --json
  -h, --help                           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review assist batch collect

```text
Collect once and settle terminal known usage; never wait

Usage: saccade review assist batch collect [OPTIONS] --plan <PLAN> --job <JOB>

Options:
      --allow-spend-above-25-usd       Explicitly acknowledge a plan allowance above the default 25 USD ceiling
      --source-ref <SOURCE_REF>        External capture URI/key (repeatable); recorded in generated reports
      --plan <PLAN>                    Source-bound saccade-assist-batch-plan.v1 artifact
      --report-index <REPORT_INDEX>    Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --job <JOB>                      Durable receipt under the output root
      --experimental
      --run                            Authorize one live submission or one poll; default local only
      --response <RESPONSE>            Recorded collection fixture; cannot settle a live reservation
      --budget-calls <BUDGET_CALLS>    Maximum provider calls, 1-128 (default 8); the run stops when reached [default: 8]
      --deadline-secs <DEADLINE_SECS>  Wall-clock limit in seconds, 1-300 (default 300) [default: 300]
      --user-config <USER_CONFIG>
      --json
  -h, --help                           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review explain

```text
Experimental localized visible explanations, advisory only

Usage: saccade review explain [OPTIONS] --report <REPORT> --out <OUT>

Options:
      --report <REPORT>

      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --entry <ENTRY>
          Select exactly one report entry; required when the report contains several
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --mask-manifest <MASK_MANIFEST>
          Optional original individual-mask declarations, bound to exact report bytes
      --vision-provider <VISION_PROVIDER>
          Prepare Claude/GPT mappings in the assist layer; no live calls [possible values: claude, gpt]
      --vision-response <VISION_RESPONSE>
          Explicit recorded response; bound to this catalog and request
      --experimental
          Required acknowledgement: this feature is unqualified experimental advice
      --out <OUT>
          New empty directory for immutable sidecars and the advice report
      --offline
          Replay existing observations; never authorize providers
      --replay <REPLAY>
          Recorded exact cache entries for offline fixture replay
      --run
          Explicitly authorize evidence export under fixed user root policy
      --route <ROUTE>
          Deterministic rules, routed cascade or the full visual path [default: cascade] [possible values: rules, cascade, all-vision]
      --jev-routing
          Optional separately measured Jev evidence-need routing; disabled by default
      --budget-calls <BUDGET_CALLS>
          Real provider request cap; no retries or automatic top-up [default: 4]
      --max-spend-usd <MAX_SPEND_USD>
          Finite per-entry USD ceiling, at most 0.15 [default: 0.15]
      --deadline-secs <DEADLINE_SECS>
          Overall deadline, including both orders and support [default: 300]
      --gemini-revision <GEMINI_REVISION>
          Required immutable returned revision for dispatch/replay
      --user-config <USER_CONFIG>

      --jev-revision <JEV_REVISION>
          Required Jev returned revision; the pinned model ID is the initial binding [default: jev-1.13.0]
      --bypass-cache
          Do not reuse cache; required for independent qualification samples
      --json

      --source-evidence <SOURCE_EVIDENCE>
          Hash/dimension-bound Wave 3 source packets (at most one per image)
      --incomplete-capture
          Producer states some requested capture scope was not captured
      --pre-masked
          Original pixels were blacked out before capture and are unavailable
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review audit-mask

```text
Experimental individual-mask audit, advisory only

Usage: saccade review audit-mask [OPTIONS] --report <REPORT> --out <OUT>

Options:
      --report <REPORT>

      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --entry <ENTRY>
          Select exactly one report entry; required when the report contains several
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --mask-manifest <MASK_MANIFEST>
          Optional original individual-mask declarations, bound to exact report bytes
      --vision-provider <VISION_PROVIDER>
          Prepare Claude/GPT mappings in the assist layer; no live calls [possible values: claude, gpt]
      --vision-response <VISION_RESPONSE>
          Explicit recorded response; bound to this catalog and request
      --experimental
          Required acknowledgement: this feature is unqualified experimental advice
      --out <OUT>
          New empty directory for immutable sidecars and the advice report
      --offline
          Replay existing observations; never authorize providers
      --replay <REPLAY>
          Recorded exact cache entries for offline fixture replay
      --run
          Explicitly authorize evidence export under fixed user root policy
      --route <ROUTE>
          Deterministic rules, routed cascade or the full visual path [default: cascade] [possible values: rules, cascade, all-vision]
      --jev-routing
          Optional separately measured Jev evidence-need routing; disabled by default
      --budget-calls <BUDGET_CALLS>
          Real provider request cap; no retries or automatic top-up [default: 4]
      --max-spend-usd <MAX_SPEND_USD>
          Finite per-entry USD ceiling, at most 0.15 [default: 0.15]
      --deadline-secs <DEADLINE_SECS>
          Overall deadline, including both orders and support [default: 300]
      --gemini-revision <GEMINI_REVISION>
          Required immutable returned revision for dispatch/replay
      --user-config <USER_CONFIG>

      --jev-revision <JEV_REVISION>
          Required Jev returned revision; the pinned model ID is the initial binding [default: jev-1.13.0]
      --bypass-cache
          Do not reuse cache; required for independent qualification samples
      --json

      --source-evidence <SOURCE_EVIDENCE>
          Hash/dimension-bound Wave 3 source packets (at most one per image)
      --incomplete-capture
          Producer states some requested capture scope was not captured
      --pre-masked
          Original pixels were blacked out before capture and are unavailable
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review check-ui

```text
Experimental bounded visible condition, never behavioral success

Usage: saccade review check-ui [OPTIONS] --image <IMAGE> --box <BOX> --out <OUT> <CONDITION>

Arguments:
  <CONDITION>  Literal visible label, never an acting agent's success claim

Options:
      --image <IMAGE>

      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --box <BOX>
          Original image pixels: X,Y,W,H
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --kind <KIND>
          Closed screenshot-only condition category [default: label-visible] [possible values: label-visible, banner-absent, not-clipped, non-overlap]
      --target <TARGET>
          Stable source node ID for geometric conditions
      --second-target <SECOND_TARGET>
          Containing panel or second source node ID
      --locate
          Attach advisory phrase localization to check-ui; never establish visibility by detection alone
      --locate-observations <LOCATE_OBSERVATIONS>

      --locate-registry <LOCATE_REGISTRY>

      --locate-cache <LOCATE_CACHE>

      --locate-runtime-library <LOCATE_RUNTIME_LIBRARY>

      --vision-provider <VISION_PROVIDER>
          Prepare Claude/GPT mappings in the assist layer; no live calls [possible values: claude, gpt]
      --vision-response <VISION_RESPONSE>
          Explicit recorded response; bound to this catalog and request
      --experimental
          Required acknowledgement: this feature is unqualified experimental advice
      --out <OUT>
          New empty directory for immutable sidecars and the advice report
      --offline
          Replay existing observations; never authorize providers
      --replay <REPLAY>
          Recorded exact cache entries for offline fixture replay
      --user-config <USER_CONFIG>

      --run
          Explicitly authorize evidence export under fixed user root policy
      --route <ROUTE>
          Deterministic rules, routed cascade or the full visual path [default: cascade] [possible values: rules, cascade, all-vision]
      --jev-routing
          Optional separately measured Jev evidence-need routing; disabled by default
      --json

      --budget-calls <BUDGET_CALLS>
          Real provider request cap; no retries or automatic top-up [default: 4]
      --max-spend-usd <MAX_SPEND_USD>
          Finite per-entry USD ceiling, at most 0.15 [default: 0.15]
      --deadline-secs <DEADLINE_SECS>
          Overall deadline, including both orders and support [default: 300]
      --gemini-revision <GEMINI_REVISION>
          Required immutable returned revision for dispatch/replay
      --jev-revision <JEV_REVISION>
          Required Jev returned revision; the pinned model ID is the initial binding [default: jev-1.13.0]
      --bypass-cache
          Do not reuse cache; required for independent qualification samples
      --source-evidence <SOURCE_EVIDENCE>
          Hash/dimension-bound Wave 3 source packets (at most one per image)
      --incomplete-capture
          Producer states some requested capture scope was not captured
      --pre-masked
          Original pixels were blacked out before capture and are unavailable
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review brand

```text
Review brand colours, theme contrast, CVD and source typography together

Usage: saccade review brand [OPTIONS] --out <OUT> <SOURCE>

Arguments:
  <SOURCE>  Capture-bound source JSON, exported by the colour/DOM/layout producer

Options:
      --config <CONFIG>              Project swatches, profiles and CVD tolerances [default: saccade.toml]
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>                    New review packet JSON file
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review ui

```text
Review source text/layout and localized UI changes in one packet

Usage: saccade review ui [OPTIONS] --out <OUT> <REFERENCE> <CANDIDATE>

Arguments:
  <REFERENCE>
  <CANDIDATE>

Options:
      --reference-source <REFERENCE_SOURCE>
          Source JSON exported by the Playwright ingest, or a DOM/AX producer
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --candidate-source <CANDIDATE_SOURCE>

      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --region <REGION>
          Frozen reference inclusion region; protected complement is exact by default
      --box <BBOX>
          Intended pixel box x,y,width,height
      --ocr-contract <OCR_CONTRACT>
          Optional saccade-tesseract.v1 runtime/model contract; requires the ocr feature
      --perceptual-outside

      --maximum-outside-flip <MAXIMUM_OUTSIDE_FLIP>
          Largest FLIP score allowed outside the intended region, 0-1 (default 0.01; above it fails) [default: 0.01]
      --ppd <PPD>
          Viewing condition in pixels per degree of visual angle (default 67) [default: 67]
      --out <OUT>

      --user-config <USER_CONFIG>

      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review motion

```text
Diagnose dense correspondence and validate supplied renderer vectors

Usage: saccade review motion [OPTIONS] --out <OUT> <REFERENCE> <CANDIDATE>

Arguments:
  <REFERENCE>
  <CANDIDATE>

Options:
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --vectors <VECTORS>
          Row-major saccade-vector-buffer.v1 JSON (requires --sidecar)
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --sidecar <SIDECAR>
          Pinned units, direction, origin and jitter contract (requires --vectors)
      --ppd <PPD>
          Viewing condition in pixels per degree of visual angle (default 67) [default: 67]
      --maximum-raw-mean <MAXIMUM_RAW_MEAN>
          Raw full-frame mean FLIP threshold; motion cannot relax it [default: 0.01]
      --out <OUT>

      --user-config <USER_CONFIG>

      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review request

```text
Prepare a closed request from an existing canonical case, locally

Usage: saccade review request [OPTIONS] --question <QUESTION> --out <OUT> <REPORT>

Arguments:
  <REPORT>

Options:
      --question <QUESTION>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review propose

```text
Validate and record proposed answers against the exact request

Usage: saccade review propose [OPTIONS] --answers <ANSWERS> <REQUEST>

Arguments:
  <REQUEST>

Options:
      --answers <ANSWERS>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --out <OUT>
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review ask

```text
Create or retrieve a local human review item for an unresolved request

Usage: saccade review ask [OPTIONS] <REQUEST>

Arguments:
  <REQUEST>

Options:
      --out <OUT>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review eval

```text
Plan or run a resumable evaluation manifest

Usage: saccade review eval [OPTIONS] --manifest <MANIFEST>

Options:
      --manifest <MANIFEST>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --run
      --user-config <USER_CONFIG>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment

```text
Analyze existing graphics captures: ablation, sequences, ranking, bisection

Usage: saccade experiment [OPTIONS] <COMMAND>

Commands:
  transition  Measure popping, sampled convergence and steady level differences in captures
  animation   Compare timestamp-matched animation captures with localized motion diagnostics
  settle      Event-relative tile error, settling, lag and pre-change residual trajectories
  reference   Compare a render with a noisy offline reference and record alignment/noise floors
  geometry    Measure bidirectional triangle-surface distance and oriented normal deviation
  ablate      Compare ablation arms against a base with image and performance evidence
  temporal    Compare numbered SDR frames with the ColorVideoVDP temporal model
  sequence    Compare numbered colour frames by sorted index and measure added flicker
  rank        Rank candidate directories against one common FLIP reference
  bisect      Find the first diverging run or revision in an ordered series
  safety      Photosensitivity PRE-CHECK only; not certification or formal compliance
  a11y        Accessibility PRE-CHECK only; not certification or formal compliance

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment transition

```text
Measure popping, sampled convergence and steady level differences in captures

Usage: saccade experiment transition [OPTIONS] --out <OUT> --maximum-pop <MAXIMUM_POP> --maximum-duration-ms <MAXIMUM_DURATION_MS> --maximum-steady-error <MAXIMUM_STEADY_ERROR> <PLAN>

Arguments:
  <PLAN>  saccade-captured-sequence-plan.v1 JSON, paths relative to its directory

Options:
      --out <OUT>

      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --json

      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --change-frame <CHANGE_FRAME>
          Zero-based switch request; omit for paired steady levels
      --maximum-pop <MAXIMUM_POP>

      --maximum-duration-ms <MAXIMUM_DURATION_MS>

      --maximum-steady-error <MAXIMUM_STEADY_ERROR>

      --settle-threshold <SETTLE_THRESHOLD>
          [default: 0.02]
      --consecutive <CONSECUTIVE>
          [default: 2]
      --window <WINDOW>
          [default: 2]
      --tile-size <TILE_SIZE>
          [default: 16]
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment animation

```text
Compare timestamp-matched animation captures with localized motion diagnostics

Usage: saccade experiment animation [OPTIONS] --out <OUT> --maximum-frame-error <MAXIMUM_FRAME_ERROR> --maximum-local-error <MAXIMUM_LOCAL_ERROR> --maximum-flicker <MAXIMUM_FLICKER> <PLAN>

Arguments:
  <PLAN>  saccade-captured-sequence-plan.v1 JSON, paths relative to its directory

Options:
      --out <OUT>

      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --json

      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --maximum-frame-error <MAXIMUM_FRAME_ERROR>

      --maximum-local-error <MAXIMUM_LOCAL_ERROR>

      --maximum-flicker <MAXIMUM_FLICKER>

      --tile-size <TILE_SIZE>
          [default: 16]
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment settle

```text
Event-relative tile error, settling, lag and pre-change residual trajectories

Usage: saccade experiment settle [OPTIONS] <FRAMES>

Arguments:
  <FRAMES>  Directory of numbered image frames, sorted by numeric suffix

Options:
      --change-frame <CHANGE_FRAME>
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --event <EVENT>                Bounded JSON event marker containing change_frame
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --reference <REFERENCE>
      --fps <FPS>                    [default: 30]
      --tile-size <TILE_SIZE>        [default: 32]
      --threshold <THRESHOLD>        [default: 0.02]
      --consecutive <CONSECUTIVE>    [default: 3]
      --final-frames <FINAL_FRAMES>  [default: 3]
      --out <OUT>                    [default: settling]
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment reference

```text
Compare a render with a noisy offline reference and record alignment/noise floors

Usage: saccade experiment reference [OPTIONS] <RENDER> <REFERENCE>

Arguments:
  <RENDER>
  <REFERENCE>

Options:
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>
          Intended metadata variables, exact paths, prefixes or suffixes
      --config <CONFIG>

      --seed-reference <SEEDS>
          Additional independent reference seed images
      --variance <VARIANCE>
          Native scalar image of sample-mean variance in linear luminance squared
      --mask <MASK>
          White pixels include the reference/fit scope
      --policy <POLICY>

      --out <OUT>

      --json

  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment geometry

```text
Measure bidirectional triangle-surface distance and oriented normal deviation

Usage: saccade experiment geometry [OPTIONS] --unit <UNIT> <BASELINE> <CAPTURE>

Arguments:
  <BASELINE>
  <CAPTURE>

Options:
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --unit <UNIT>                  Declared common coordinate unit; no conversion or registration is performed
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --samples <SAMPLES>            Approximate area samples per direction, plus mandatory triangle/edge/vertex coverage [default: 4096]
      --views <VIEWS>                Supplied finite-camera render manifest, bound to these exact mesh inputs
      --out <OUT>                    Write the combined geometry and optional multi-view packet
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment ablate

```text
Compare ablation arms against a base with image and performance evidence

Usage: saccade experiment ablate [OPTIONS] [BASE] [ARMS]...

Arguments:
  [BASE]
  [ARMS]...

Options:
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>
          Intended metadata variable for every arm
      --arm-variable <ARM_VARIABLES>
          Arm-specific variable, LABEL=KEY (repeatable; KEY may be a glob)
      --base <RUN_DIR>...
          Base repeat directories. Accepts a directory or a quoted glob; repeatable
      --arm <LABEL=RUN_GLOB>
          Labelled arm repeats, e.g. --arm 's2=s2_r*'; repeatable
      --out <OUT>
          [default: ablation]
      --config <CONFIG>

      --json

      --top <TOP>
          Per-term deltas beyond noise to show per arm [default: 5]
  -h, --help
          Print help

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clock-map <FILE>       TOML/JSON map from producer telemetry to GPU clock evidence
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment temporal

```text
Compare numbered SDR frames with the ColorVideoVDP temporal model

Usage: saccade experiment temporal [OPTIONS] --fps <FPS> <BASELINE_DIR> <CAPTURE_DIR>

Arguments:
  <BASELINE_DIR>  Directory of numbered baseline PNG/JPEG frames
  <CAPTURE_DIR>   Directory of numbered capture PNG/JPEG frames

Options:
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --source-ref <SOURCE_REF>
          External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>
          Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>

      --config <CONFIG>

      --fps <FPS>
          Frame rate used by the temporal visibility model
      --display <DISPLAY>
          Embedded ColorVideoVDP display model [default: standard_4k]
      --pattern <PATTERN>
          Relative-name glob for numbered frames [default: *]
      --out <OUT>
          Output directory for the sequence and temporal reports [default: temporal-report]
      --min-jod <MIN_JOD>
          Optional minimum acceptable video quality in JOD units
      --json
          Print a bounded JSON summary
  -h, --help
          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment sequence

```text
Compare numbered colour frames by sorted index and measure added flicker

Usage: saccade experiment sequence [OPTIONS] <BASELINE_DIR> <CAPTURE_DIR>

Arguments:
  <BASELINE_DIR>
  <CAPTURE_DIR>

Options:
      --fixed-camera                 Declare a fixed camera and measure per-tile flicker with motion qualification
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --pattern <PATTERN>            Relative-name glob; frames must end in an integer before the extension [default: *]
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --out <OUT>                    [default: sequence-report]
      --threshold <THRESHOLD>        FLIP score limit in 0-1 (0 = identical); above it fails
      --metric <METRIC>              [possible values: mean, p95, p99, max]
      --config <CONFIG>
      --ppd <PPD>                    Viewing condition in pixels per degree of visual angle (default 67)
      --fail-on-new
      --allow-empty
      --labels <LABELS>
      --json
  -h, --help                         Print help

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)
      --junit <FILE.xml>              Write one JUnit testcase per entry

Metadata sidecars:
      --meta-name <NAME>
          Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>
          Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>
          Intended experiment metadata variables (exact keys or globs)
      --require-matching-meta
          Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>
          Sidecar keys (or globs) that may differ with --require-matching-meta

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment rank

```text
Rank candidate directories against one common FLIP reference

Usage: saccade experiment rank [OPTIONS] <REFERENCE_DIR> <CANDIDATE_DIRS>...

Arguments:
  <REFERENCE_DIR>
  <CANDIDATE_DIRS>...

Options:
      --labels <LABELS>              One unique, safe directory label per candidate, comma separated
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --metric <METRIC>              [default: mean] [possible values: mean, p95, p99, max]
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --out <OUT>                    [default: rank-report]
      --config <CONFIG>
      --threshold <THRESHOLD>        FLIP score limit in 0-1 (0 = identical); above it fails
      --ppd <PPD>                    Viewing condition in pixels per degree of visual angle (default 67)
      --fail-on-new
      --allow-empty
      --json
  -h, --help                         Print help

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)
      --junit <FILE.xml>              Write one JUnit testcase per entry

Metadata sidecars:
      --meta-name <NAME>
          Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>
          Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --allow-unreached <ALLOW_UNREACHED>
          Permit both unreached arms only under an identical criterion and observation
      --require-valid-arms
          Refuse verdicts for incomplete or mismatched producer identity
      --fingerprint-map <FINGERPRINT_MAP>
          Generic TOML/JSON mapping from producer fields and sibling records
      --arm-ignore <ARM_IGNORE>
          Explicit ignored metadata tokens, echoed in arm validation output
      --intended-variable <INTENDED_VARIABLES>
          Intended experiment metadata variables (exact keys or globs)
      --require-matching-meta
          Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>
          Sidecar keys (or globs) that may differ with --require-matching-meta

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment bisect

```text
Find the first diverging run or revision in an ordered series

Usage: saccade experiment bisect [OPTIONS]

Options:
      --runs <RUNS>...               Ordered run directories, oldest first (repeatable)
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --runs-from <RUNS_FROM>        One ordered run path per line
      --good <GOOD>                  Reference for existing runs (default: first run)
      --threshold <THRESHOLD>        Explicit FLIP threshold relaxes native sample identity
      --metric <METRIC>              mean, p95, p99 or max (default max)
      --entries <ENTRIES>            Select image names by glob
      --out <OUT>                    Report directory, separate from inputs [default: bisect-report]
      --json                         Print saccade-bisect.v1 JSON
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment safety

```text
Photosensitivity PRE-CHECK only; not certification or formal compliance

Usage: saccade experiment safety [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Numbered frames or mp4/mov/mkv (requires external ffmpeg)

Options:
      --fps <FPS>                    Frame rate override; otherwise metadata, or 60 for frame directories
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --display <DISPLAY>            WxH@diagonal_inches,distance_metres (default 1920x1080@55,4)
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --standard <STANDARD>          itu-bt1702 or wcag. PRE-CHECK only, never certification [default: itu-bt1702]
      --json                         Print full saccade-safety.v1 JSON
      --out <OUT>                    Output directory for JSON, text, HTML, static frames and risk heatmaps [default: safety-report]
      --junit <JUNIT>                Optional JUnit XML destination
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment a11y

```text
Accessibility PRE-CHECK only; not certification or formal compliance

Usage: saccade experiment a11y [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Opaque sRGB image or image directory

Options:
      --config <CONFIG>              Explicit saccade.toml with [[region]] kind="text" or "ui"
      --source-ref <SOURCE_REF>      External capture URI/key (repeatable); recorded in generated reports
      --json                         Print full saccade-a11y.v1 JSON
      --report-index <REPORT_INDEX>  Shared report index destination (default reports/index.jsonl next to each report, inside --out)
      --out <OUT>                    Output directory for JSON, text, HTML and simulation/heatmap artifacts [default: a11y-report]
      --junit <JUNIT>                Optional JUnit XML destination
      --suggest-regions              Explicitly upload 16 crops/image to Gemini for unconfirmed region proposals
      --keys-dir <KEYS_DIR>          Judge key policy: gemini.env/SACCADE_GEMINI_API_KEY, never ambient keys
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```
