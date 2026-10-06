# Conditional embedding similarity

Build with the `embeddings` feature. It reuses `ort` 2.0.0-rc.10's dynamic CPU
ONNX Runtime 1.22 loader and the existing hash-verified runtime cache. No native
runtime, export or weights are bundled, and the default features do not change.

```sh
saccade similar a.png b.png --model model.json --cache models --library runtime.so --out similarity --json
saccade index build images/ --model model.json --cache models --library runtime.so --out archive --json
saccade index query archive/ query.png --model model.json --cache models --library runtime.so --top 10 --out matches --json
```

The supplied `saccade-embedding-model.v1` contract declares `family:
"dinov2-small"`, an Apache-2.0 self-contained ONNX artifact with its exact
`sha256`, `bytes`, revision-pinned HTTPS `url`, `version`, `role`, `format:
"onnx"` and `license`. It also declares input/output tensor names, resize
`size: [width,height]`, RGB `mean` and `std`, and output `dimensions` (1..4096).
The runtime accepts NCHW f32 inputs and a pooled `[1,dimensions]` f32 output.
Preprocessing is explicit triangle resize, white alpha compositing, division by
255, then mean/std normalization. Exports requiring center crops, token selection
or a different tensor contract need a matching export; they are not guessed.

`--download-model` explicitly enables the existing SHA-pinned runtime downloader.
Without it, the content-addressed cache must already contain the artifact. The
cache and runtime must be provisioned by the operator. MCP operations `similar`,
`index_build`, `index_query` mirror the measurements with `model`, `cache`,
`library`, `out` and their image/directory arguments. Downloads remain explicit
CLI-only cache provisioning; MCP performs no network calls.

Cosine uses finite, nonzero L2-normalized vectors. Optional `calibration` contains
`corpus_sha256`, `scope` and ordered `bands: [{minimum, label}]`. Bands are
reported as supplied calibration requiring external qualification. Absent
calibration yields a null band and `uncalibrated`; no arbitrary threshold becomes
a calibrated semantic verdict. `similar`/query return `verdict: unknown` and exit
0 after successful execution. Neither raw cosine nor a supplied band proves
image identity, OCR equality or baseline approval.

The flat index streams `vectors.bin` (f32 little endian) beside
`saccade-embedding-index.v1.json`. It supports <=100000 images and <=512 MiB
vectors, with 64 MiB input paths. Build decodes each image once and records
failures. Query verifies metadata/model identity, vector size and SHA-256, then
performs exact cosine search in bounded memory. Results break ties by row order.
File paths are provenance; query does not revalidate current archive source bytes.
The JSON receipt points to full versioned similarity, index or query evidence.
Invalid models/indexes/runtime failures exit 2; a partial/empty build exits 1.

DINOv2-small's checkpoint licence (Apache-2.0) is given by the lane specification.
No canonical ONNX export URL/SHA, export parity receipt or calibrated corpus was
provided. Wave 6b adds the export/parity/holdout jobs below. A preselected canonical
model and actual calibration qualification remain deferred. CLIP/text-to-image retrieval
is deferred until a reviewed model licence and pinned export are available.
Model-dependent tests are ignored under `heavy: embeddings`; the integrator
supplies `SACCADE_W6_EMBEDDING_MODEL`, `SACCADE_W6_MODEL_CACHE`, and
`SACCADE_W6_ORT_LIBRARY` before running them. No model tests ran in this lane.

MCP native execution requires an operator-owned
`$XDG_CONFIG_HOME/saccade/onnx-runtime.json` (default
`~/.config/saccade/onnx-runtime.json`). Its `saccade-onnx-runtime-authority.v1`
object contains `library` (the canonical operator-approved native runtime path)
and `sha256` (64 lowercase hexadecimal characters). Both must match the request;
the runtime is hashed with a 512 MiB bound. On Unix the configuration root,
`saccade` directory and file must be owned by the home-directory owner, not
symlinks, and not group/world writable. All requested paths still need startup
root access. Absent/mismatching authority returns `execution_authorization_required`.
This prevents a request from turning read access into arbitrary native execution;
it does not sandbox the approved native runtime. The library must remain immutable
during a session. CLI explicitly supplied library paths retain operator authority.
The ONNX export is loaded from its rehashed bounded memory buffer; file-based
external initializers are not provisioned by this interface.

## Export and calibration job (Wave 6b)

The executable plumbing now replaces the missing export/qualification procedure:

