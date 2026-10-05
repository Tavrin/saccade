# Wave 6 decisions

- Scope: only Wave 6, branch feat/wave6. No subagents. Commit each item; integration belongs to the coordinator.
- Registration: implement a bounded pure Rust ORB-style FAST / oriented BRIEF detector, ratio-tested reciprocal matches and deterministic RANSAC. No new dependency or copied implementation. Reject a native OpenCV dependency. Reversal cost: replace detector behind the match interface and requalify synthetic fixtures.
- Registration outputs use a separate versioned contract for explicitly selected alignment; the existing default comparison contract remains intact. Geometric exclusions get an inclusion PNG and hatched heatmap. Reject silent black-border scoring. Reversal cost: additive integration into the normal report after exclusion-contract review.
- External facts: do not browse. Local registry has ort, but no ocrs, resvg/usvg, pdfium-render or c2pa sources. No model artifact URL/hash or calibration supplied. Those source/licence and pinned-artifact gaps are explicit blockers, never guessed facts. Continue deterministic items first.
- Verification: CARGO_TARGET_DIR=/mnt/linux-extra/moss-cargo-targets/codex-saccade-w6; check free disk before each compilation; only nice -n 19, -j 4 check/clippy and module-targeted core unit tests. Heavy integration gates are written, never run here.

- Hashing: aHash/dHash/pHash, original Rust arithmetic, no added dependencies. Rejected blockhash as redundant low-frequency coverage. Reversal cost: additive algorithm/version and index rebuild.
- Dedupe: BK-tree over unique hashes and transitive connected components, <=100000 inputs and 64 MiB path budget. Rejected decoded-pixel caching and all-pairs edges. Exact duplicates collapse; adversarial tree search remains a stated time limitation. Reversal cost: replace candidate search, retain hash contracts, requalify clustering.

- Embeddings: implement configurable DINOv2-small pooled ONNX interface, SHA-pinned runtime cache, explicit preprocessing, f64 cosine and streaming exact flat index. Reject inventing an export hash or universal cosine bands. No new package/default feature dependency. Supplied calibration is labelled unqualified; canonical export parity/calibration and optional CLIP are deferred. Reversal cost: provision/review a pinned export and corpus, then heavy gates; no index-format change needed for the same export.

- Resource decision: stop all compilation/tests after df reported 24 GB. Reject bypassing the disk gate or deleting shared targets. Reversal cost: coordinator restores >=25 GB and reruns light/heavy gates.

- Text: literal Unicode-scalar CER and whitespace WER, heuristic line/word position/content matching and expectations; preserve accents and boxes. Reject ASCII normalization, instructions from extracted text and confidence-as-probability. Reversal cost: add opt-in grapheme normalization or a global matching algorithm with a new recorded policy.
- OCR: reuse the existing repository's pinned Tesseract CLI adapter under the existing ocr feature; no new engine/model licence asserted. Bound adapter reads at 64 MiB. MCP imports observations only, since arbitrary executable/model paths introduce an execution authority not granted by root reads. ONNX/Rust OCR engine/model licence verification deferred because crate/model sources are absent. Reversal cost: verify a permissive engine/artifacts, implement behind the same source interface, run generated accent gates.

- Assessment: report content-dependent blur/noise/block/banding/clipping indicators and paired deltas; unknown verdict, no universal pass threshold. Learned score skipped absent reviewed permissive pin. Reversal cost: qualify a model behind a feature or add explicit content-scoped policies without changing raw indicators.
- Disk admission reopened at 29 GB; resume bounded light checks without heavy execution.

- Documents: defer concrete SVG/PDF adapters and all-command/page-summary routing; retain bounded DPI/page renderer interface and a real-rendering acceptance gate. Reject writing an incomplete SVG/PDF interpreter or inferring dependency licences from names. Reversal cost: provision/review permissive sources, add feature-gated adapters, connect input routes and page summaries, then pass generated document gates.

