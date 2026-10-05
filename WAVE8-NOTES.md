# Wave 8 decisions and read-only scout

Starting branch `feat/wave8`, HEAD `3bd1bf5`; clean worktree. `git log -n 15`
and the complete integration report were read before edits. Waves 4–7 are integrated;
inherited model accuracy/parity and Rust OCR qualification remain explicitly incomplete.

## Scout map (before implementation)
- `general/input.rs:9`: bounded encoded reads and SDR decode.
- `general/integrity.rs:257`, `metadata.rs:6`: EXIF/XMP/IPTC/ICC container extraction.
- `general/assessment.rs:46`, `hashing.rs:42`: deterministic quality and hashes.
- `general/registration.rs:447`: FAST/oriented BRIEF + RANSAC, reference-to-target transform.
- `general/embedding.rs:178`: optional lazy ONNX image embeddings.
- `wave7/faces.rs:82,198`: detector and crop checks; `runtime.rs:27`: pinned ONNX sessions.
- `wave7/models.rs:313`: official pinned registry; `runtime_install.rs`: runtime provisioning.
- `wave7/observation.rs:244`: generic description observation boundary.
- `main.rs:612,1628`: existing loopback viewer; `mcp.rs:1076,2270`: additive operations.
- `wave7/providers.rs:156`: GPT Responses fixture mapping (no live transport).

## Resource and scope decisions
- One agent, one Cargo command at a time, nice 19, jobs 4, owned wave8 target,
  incremental/debug info disabled. No model/provider/browser/GPU work during development.
- The shared development rule forbids release builds, so local maturin release build is
  wired into the heavy gate; a compiled abi3 package import and light pytest are the local goal.
- Do not claim learned saliency or text-image calibration from deterministic fixtures.
- Records are built from one retained encoded buffer. Optional unavailability is explicit;
  description is off by default and requires an injected observation provider.
- Rust structs own library contracts; CLI, MCP, Python and HTTP delegate to them.
  Reversal cost: additive APIs/schemas only; no legacy contract migration.

- Disk crossed below floor after the first targeted test completed: 22 GB available. Cargo paused; continuing source/tests/docs. First media test receipt: 4 passed, 110 filtered out, no models, no downloads.

## 8.3 text-image search disposition
- Implemented shared exact index add/build/load/save and image/text/vector query boundaries,
  interoperable with wave 6 vectors.bin and index schema v1. Ties sort by row order;
  bands always uncalibrated until heavy qualification. Generated vector tests verify ranking
  and corruption rejection, not learned text semantics.
- Deferred SigLIP 2 inference: supplied registry has no joint export; existing embedding
  contract only admits DINOv2. No official revision or weights licence supplied, and the
  shared rule prohibits web-content research/model downloads in development. This is an
  evidence availability gap, not a claim that no permissive checkpoint exists.
- Rejected guessing a hash/revision/licence and using image-only vectors for text queries.
  Reversal cost: add a verified joint model/tokenizer contract and engine adapter; retain
  index files, query transport and explicit uncalibrated bands.

## 8.4 focal point
- Use deterministic 64x64 colour-surround contrast with a weak centre/edge prior.
  The specification gives spectral residual as an example; this simpler pure Rust
  method satisfies the deterministic first requirement and handles colour-only contrast.
  Generated objects at three known positions assert a measurable focal displacement;
  flat/transparent fixtures assert the explicit centre fallback.
- Learned saliency skipped until an official permissive pin is supplied. Reversal cost:
  replace/extend a versioned algorithm ID; preserve focal/map/crop transport contracts.

## 8.5 video
- Reuse onset's deterministic penalized segmentation recurrence with RGB histogram SSE
  costs (not timing MAD/qualification). Reject invalid/empty/oversized inputs and excessive
  unique shots. Dedup uses hashes only as candidates, then histogram verification, so
  differently coloured flat frames are not collapsed by equal perceptual hashes.
- External ffmpeg + ffprobe only; no new native/GPL dependency, executable redistribution
  or linkage. Missing decoder is typed; supplied frame directories work offline.
- Decode <=600 sec / <=300 frames / <=640x640 with 120 sec subprocess deadlines; normalize
  timestamps to the explicit fps grid. Reject raising timeouts. Reversal cost: version a
  different sampling/shot algorithm; retain keyframe/record schemas and frame manifests.

## 8.6 usage matching
- Reuse wave 6 FAST/oriented-BRIEF matcher/RANSAC for both source pixels and saved records.
  No source image reopen is required for a fingerprint record. Reports retain transform,
  source-space crop bounds, consensus confidence and every failed target.
- Reject a hard global-hash cutoff: crops and larger page captures can have dissimilar
  global hashes even with good registration. Hashes provide candidate evidence; keypoints
  decide. Reversal cost: tune a future separately versioned prefilter without changing
  the stored compact fingerprint or matched-transform direction.

