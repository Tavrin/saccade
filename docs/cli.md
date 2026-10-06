# Command reference

Generated from compiled capabilities and `--help`; do not edit by hand.

Generation: `cargo build --release -p saccade --all-features`, then `python3 scripts/gen-docs.py --saccade target/release/saccade`.
The all-features binary includes every supported operation.

Compiled features: `ai`, `assist`, `compression`, `credentials`, `dense-motion`, `documents`, `embeddings`, `evaluation`, `geometry`, `graphics`, `imgtune-avif`, `local-models`, `local-vlm`, `mcp`, `media-http`, `ocr`, `ocr-provider`, `parallel`, `prechecks`, `products`, `schema`, `semantic-regions`, `vision-providers`, `workbench`.

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
  render-evidence     Compare structural rendering evidence with explicit scope and ID attribution
  review              Preview a review plan or handle a local closed decision request
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
  -h, --help     Print help
  -V, --version  Print version

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output


Start here:
  saccade compare baseline/ captures/ --out report
  saccade prove identity parent/ candidate/ --out proof
  saccade prove performance --base 'base_r*' --arm 'candidate=candidate_r*'
  saccade review report/saccade-report.v1.json --out review

Exit codes: 0 no image regression, 1 image regression found, 2 the command could not run.
Advanced: demo, identity, noise, view, inspect, experiment, approve, init,
serve, mcp, ingest, bisect, history, doctor. Existing commands keep working; use `saccade COMMAND --help`.
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
      --config <CONFIG>

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
  -h, --help  Print help

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
  -h, --help  Print help

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
      --json
  -h, --help       Print help

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
  -h, --help  Print help

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
  -h, --help  Print help

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
  -h, --help  Print help

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
  -h, --help  Print help

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
      --vary <VARY>
          Allowed difference matching destination or mapped source: exact key, dotted prefix, suffix or glob; repeat or comma-separate
      --allow-unreached <ALLOW_UNREACHED>
          Permit intentionally unreached captures with exactly matching observations
      --ignore <IGNORE>
          Explicit exception matching destination or mapped source, echoed even if unmatched; repeat or comma-separate
      --fingerprint-map <FINGERPRINT_MAP>

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
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade sweep plan

```text
Sample a URL list or bounded sitemap tree into a driver-neutral manifest

Usage: saccade sweep plan [OPTIONS] --before-origin <BEFORE_ORIGIN> --after-origin <AFTER_ORIGIN> --out <OUT>

Options:
      --urls <URLS>
      --sitemap <SITEMAP>
      --before-origin <BEFORE_ORIGIN>
      --after-origin <AFTER_ORIGIN>
      --samples <SAMPLES>              [default: 3]
      --seed <SEED>                    [default: 42]
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
      --out <OUT>
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
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade imgtune audit

```text
Record actual HTTP content negotiation, bytes and decoded dimensions

Usage: saccade imgtune audit [OPTIONS] --urls <URLS> --accept <ACCEPT> --out <OUT>

Options:
      --urls <URLS>
      --accept <ACCEPT>
      --out <OUT>
      --json
  -h, --help             Print help

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
      --json
  -h, --help       Print help

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
  -h, --help  Print help

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
      --cache <CACHE>
      --fixture-dir <FIXTURE_DIR>
      --scale <SCALE>              [default: 1]
      --json
  -h, --help                       Print help

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
      --captures <CAPTURES>
      --out <OUT>
      --config <CONFIG>
      --align <ALIGN>        [default: translation] [possible values: none, translation]
      --json
  -h, --help                 Print help

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
      --template <TEMPLATE>        [default: generic] [possible values: generic, slack, teams]
      --report-link <REPORT_LINK>  Display link; defaults to the report path. Never used as the webhook endpoint
      --json
  -h, --help                       Print help

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
  -h, --help  Print help

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
      --face-observations <FACE_OBSERVATIONS>
          Image-bound face receipt; explicitly labelled replay
      --model-registry <MODEL_REGISTRY>
          Shared model registry (vision, embedding and OCR pins)
      --model-cache <MODEL_CACHE>

      --runtime-library <RUNTIME_LIBRARY>

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
      --face-observations <FACE_OBSERVATIONS>
          Image-bound face receipt; explicitly labelled replay
      --model-registry <MODEL_REGISTRY>
          Shared model registry (vision, embedding and OCR pins)
      --model-cache <MODEL_CACHE>

      --runtime-library <RUNTIME_LIBRARY>

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
      --ocr-model <OCR_MODEL>
          Explicit dated OCR model (aliases refused)
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
          Override the default pinned PP-OCRv5 contract (or select external Tesseract)
      --download-model
          Explicitly fetch SHA-pinned Rust OCR models into the contract cache
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