```sh
# No model execution: prepare exact NCHW tensors using a supplied model template.
saccade index export-inputs fixtures/ --model template.json --out prepared --json
# Heavy, offline job: operator-reviewed source/checkpoint, no automatic weights pull.
python3 scripts/export-wave6-embeddings.py --inputs prepared/saccade-embedding-export-inputs.v1.json --source SOURCE_DIR --source-revision FULL_SHA --checkpoint checkpoint.pth --checkpoint-sha256 CHECKPOINT_SHA --pairs pairs.json --scope 'declared corpus scope' --artifact-url https://example.org/revision/model.onnx --out exported
# Cache the exported graph under its actual SHA-256, or explicitly runtime-download it.
saccade index calibrate exported/corpus.json --model exported/model.json --cache models --library runtime.so --out qualification --json
```

Templates use the existing model contract; artifact fields are required for
validation, but tensor preparation never loads the artifact. The exporter
replaces those fields with the actual self-contained ONNX graph hash/size and
operator-declared revision URL. It loads DINOv2-small from a clean local source
checkout at the declared revision and a hash-pinned checkpoint with
`pretrained=False`. Operator-installed torch/numpy/onnx versions and script,
source, checkpoint, input and pair identities appear in `export-receipt.json`.
It rejects external ONNX initializers. No models were fetched during development.
The built-in checkpoint licence is Apache-2.0 per the binding specification;
no universal or fabricated canonical ONNX pin is supplied.

`pairs.json` is a frozen array of `{a,b,split,same_content}` sample indices,
with `split: fit|holdout`. Both splits need positive/negative labels and must
use disjoint sample indices; duplicate image hashes/pairs and path escapes are
rejected. Up to 128 samples and 4096 pairs are supported. Corpus JSON binds
source/checkpoint/export pins, encoded sample and exact tensor hashes, source
checkpoint embeddings, scope and a fixed parity tolerance <=0.001. Independent
checkpoint embeddings use those same Rust tensors, avoiding mismatched image
preprocessing. Every ONNX vector must meet component and cosine parity limits.

Calibration chooses a separating cosine boundary from fit labels only, then
checks the frozen holdout without retuning. Overlapping fit labels fail. A
holdout/parity failure writes a regression receipt and no calibrated model.
Passing writes `model.json` and `saccade-embedding-qualification.v1.json`, binding
its model contract hash. Qualification is scoped to supplied source vectors
and truth labels; their provenance is recorded, not independently attested.
Supplied calibration in the existing similarity output retains its cautious
status; merely supplying bands does not authenticate a qualification receipt.

MCP operations `embedding_export_inputs` and `embedding_calibrate` mirror these
local commands; calibration retains operator-owned ONNX library authority and
never downloads. The Python source/checkpoint exporter is an operator job,
not executable through MCP.

The heavy gate requires `SACCADE_W6_EMBEDDING_MODEL`, `SACCADE_W6_MODEL_CACHE`,
`SACCADE_W6_ORT_LIBRARY`, `SACCADE_W6_EMBEDDING_CORPUS` and
`SACCADE_W6_EMBEDDING_CORPUS_SHA256`. Optional `SACCADE_W6_EMBEDDING_RECEIPT`
retains its qualification result. Export/calibration/holdout execution remains
NOT RUN; CLIP and a preselected universal export/calibration remain deferred.

## Joint image/text retrieval

A `siglip2-base` model contract adds a `text` tower with pinned ONNX and tokenizer
artifacts, `input_ids`, `embedding`, context length 64 and right padding id 0.
The image tower uses 224x224 RGB, triangle resize, white alpha compositing,
and mean/std 0.5. Historical DINOv2 contracts retain their identities and remain
image-only. Every graph, tokenizer and preprocessing field binds the index.

```sh
saccade index query archive/ --text "a red square" --model joint-model.json --cache models --library runtime.so --out text-matches --json
```

`scripts/models/export-siglip2.py` reproduces separate CPU exports from official
`google/siglip2-base-patch16-224` revision
`75de2d55ec2d0b4efc50b3e9ad70dba96a7b2fa2`. The upstream model card declares
Apache-2.0. The script verifies every downloaded SHA-256 and loads only local
files. Exports are cache-only artifacts, with no invented hosted export URL.
Float16 text weights retain float32 outputs; fixed normalized checkpoint parity
must pass before the script writes a usable contract. That receipt covers three
text prompts and one tensor, not broad retrieval accuracy. Bands stay uncalibrated.
The shared `Analyzer` and Python/HTTP index use the same installed joint contract.