## 8.7 local API
- Implement a bounded HTTP/1.1 subset over std sockets so header/body reads have a
  terminating 10-second total deadline. Request Content-Length/JSON required; no chunked
  bodies. Reject duplicate framing/auth headers. The core handles all endpoint operations.
- Loopback is default; nonloopback bind is startup-only explicit configuration (needed for
  container port forwarding). Root containment applies to source and index files. Token
  only from the startup env file, never request configuration or ambient provider keys.
- Docker contains binary only, unprivileged user, mounted model/runtime volume, no publishing.
  Reversal cost: replace transport with an HTTP framework without changing core records,
  endpoint version or startup authority.

## 8.8 endpoint adapters
- Add env-file configured OpenAI-compatible Responses/Chat Completions mappings and
  classic Azure deployment Chat Completions URL + api-key auth header. Azure Responses
  is not guessed from the classic deployment path. All mappings/decoders are fixture-only;
  existing human-authorized transport remains owner-controlled.
- Allow configured model IDs while retaining exact requested/returned model identity,
  closed output schema, refusal/truncation and original-pixel geometry checks. Fixture
  tests assert actual Azure path, header name, image request mapping, usage, raw response
  digest and secret absence/redacted errors. Reversal cost: add another explicit wire
  style; unchanged observation schema and authority boundary.

## Development qualification decisions
- Cargo's targeted Python integration test builds both cdylib/rlib (the latter only
  permits the test target to depend on the binding). It imports a temporary ordinary
  package backed by that abi3 library and runs the three light pytest fixtures. This is
  the permitted targeted-test route; no standalone cargo build/maturin release run.
  Release/manylinux wheel archives remain gated. Reversal cost: remove rlib/test feature
  if the build policy later permits direct development wheels; Python API unchanged.
- Expanded core fixture suite: 13 pass, ffmpeg ignored. Optional-feature CLI/Python
  cargo check passes; strict default CLI/core/Python Clippy with model features passes.
- CLI error fixture initially assumed historical saccade-error.v1, but current errors
  use saccade-result.v2 with errors[].code. Corrected the fixture to assert the existing
  envelope and the same stable strict code; writer behavior was preserved.
- Source reads use safe Unix nonblocking OpenOptions flags plus regular-file checks;
  special-file input cannot hold a compute thread indefinitely. No unsafe code.
- Section timings include decode/hash, metadata headers and requested face inference;
  video timing includes decode/selection. Directory identity covers all sampled frame
  hashes plus the declared sample rate. Keyframes are not decoded/model-analyzed twice.
- GPU preset is named but explicitly unavailable with the inherited pinned CPU runtime;
  no unprovisioned GPU provider or silent CPU substitution is claimed. A GPU runtime/
  execution-provider qualification is deferred; enabling it is an additive adapter change.

- Consumer-facing model cache defaults follow XDG/HOME, matching existing provisioning; lane gate downloads explicitly use /mnt/linux-extra/saccade-models. No host-specific path is embedded in the ordinary package API default.

## Closing gate and identity handoff
- Heavy script covers fmt/docs, feature Clippy, touched full/minimal suites, installed
  runtime/models, ffmpeg, Python release/models, both manylinux architectures and Docker
  build/smoke. Each gate has a <=900-second bound and an explicit failure receipt. Joint
  text/image model gate deliberately remains failing until reviewed pins and licence
  evidence replace the deferral. No heavy gate was executed in this lane.
- The abi3 fixture uses Cargo's rebuilt dependency library under debug/deps, avoiding a
  possibly stale top-level library. Native package, CLI/test hashes and source receipts
  are retained outside the disposable target. No wheel qualification is inferred.
- Existing upstream OCR initialisation panic containment is retained when moving it into
  the reusable Engine. CPU model sessions are compile-checked only; inherited Rust OCR
  accuracy/parity remains unqualified.
- Owner must regenerate docs/cli.md and generated schema/agent packs, and add README and
  CHANGELOG entries. No release/publishing or integration acceptance is implied.

## Final development receipts
- Final implementation/gate commit: b1f753d. Strict Clippy on all touched crates/targets
  with model features and Python integration-test feature: PASS (4.70s final rerun).
- Core media: 15 PASS / 1 ffmpeg ignored (0.68s); CLI/MCP generated schema fixtures:
  3 PASS (0.65s); final compiled abi3 import/light pytest: 3 PASS (0.06s).
- fmt, diff whitespace, shellcheck and docs/schema/stub/artifact checks PASS.
- Retained source/binary hashes, tested ordinary package and command logs in
  /mnt/linux-extra/moss-scratch/saccade-wave8/FINAL-RECEIPT.json.
- Deleted only the exact lane target after commands exited and lane/Cargo locks were
  acquired. Free disk 51.85 -> 53.93 GiB. No models, other targets or shared caches deleted.
