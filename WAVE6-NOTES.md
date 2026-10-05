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