## saccade similar

```text
Cosine similarity with an explicitly pinned optional ONNX export

Usage: saccade similar [OPTIONS] --model <MODEL> --cache <CACHE> --library <LIBRARY> <A> <B>

Arguments:
  <A>
  <B>

Options:
      --model <MODEL>      Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing
      --cache <CACHE>      Content-addressed model cache; downloads require --download-model
      --library <LIBRARY>  Explicit ONNX Runtime 1.22 dynamic library, CPU execution only
      --download-model     Explicitly allow the pinned export to be downloaded to the cache
      --out <OUT>          [default: similar-report]
      --json
  -h, --help               Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index

```text
Build or query a streaming exact flat embedding index

Usage: saccade index [OPTIONS] <COMMAND>

Commands:
  export-inputs  Write exact Rust-preprocessed tensors for independent checkpoint/export parity
  calibrate      Run pinned export parity and fit/holdout calibration over a frozen corpus (heavy)
  build          Build a streaming exact flat index, up to 100000 images and 512 MiB vectors
  query          Search an existing index; model/preprocessing must exactly match the index

Options:
  -h, --help  Print help

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
      --out <OUT>
      --json
  -h, --help           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index calibrate

```text
Run pinned export parity and fit/holdout calibration over a frozen corpus (heavy)

Usage: saccade index calibrate [OPTIONS] --model <MODEL> --cache <CACHE> --library <LIBRARY> --out <OUT> <CORPUS>

Arguments:
  <CORPUS>

Options:
      --model <MODEL>      Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing
      --cache <CACHE>      Content-addressed model cache; downloads require --download-model
      --library <LIBRARY>  Explicit ONNX Runtime 1.22 dynamic library, CPU execution only
      --download-model     Explicitly allow the pinned export to be downloaded to the cache
      --out <OUT>
      --json
  -h, --help               Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index build

```text
Build a streaming exact flat index, up to 100000 images and 512 MiB vectors

Usage: saccade index build [OPTIONS] --model <MODEL> --cache <CACHE> --library <LIBRARY> --out <OUT> <DIR>

Arguments:
  <DIR>

Options:
      --model <MODEL>      Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing
      --cache <CACHE>      Content-addressed model cache; downloads require --download-model
      --library <LIBRARY>  Explicit ONNX Runtime 1.22 dynamic library, CPU execution only
      --download-model     Explicitly allow the pinned export to be downloaded to the cache
      --out <OUT>
      --json
  -h, --help               Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade index query

```text
Search an existing index; model/preprocessing must exactly match the index

Usage: saccade index query [OPTIONS] --model <MODEL> --cache <CACHE> --library <LIBRARY> <INDEX> [IMAGE]

Arguments:
  <INDEX>
  [IMAGE]

Options:
      --text <TEXT>        Text query requires a pinned SigLIP 2 joint text/image model
      --model <MODEL>      Supplied saccade-embedding-model.v1 contract; includes export SHA-256 and preprocessing
      --cache <CACHE>      Content-addressed model cache; downloads require --download-model
      --library <LIBRARY>  Explicit ONNX Runtime 1.22 dynamic library, CPU execution only
      --download-model     Explicitly allow the pinned export to be downloaded to the cache
      --top <TOP>          [default: 10]
      --out <OUT>          [default: query-report]
      --json
  -h, --help               Print help

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
      --out <OUT>  New or empty output directory [default: hash-report]
      --json       Emit a bounded JSON artifact receipt
  -h, --help       Print help

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
      --algorithm <ALGORITHM>  Algorithm for the Hamming index [default: phash] [possible values: ahash, dhash, phash]
      --threshold <THRESHOLD>  Inclusive Hamming radius in 0..64; clusters use transitive connectivity [default: 6]
      --out <OUT>              New or empty output directory [default: dedupe-report]
      --json                   Emit a bounded JSON artifact receipt
  -h, --help                   Print help

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
      --profile <PROFILE>          [default: cpu-lite] [possible values: cpu-lite, cpu-full, gpu]
      --model-dir <MODEL_DIR>
      --registry <REGISTRY>
      --options <OPTIONS>          Per-section options JSON file
      --strict
      --output-size <OUTPUT_SIZE>  Repeat output size WxH
      --json
  -h, --help                       Print help

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
      --sample-fps <SAMPLE_FPS>      [default: 1]
      --shot-penalty <SHOT_PENALTY>  [default: 0.15]
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
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade models

```text
List or explicitly pull pinned local models

