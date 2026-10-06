# OCR lane progress

Base `dd1bad0`, branch `feat/ocr-paddle`; implementation commit `9c6a63f` (parent of the final handoff commit).

- Local PP-OCRv5: implemented with official immutable ONNX/dictionary pins, runtime reuse and default CLI/media routing; final sampler/numeric changes await resource-admitted gates.
- Accent qualification: BLOCKED, last completed corpus 26/27 pass (18/18 original two-font subset), 27/27 stripping controls rejected; Serif/48 œ→æ failure retained. Corrected processor not accepted until Rust rerun.
- Mistral provider: implemented, optional/off by default, fixtures and mock spend/egress/fixed-key controls passed; no live calls/qualification. MCP fixtures honor file roots.
- Docs/schema/genericity: updated; no-build checks pass. Reproducible narrow gates in scripts/gates-ocr.sh.
- Resource: stop builds below 25 GB; final build target removed; model cache and evidence retained outside repository. No push/integration.
