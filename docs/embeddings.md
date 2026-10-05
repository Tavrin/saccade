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
provided. This implementation supplies the interface and execution path; built-in
model and calibration qualification remain deferred. CLIP/text-to-image retrieval
is deferred until a reviewed model licence and pinned export are available.
Model-dependent tests are ignored under `heavy: embeddings`; the coordinator
supplies `SACCADE_W6_EMBEDDING_MODEL`, `SACCADE_W6_MODEL_CACHE`, and
`SACCADE_W6_ORT_LIBRARY` before running them. No model tests ran in this lane.