Usage: saccade models [OPTIONS] <COMMAND>

Commands:
  list  Inspect selections, real pins, cache integrity and source-parity status
  pull  Explicit opt-in to download only the named model's pinned artifacts

Options:
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade models list

```text
Inspect selections, real pins, cache integrity and source-parity status

Usage: saccade models list [OPTIONS]

Options:
      --registry <REGISTRY>
      --cache <CACHE>
      --json
  -h, --help                 Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade models pull

```text
Explicit opt-in to download only the named model's pinned artifacts

Usage: saccade models pull [OPTIONS] <ID>

Arguments:
  <ID>

Options:
      --registry <REGISTRY>
      --cache <CACHE>
      --json
  -h, --help                 Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
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

      --detector <DETECTOR>
          [default: grounding-dino-tiny]
      --segmenter <SEGMENTER>
          [default: sam-2.1-tiny]
      --observations <OBSERVATIONS>
          Explicit generated/frozen observation receipt; output is labelled replay
      --overlay <OVERLAY>
          New overlay PNG; existing files are never overwritten
      --registry <REGISTRY>

      --cache <CACHE>

      --runtime-library <RUNTIME_LIBRARY>
          ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download

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
      --metric <METRIC>
          [default: musiq]
      --observations <OBSERVATIONS>
          Explicit stand-in/frozen measurement receipt, always labelled replay
      --registry <REGISTRY>

      --cache <CACHE>

      --runtime-library <RUNTIME_LIBRARY>
          ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download

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
      --quantization-step <QUANTIZATION_STEP>
          [default: 36]
      --minimum-agreement <MINIMUM_AGREEMENT>
          [default: 0.9]
      --observations <OBSERVATIONS>
          Explicit frozen/generated primary-decoder observation report
      --trustmark
          Run the pinned Q neural graph; ECC/resize qualification remains unavailable
      --registry <REGISTRY>

      --cache <CACHE>

      --runtime-library <RUNTIME_LIBRARY>
          ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download

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
      --observations <OBSERVATIONS>

      --blur-faces <BLUR_FACES>
          Write a new strongly redacted PNG; never overwrite an original
      --registry <REGISTRY>

      --cache <CACHE>

      --runtime-library <RUNTIME_LIBRARY>
          ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download

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
      --observations <OBSERVATIONS>

      --blur-faces <BLUR_FACES>
          Write a new strongly redacted PNG; never overwrite an original
      --registry <REGISTRY>

      --cache <CACHE>

      --runtime-library <RUNTIME_LIBRARY>
          ONNX Runtime 1.22 library; otherwise ORT_DYLIB_PATH or verified model cache
      --allow-download

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

      --endpoint-profile <ENDPOINT_PROFILE>
          Startup env-file mapping for generic OpenAI-compatible or Azure deployment endpoints [possible values: openai-compatible, azure-openai]
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
      --out <OUT>  New JSON localization report
      --json
  -h, --help       Print help

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
  -h, --help  Print help

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
      --mask <MASK>
      --phrase <PHRASE>
      --out <OUT>              New frozen-region JSON file; use it with localized-check --region
  -h, --help                   Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade regions status

```text
Report honest text-to-mask and import capabilities without loading models

Usage: saccade regions status [OPTIONS]

Options:
  -h, --help  Print help

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
      --cache <CACHE>
  -h, --help                 Print help

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
      --cache <CACHE>
      --library <LIBRARY>
  -h, --help                 Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade explain-grounded

```text
Render verified atomic numerical claims with region and evidence citations

Usage: saccade explain-grounded [OPTIONS] --report <REPORT> --out <OUT>

Options:
      --report <REPORT>        Immutable comparison or localized measurement JSON
      --proposals <PROPOSALS>  Optional JSON array of atomic proposals; no provider calls are made
      --out <OUT>              New explanation JSON file
      --json
  -h, --help                   Print help

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
          [default: 0.01]
      --ppd <PPD>
          [default: 67]
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
      --manifest <MANIFEST>  Expected suite and supplied capture attempts, with stable case IDs
      --report <REPORT>      Existing comparison report
      --out <OUT>            New inventory JSON file
      --json
  -h, --help                 Print help

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
      --out <OUT>  New JSON report file; existing files are preserved
      --json
  -h, --help       Print help

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
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade history onset

