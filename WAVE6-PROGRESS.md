# Wave 6 progress

6.1 registration: done, 8ded843; focused core tests 4 PASS, heavy rotation/perspective + CLI/schema fixture gates written, not run. Explicit route rejects unsupported evidence options.
6.2 hashing/dedupe: done, c47e67a; focused tests 3 PASS, 100k scale + CLI heavy gates written and not run.
6.3 embeddings: partial, baf3a66; configurable pinned DINOv2 ONNX inference and streaming flat index written; feature check PASS. Canonical export pin/parity and calibrated corpus absent, CLIP deferred. One clippy iterator correction written after disk fell below 25 GB; subsequent light checks and model gates NOT RUN.
6.4 OCR/text: partial, c811481; image-bound imports, line/word changes, Unicode CER/WER and expectations written; reuse existing pinned Tesseract adapter behind ocr. ONNX/Rust OCR engine/model licence verification and real Latin-accent qualification deferred: ocrs/model sources absent. Tests written, NOT RUN due disk admission.
6.5 assessment: done (implementation commit recorded next); deterministic measures/deltas, CLI/MCP/docs and generated focused tests written. Learned score skipped: no reviewed permissive pinned model. Light checks pending.
6.6 documents: deferred (decision commit recorded next); bounded renderer interface and generated SVG/PDF acceptance gate written. resvg/usvg/pdfium-render or eligible pure Rust PDF sources absent; licence/native adapter and all-command/page-summary routing cannot be verified/implemented from permitted facts. Gate intentionally remains failing until adapter is supplied.
6.7a integrity: deferred pending preceding items; c2pa licence source absent locally.
6.7 capabilities/router: deferred pending preceding items; no blocker yet.

Build admission closed: df -BG /mnt/linux-extra reported 24 GB free; all subsequent compilation/tests suspended until >=25 GB. Development continues; heavy gate script NOT RUN.

Disk admission reopened: df reported 29 GB; resume only admitted light checks. Heavy gate script remains NOT RUN.

Admission closed again at 21 GB after clippy compilation. Clippy found a heavy-test JSON reader type error and iterator/API style issues; corrections written, recheck pending. Focused embedding/text/assessment tests remain NOT RUN. No heavy tests executed.
