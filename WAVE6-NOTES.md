# Wave 6 decisions

- Scope: only Wave 6, branch feat/wave6. No subagents. Commit each item; integration belongs to the coordinator.
- Registration: implement a bounded pure Rust ORB-style FAST / oriented BRIEF detector, ratio-tested reciprocal matches and deterministic RANSAC. No new dependency or copied implementation. Reject a native OpenCV dependency. Reversal cost: replace detector behind the match interface and requalify synthetic fixtures.
- Registration outputs use a separate versioned contract for explicitly selected alignment; the existing default comparison contract remains intact. Geometric exclusions get an inclusion PNG and hatched heatmap. Reject silent black-border scoring. Reversal cost: additive integration into the normal report after exclusion-contract review.
- External facts: do not browse. Local registry has ort, but no ocrs, resvg/usvg, pdfium-render or c2pa sources. No model artifact URL/hash or calibration supplied. Those source/licence and pinned-artifact gaps are explicit blockers, never guessed facts. Continue deterministic items first.
- Verification: CARGO_TARGET_DIR=/mnt/linux-extra/moss-cargo-targets/codex-saccade-w6; check free disk before each compilation; only nice -n 19, -j 4 check/clippy and module-targeted core unit tests. Heavy integration gates are written, never run here.