```text
Find candidate performance onsets in qualified, comparable history observations

Usage: saccade history onset [OPTIONS] --store <STORE>

Options:
      --store <STORE>
      --limit <LIMIT>  Most recent distinct observations per partition; exact DP is bounded to 120 [default: 60]
      --json
  -h, --help           Print help

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
      --environment-id <ENVIRONMENT_ID>
          Frozen browser/device, fonts, viewport, warmup and temporal protocol identity
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
      --store <STORE>
      --entry <ENTRY>
      --drift          Diagnose sustained anchor-relative drift in recorded run order
      --out <OUT>      New file containing the complete witness for the selected groups
      --limit <LIMIT>  [default: 10]
      --json
  -h, --help           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade bisect

```text
Locate the first commit whose fresh capture fails its baseline

Usage: saccade bisect [OPTIONS] --capture <CAPTURE> --baseline <BASELINE>

Options:
      --good <GOOD>          Known good revision in the current repository
      --bad <BAD>            Known bad revision descended from --good
      --capture <CAPTURE>    Shell capture command; write images to SACCADE_CAPTURE_DIR (sh on Unix, cmd on Windows)
      --baseline <BASELINE>  Stable baseline directory, copied before Git changes revisions
      --perf                 Require qualified performance evidence and count a slower frame as bad
      --out <OUT>            Evidence directory outside the repository; defaults to a new sibling
      --json                 Print bounded JSON
  -h, --help                 Print help

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
  -h, --help  Print help

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
      --json
  -h, --help       Print help

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
      --json
  -h, --help       Print help

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
      --json
  -h, --help       Print help

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
      --json
  -h, --help       Print help

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
      --out <OUT>              New directory for paired inputs and the comparison report
      --json                   Print the bounded comparison result
      --threshold <THRESHOLD>  FLIP threshold for the comparison
  -h, --help                   Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade doctor

```text
Print installed version, features and supported evidence schemas

Usage: saccade doctor [OPTIONS]

Options:
      --json  Print machine-readable JSON
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade init

```text
Bootstrap a commented configuration and print baseline adoption steps

Usage: saccade init [OPTIONS]