- Inspection: emit unsigned metadata, current JPEG-table compatibility, copy-move candidates, weak ELA, archive candidates and declared publication geometry. Always unknown authenticity/AI-generation status. Reject heuristic real/fake inference or trusting unvalidated JUMBF assertions. GPS is opt-in; opaque XMP/IPTC hashes do not expose their location text. Reversal cost: add licensed C2PA validation and signed/watermark evidence with explicit provenance.
- Deferred integrity portions: c2pa source/licence and dependency review unavailable; XMP/IPTC parsing, encoder signatures and double-compression/resampling detector qualification lack implementation/constructed evidence. These remain labelled unavailable, not substitute heuristics. Reversal cost: review/provision sources and implement focused indicators with generated truth and failure cases.

- Router: explicit questions select measured families, record the choice, and reject unsupported options without fallback. Existing same-render report identity remains intact via a SHA-bound additive component, removed on ordinary rerun. Reject auto-selecting models or interpreting FLIP thresholds as text/quality policies. Reversal cost: additive corpus-level routes and qualified policies, with existing evidence schemas preserved.
- Catalogue: describes compiled features and conditional/deferred availability for every requested family, including existing HDR-FLIP and ColorVideoVDP; does not claim execution qualification. Rejected treating absent integrations as available. Reversal cost: update conditional status after integration with actual commands/features.
- MCP native execution: read roots cannot authorize arbitrary dynamic libraries. Require operator-owned onnx-runtime.json path/SHA authority for all embedding calls, including question routing; load ONNX bytes from a freshly rehashed buffer to avoid file-base external-initializer reads. Reject arbitrary-library execution from a tool argument. Reversal cost: replace configuration with an equivalent explicit startup runtime authority, preserving the native execution boundary. Approved runtime remains operator-trusted and immutable for the session, not sandboxed.
- Final development checks: formatting, diff whitespace, gate shell syntax and wave-local docs/schema checks are executable without compilation. Latest disk admission was 18 GB; compilation/test verification remains deferred under the explicit 25 GB shared rule. No model downloads, browser/provider/GPU/release runs or heavy gates executed. Renderer acceptance remains intentionally failing until the deferred adapter is supplied.


## Wave 6b decisions (SPEC-wave6b.md, same feat/wave6 worktree)

- Sources: Cargo crates.io fetching is explicitly permitted by the follow-on.
  No browsing, weight download, provider/browser/GPU call or heavy test was run.
  All 244 newly resolved registry packages were reviewed from Cargo.toml and
  published licence/notice files; THIRD_PARTY.md records permitted choices and
  exact notice hashes. Published c2pa/ocrs/RTen/hayro packages omit some licence
  files; their source metadata/README declarations are recorded rather than
  fabricating files. Small roxmltree metadata parsing is unconditional; heavy
  document/credential/OCR engines remain opt-in. Rejected bundling native PDFium,
  native OpenSSL and default C2PA HTTP. Reversal: reselect adapters behind the
  existing interfaces and requalify input/credential semantics.
- 6.6: resvg/usvg 0.48.1 and pure-Rust hayro 0.3.0. Hayro 0.8.0's registry
  rust-version is 1.92, exceeding the repository baseline. Both engines render
  actual pages; source hashes and declared DPI bind page summaries. Every
  missing/error page remains a failure. Disable both SVG resource resolvers;
  reject text/images/active content rather than silently omit them. PDF missing
  fonts/interpreter warnings fail pages. Rejected host-dependent font discovery
  and claiming complete PDF support: hayro 0.3.0 source explicitly lacks some
  blending/isolation/encryption semantics and exposes no allocation/time budget.
  Remaining all-historical-command routing (e.g. direct image::open in quality,
  rank/temporal and other raster consumers), arbitrary SVG/font/resource support,
  wide PDF correctness and worker isolation are explicitly deferred. Reversal:
  operator-pinned fonts/resources or a newer MSRV-compatible renderer, shared
  page-aware inputs in each remaining family, plus independent coverage gates.
- 6.7a: c2pa 0.90.22 with default-features=false/rust_native_crypto; no HTTP
  backend, remote-manifest feature, OCSP fetch, thumbnails or native crypto.
  Explicit per-reader validation settings; project active signer/actions/
  ingredients and validation codes. Only exact trained-model declarations from
  validated signed credentials change generation from unknown. Rejected trusting
  raw JUMBF/XMP metadata or heuristic real/fake output. Add namespace-checked XMP
  capture/edit tags and selected IPTC IIM dates/byline/copyright, GPS opt-in.
  Raw decoded-DCT histogram gaps and derivative periodicity are bounded and
  explicitly unqualified. Conventional Annex K compatibility is a signature
  family shared by encoders, not an encoder identity. Full/extended/compressed
  metadata, exact encoder attribution and forensic discrimination qualification
  remain deferred: no frozen signature corpus/operating policy was supplied;
  the constructed history gate demonstrates sensitivity only. Reversal: add
  parsed formats or frozen truth/policy qualification without changing unknown
  authenticity authority. Signed JPEG + asset-binding transplant heavy gate
  requires operator-generated, licence-recorded pinned fixture.
