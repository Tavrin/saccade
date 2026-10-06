# OCR lane progress

Base `dd1bad0`, branch `feat/ocr-paddle`; implementation `9c6a63f`, resumed from `00880c4`.

- Local PP-OCRv5: implemented with immutable official graph/dictionary pins and default CLI/media/inspection routing. Final resize/numeric regressions PASS; real Rust CLI/media inference PASS. No source/export parity claim.
- Contracts: coordinator-reviewed (2026-10-06); 72 cases frozen before inference with original 27 image/text/threshold/font identities preserved. Four required groups across three fonts and three sizes, plus nine numeric thin-space variants.
- Accent qualification: FAIL, strict 46/72, typographic-equivalence 61/72. French 8/9 (Serif/48 cœur→cæur persists), German 9/9, Spanish 9/9, uppercase 9/9, rarer lowercase 2/9, ligatures 9/9, numbers 0/18. Folded scores: French 8/9, German/Spanish/uppercase/rarer lowercase/ligatures each 9/9, numbers 8/18. Strict failures retained; expectations and thresholds unchanged. See docs/ocr-contract-results-2026-10-06.md.
- Controls: 54/54 applicable pure accent/ligature controls rejected; 18 numeric no-ops are N/A under the coordinator post-run disposition. Additional numeric format controls rejected 18/18. Truth-derived controls do not borrow OCR errors.
- Validation: full scripts/gates-ocr.sh ran, exit 1 solely for accents. Fmt/minimal/strict clippy/core (29 pass, four unrelated ignores)/CLI fixtures (three pass)/generation/frozen-contract match/local inference (one pass)/docs/genericity PASS.
- Mistral provider: optional/off by default, fixtures and policy tests PASS; no live calls/qualification. Integration and live API/billing verification remain coordinator-owned.
- Resource: serial -j 4 builds, paused below 25 GB and resumed after recovery. Owned target removed after validation; model cache and identity/result/log evidence retained outside repository. No push/integration.