Options:
      --template <TEMPLATE>  [default: renderer] [possible values: renderer, ui, identity, ml, ci, nightly, lookdev]
      --dir <DIR>            [default: .]
      --force
  -h, --help                 Print help

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
      --out <OUT>  Directory for the demo images and reports (default: a new temporary directory)
  -h, --help       Print help

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
      --question <QUESTION>
          Explicit comparison question; no automatic model fallback [possible values: same-render, same-content, same-text, near-duplicate, quality]
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
      --threshold <THRESHOLD>  Default pass threshold (overrides the config file's top level)
      --metric <METRIC>        Default deciding metric (overrides the config file's top level) [possible values: mean, p95, p99, max]
      --config <CONFIG>        Config file; defaults to ./saccade.toml when it exists
      --fail-on-new            Treat new images (no baseline) as a regression
      --allow-empty            Accept a run that compared no pair (for example the first run, with an empty baseline directory). Without it, nothing compared exits 1
      --ppd <PPD>              FLIP pixels per degree

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
  -h, --help  Print help

Output:
      --out <OUT>         Report output directory [default: report]
      --json              Print a bounded machine-readable result
      --labels <A,B>      Display names of the two sides, `parent,candidate`
      --junit <FILE.xml>  Write one JUnit testcase per entry

Gate:
      --allow-empty      Accept a run that compared no pair. Without it, nothing compared exits 1
      --config <CONFIG>  Config file; defaults to ./saccade.toml when it exists
      --ppd <PPD>        FLIP pixels per degree, used only to describe differences

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
  -h, --help  Print help

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
      --unit <UNIT>  Declared common coordinate unit; no conversion or registration is performed
      --json
  -h, --help         Print help

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
      --out <OUT>         [default: report]
      --config <CONFIG>
      --json
      --allow-empty
      --ppd <PPD>
      --labels <A,B>
      --junit <FILE.xml>
      --entry <GLOB>
  -h, --help              Print help

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
      --kind <KIND>  Image calibration (default) or qualified performance noise in ms [default: image] [possible values: image, performance]
  -h, --help         Print help

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)
      --config <CONFIG>
      --margin <MARGIN>            [default: 1.5]
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
  -h, --help  Print help

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

Comparison:
      --reference <REFERENCE>  FLIP reference: a label or one of the directories (default: the first)
      --ppd <PPD>              FLIP pixels per degree
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
      --entry <NAME>                 Select a report entry without positional directories; repeatable
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
      --api-max-bytes <API_MAX_BYTES>
          [default: 16777216]
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
          Storage deadline in milliseconds (default: 3000)
      --port <PORT>
          Port on 127.0.0.1 (0 picks a free one) [default: 7878]
      --cache-dir <CACHE_DIR>
          Cache directory for sessions, thumbnails and uploads (default: `$XDG_CACHE_HOME/saccade`)
      --decisions-dir <DECISIONS_DIR>
          Directory the viewer's decisions are written to (default: `$XDG_DATA_HOME/saccade/decisions`)
      --config <CONFIG>
          Config file for sidecar settings and preset regions (default: `./saccade.toml` when present)
      --ppd <PPD>
          FLIP pixels per degree
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
      --out-root <OUT_ROOT>           Generated artifacts require this separate root
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
      --entry <ENTRY>
      --validity-reasons                     List every capture-validity reason, with pagination
      --status <STATUS>
      --limit <LIMIT>                        [default: 10]
      --cursor <CURSOR>
      --expected-case-id <EXPECTED_CASE_ID>
      --json
  -h, --help                                 Print help

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
  -h, --help  Print help

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
      --entry <ENTRIES>
      --top <TOP>          [default: 5]
      --stretch
      --blind
      --key-out <KEY_OUT>
      --seed <SEED>
      --json
  -h, --help               Print help

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
      --out <OUT>
      --entry <ENTRY>
      --state <STATE>
      --width <WIDTH>                [default: 1024]
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
      --entry <PATH_OR_NAME>
      --json
  -h, --help                  Print help

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
  -h, --help  Print help

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
      --run
      --budget-calls <BUDGET_CALLS>
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
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --voter <VOTER>
      --item <ITEM>
      --answer <ANSWER>
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --voter <VOTER>
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review assist batch submit

```text
Verify and submit once; ambiguous submissions cannot repeat

Usage: saccade review assist batch submit [OPTIONS] --plan <PLAN> --job <JOB>

Options:
      --plan <PLAN>                    Source-bound saccade-assist-batch-plan.v1 artifact
      --job <JOB>                      Durable receipt under the output root
      --experimental
      --run                            Authorize one live submission or one poll; default local only
      --response <RESPONSE>            Recorded collection fixture; cannot settle a live reservation
      --budget-calls <BUDGET_CALLS>    [default: 8]
      --deadline-secs <DEADLINE_SECS>  [default: 300]
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
      --plan <PLAN>                    Source-bound saccade-assist-batch-plan.v1 artifact
      --job <JOB>                      Durable receipt under the output root
      --experimental
      --run                            Authorize one live submission or one poll; default local only
      --response <RESPONSE>            Recorded collection fixture; cannot settle a live reservation
      --budget-calls <BUDGET_CALLS>    [default: 8]
      --deadline-secs <DEADLINE_SECS>  [default: 300]
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
      --plan <PLAN>                    Source-bound saccade-assist-batch-plan.v1 artifact
      --job <JOB>                      Durable receipt under the output root
      --experimental
      --run                            Authorize one live submission or one poll; default local only
      --response <RESPONSE>            Recorded collection fixture; cannot settle a live reservation
      --budget-calls <BUDGET_CALLS>    [default: 8]
      --deadline-secs <DEADLINE_SECS>  [default: 300]
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

      --entry <ENTRY>
          Select exactly one report entry; required when the report contains several
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

      --entry <ENTRY>
          Select exactly one report entry; required when the report contains several
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

      --box <BOX>
          Original image pixels: X,Y,W,H
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
      --config <CONFIG>            Project swatches, profiles and CVD tolerances [default: saccade.toml]
      --out <OUT>                  New review packet JSON file
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --candidate-source <CANDIDATE_SOURCE>

      --region <REGION>
          Frozen reference inclusion region; protected complement is exact by default
      --box <BBOX>
          Intended pixel box x,y,width,height
      --ocr-contract <OCR_CONTRACT>
          Optional saccade-tesseract.v1 runtime/model contract; requires the ocr feature
      --perceptual-outside

      --maximum-outside-flip <MAXIMUM_OUTSIDE_FLIP>
          [default: 0.01]
      --ppd <PPD>
          [default: 67]
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
      --vectors <VECTORS>
          Row-major saccade-vector-buffer.v1 JSON (requires --sidecar)
      --sidecar <SIDECAR>
          Pinned units, direction, origin and jitter contract (requires --vectors)
      --ppd <PPD>
          [default: 67]
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
      --out <OUT>
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --out <OUT>
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

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
      --run
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment

```text
Analyze existing graphics captures: ablation, sequences, ranking, bisection

Usage: saccade experiment [OPTIONS] <COMMAND>

Commands:
  reference  Compare a render with a noisy offline reference and record alignment/noise floors
  geometry   Measure bidirectional triangle-surface distance and oriented normal deviation
  ablate     Compare ablation arms against a base with image and performance evidence
  temporal   Compare numbered SDR frames with the ColorVideoVDP temporal model
  sequence   Compare numbered colour frames by sorted index and measure added flicker
  rank       Rank candidate directories against one common FLIP reference
  bisect     Find the first diverging run or revision in an ordered series
  safety     Photosensitivity PRE-CHECK only; not certification or formal compliance
  a11y       Accessibility PRE-CHECK only; not certification or formal compliance

Options:
  -h, --help  Print help

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
      --unit <UNIT>        Declared common coordinate unit; no conversion or registration is performed
      --samples <SAMPLES>  Approximate area samples per direction, plus mandatory triangle/edge/vertex coverage [default: 4096]
      --views <VIEWS>      Supplied finite-camera render manifest, bound to these exact mesh inputs
      --out <OUT>          Write the combined geometry and optional multi-view packet
      --json
  -h, --help               Print help

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
      --fixed-camera           Declare a fixed camera and measure per-tile flicker with motion qualification
      --pattern <PATTERN>      Relative-name glob; frames must end in an integer before the extension [default: *]
      --out <OUT>              [default: sequence-report]
      --threshold <THRESHOLD>
      --metric <METRIC>        [possible values: mean, p95, p99, max]
      --config <CONFIG>
      --ppd <PPD>
      --fail-on-new
      --allow-empty
      --labels <LABELS>
      --json
  -h, --help                   Print help

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
      --labels <LABELS>        One unique, safe directory label per candidate, comma separated
      --metric <METRIC>        [default: mean] [possible values: mean, p95, p99, max]
      --out <OUT>              [default: rank-report]
      --config <CONFIG>
      --threshold <THRESHOLD>
      --ppd <PPD>
      --fail-on-new
      --allow-empty
      --json
  -h, --help                   Print help

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
      --runs <RUNS>...         Ordered run directories, oldest first (repeatable)
      --runs-from <RUNS_FROM>  One ordered run path per line
      --good <GOOD>            Reference for existing runs (default: first run)
      --threshold <THRESHOLD>  Explicit FLIP threshold relaxes native sample identity
      --metric <METRIC>        mean, p95, p99 or max (default max)
      --entries <ENTRIES>      Select image names by glob
      --out <OUT>              Report directory, separate from inputs [default: bisect-report]
      --json                   Print saccade-bisect.v1 JSON
  -h, --help                   Print help

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
      --fps <FPS>            Frame rate override; otherwise metadata, or 60 for frame directories
      --display <DISPLAY>    WxH@diagonal_inches,distance_metres (default 1920x1080@55,4)
      --standard <STANDARD>  itu-bt1702 or wcag. PRE-CHECK only, never certification [default: itu-bt1702]
      --json                 Print full saccade-safety.v1 JSON
      --out <OUT>            Output directory for JSON, text, HTML, static frames and risk heatmaps [default: safety-report]
      --junit <JUNIT>        Optional JUnit XML destination
  -h, --help                 Print help

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
      --config <CONFIG>      Explicit saccade.toml with [[region]] kind="text" or "ui"
      --json                 Print full saccade-a11y.v1 JSON
      --out <OUT>            Output directory for JSON, text, HTML and simulation/heatmap artifacts [default: a11y-report]
      --junit <JUNIT>        Optional JUnit XML destination
      --suggest-regions      Explicitly upload 16 crops/image to Gemini for unconfirmed region proposals
      --keys-dir <KEYS_DIR>  Judge key policy: gemini.env/SACCADE_GEMINI_API_KEY, never ambient keys
  -h, --help                 Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```