- 6.4: ocrs 0.10.4 + matching RTen 0.21.0, behind ocr and the existing text
  source interface. saccade-ocrs.v1 pins both exports and requires explicit CTC
  alphabet and model licence evidence. Runtime-only explicit --download-model
  uses the existing bounded/hash-verified cache transport. Model bytes fed from
  freshly verified memory, no native library/program. Source evidence:
  ocrs-0.10.4/src/lib.rs DEFAULT_ALPHABET lacks accented characters;
  src/text_items.rs TextChar exposes only char and rect, no confidence;
  README points to download-models.sh, but the published crate includes neither
  that script nor model hashes/licence receipts. Actual accent-capable model
  selection, licence verification and recognition qualification are deferred;
  supplied licences are labelled declarations. Readability stays unknown/failed
  when confidence is absent. Rejected fabricated SHA/licence/confidence and
  changing expect-text's fail-closed contract. Reversal: reviewed accent model
  plus real heavy gate; a confidence-capable engine/API must be separately
  recorded before qualification of expected readability. Imports remain MCP's
  OCR path; arbitrary runtime execution/downloads are CLI-only.
- 6.3: exact NCHW tensor preparation, offline operator-source/checkpoint export,
  independent source-vector parity, frozen fit/holdout calibration and receipts.
  Sample-disjoint splits and unique image hashes/pairs; threshold fit only on
  fit, no retuning after holdout. Emit calibrated model only after parity and
  zero observed holdout errors. Export recipe binds source revision/checkpoint,
  software versions, graph hash and corpus/pairs. Rejected inventing a universal
  graph pin or cosine bands. Actual model/source/holdout evidence remains heavy
  and unrun; source labels/vectors are supplied evidence, not authenticated by
  the arithmetic. Reversal: replace operator source model/recipe, freeze a new
  corpus and requalify; existing flat index interface remains stable.
- Development corrected the handoff TIFF scalar/coordinate lifetime error.
  Shared CLI option bundle boxed after the DPI addition crossed clippy's enum
  size limit. These changes are necessary for this source to compile cleanly.
- Heavy gate now includes opt-in feature/MSRV checks, touched-crate full tests,
  document/multipage/nonblank pixel checks, offline credential and tamper checks,
  Rust OCR accents with honest absent confidence, forensic history observations,
  export parity/holdout and existing Wave 6 registration/hash/index/router/MCP.
  No JS/showcase/shared README/CHANGELOG/docs/cli.md edits. Coordinator owns
  integration and every heavy execution/qualification decision.

- Extra MSRV verification found a pre-existing default-build blocker: butteraugli
  0.4.0's multiversion-generated AVX-512 target features are unstable on Rust
  1.88 (E0658 in precompute.rs). This dependency already exists at the handoff;
  no compression change is authorized by wave6b. Full/default MSRV acceptance
  is explicitly deferred to the owning lane. Check the new Wave 6 optional
  adapters with defaults disabled; the gate is labelled msrv-wave6, not a claim
  of full default-build MSRV acceptance.

- Final feature clippy passed; the isolated Rust 1.88 Wave 6 check passed with
  defaults disabled. The default butteraugli MSRV blocker remains separate.
  Export jobs load the checkpoint from the freshly SHA-verified bytes and bind
  the exact initial manifest bytes, avoiding a second path read for provenance.

- Document item committed as 6836261; source coverage and heavy acceptance limits
  remain exactly as recorded above. Credential/metadata/forensic item follows.

- Credential/metadata/forensic item committed as 0e24e07; unknown authenticity,
  offline validation and unqualified history observations retain their limits.

- Rust OCR adapter committed as 6edaeaf; no canonical model or confidence is
  invented. The queued accent/readability gate remains a qualification boundary.
