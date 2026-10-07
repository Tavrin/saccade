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

## Incremental segmented archives

`index build` retains the historical flat output. Opt in to v2 for a larger or
changing archive (the same configured embedding model/runtime is required):

```sh
saccade index build images/ --segmented --model model.json --out archive --json
saccade index update archive/ images/ --model model.json --out update-receipt --json
saccade index update archive/ images/ --prune --model model.json --out prune-receipt --json
```

Update treats relative UTF-8 source paths as keys. It hashes encoded bytes and
skips inference for unchanged entries; changed bytes replace the existing vector.
Without `--prune`, absent paths stay indexed. With it, `images/` must be the complete
archive: every missing path is removed. Traversal, decode, inference or contract
failures abort the transaction. An empty segmented build exits 1; pruning the last
entry is a successful update, but querying an empty archive is an error (exit 2).
No model downloads occur unless explicitly enabled through existing provisioning.

The `saccade-embedding-index.v2` manifest binds the full serialized model contract,
including graph hashes, tokenizer, preprocessing and supplied calibration. The
core `general::embedding_index::Update::upsert` also requires its contract digest
for every supplied vector; caller-supplied vectors retain caller-owned inference
provenance. Indexes from a different contract are refused even at equal dimensions.
The public reader and existing CLI/MCP image/text queries accept both versions;
Python/HTTP library queries stream v2 through the shared reader. Loaded v2 library
snapshots are read-only; use the core transactional writer to modify them. Legacy
relative normalized labels migrate on first update; other historical provenance
labels remain readable but require relabeling before migration.

V2 uses 256 SHA-256 path shards, sorted within each shard, split into immutable
segments of at most 4096 rows. Only affected shards are rewritten; unchanged
shards retain their files. Exact cosine search verifies each segment's metadata
and vector hashes, then streams one vector at a time. Ties use lexical paths;
v1 retains its historical row-order ties. Row numbers describe the current
snapshot and may change after updates; source paths are the durable keys. A full
segmented rebuild produces the same source ids, row numbers and scores as an
incremental build of the same archive. Query does not rehash current source files.

Explicit limits: 1,000,000 live rows, 16 GiB live vectors, 256 MiB total path text,
4096 segments, 4096 dimensions, 32 MiB metadata per segment, 4 MiB manifest,
64 MiB encoded bytes per input, and top-k 1..100. Query memory is bounded by the
manifest, one segment's metadata, one vector and top-k hits; it does not grow with
total vector bytes. Writers retain bounded path/row metadata and an on-disk vector
spool (maximum 16 GiB), rather than loading all vectors. Disk planning must allow
old generations plus staging and changed shards during publication.

On local Unix filesystems, writers take an OS file lock, sync new immutable
segments and their directory, sync a temporary manifest, atomically rename it,
then sync the directory again. A process interruption before publication leaves
the previous manifest readable; already-open readers retain their snapshot.
A post-rename sync error may report an error after a complete new manifest has
become visible. Power-loss durability depends on filesystem/device sync semantics;
non-Unix directory durability has not been qualified. Failed or interrupted
updates may leave unreferenced files. Offline cleanup requires excluding writers
and readers, retaining files referenced by the current manifest, plus v1 files
when legacy consumers still need them; there is no automatic online deletion.

Build/update evidence uses `saccade-embedding-index-update.v1`; query evidence
keeps the existing query contract. A v1 archive remains intact during migration;
updated readers prefer v2 once published. Older binaries see the historical v1
snapshot and must not be used to query an updated archive. MCP mutation mirroring
is a follow-up; no vector server or approximate retrieval was added.

The regression gate uses 10,000 generated PNG additions and a deterministic pixel
embedding stand-in, comparing incremental and full segmented rebuild results.
This verifies index mechanics independently of model availability, with no model
execution or downloads. See the cost card below for synthetic scale measurements.

### Synthetic scale cost card

Reproduce with the standalone CPU-only example; prepare each archive before timing
its query in a fresh process:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo build -p saccade-core -p saccade --features embeddings --example embedding_index_cost
"$CARGO_TARGET_DIR/debug/examples/embedding_index_cost" build /tmp/index-100k 100000
/usr/bin/time -v "$CARGO_TARGET_DIR/debug/examples/embedding_index_cost" query /tmp/index-100k 100000
"$CARGO_TARGET_DIR/debug/examples/embedding_index_cost" build /tmp/index-400k 400000
/usr/bin/time -v "$CARGO_TARGET_DIR/debug/examples/embedding_index_cost" query /tmp/index-400k 400000
```

The example uses deterministic 384-dimensional vectors, asserts exact self
retrieval at top-10, and performs real manifest/metadata/vector hash validation.
The index mechanics test additionally compares top-100 against the flat exact
reference for five generated queries. Neither test establishes real-model
embedding accuracy. Timings include manifest load, hash verification and exact
ranking; source image decoding and model inference are excluded. OS cache state
is not controlled. Each recorded query is one run, with no repeat/noise claim.

Recorded 2026-10-07 on Linux x86_64, AMD Ryzen 9 7945HX (16 cores,
32 logical CPUs), 30.53 GiB visible RAM, Rust 1.98.1. Unoptimized Cargo dev
profile, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, one query thread,
`nice -n 19`, shared host load; no GPU or model inference.

| Rows | Live vectors (MiB) | Query (s) | Query peak RSS (KiB) | Build (s) | Build peak RSS (KiB) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 100,000 | 146.484 | 6.824 | 3,868 | 15.764 | 45,220 |
| 400,000 | 585.938 | 33.976 | 3,604 | 63.550 | 175,376 |

Both exact self-query assertions passed (exit 0). At 400k, the archive exceeds
both historical caps while query RSS remains below 4 MiB in this sample. Writer
metadata memory grows with rows; query does not retain total vectors. This is
synthetic mechanics/scale evidence, not real-model retrieval qualification,
interactive readiness, optimized-build latency or a repeat/noise study. The
one-million-row/16-GiB ceiling is a defensive bound, not a measured performance
claim for every permitted archive shape. Exact remains the reference; an
approximate replacement needs a declared latency target, an optimized exact
baseline and recall measurements. No approximate mode is justified by these
unoptimized timings alone.

Measurement executable SHA-256: `448c82d505a5faf2aacea736c3ec1b677daff1499ab6e80d7a974a3053440c46`.
Source SHA-256: `embedding_index.rs` = `5b905c5d4374200ad6ec761fdbd43587020dc152fb92cdd8bedc80fd537405c5`;
`embedding_index_cost.rs` = `bfca4438c3b3d87f3a44781e6f4a177b51e0ef19eb486849e3ef24ae11ab83de`.
