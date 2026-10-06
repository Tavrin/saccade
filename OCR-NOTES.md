# OCR lane decisions and evidence

Base: `dd1bad0`, branch `feat/ocr-paddle`, clean start; waves 8/9 merged.
One implementation agent, no push/integration or live provider calls.

## Read-only scout

- `crates/saccade-core/src/general/ocr.rs:1`: current RTen/ocrs engine, contract and pins.
- `crates/saccade/src/text_cmd.rs:39`: text/expect-text observation routing.
- `crates/saccade-core/src/media/mod.rs:600`: reused media OCR session.
- `crates/saccade-core/src/ui_review.rs:125`: accepted OCR source kinds.
- `crates/saccade-core/src/wave7/models.rs:345`: registry contract validation.
- `crates/saccade-core/src/assist/execution.rs:202`: wave 4 provider execution envelope.
- `crates/saccade-core/src/wave7/runtime.rs:25`: existing ONNX Runtime load pattern.

## Decisions

- Replace local OCR implementation with PP-OCRv5; retain source/text interface, exact Unicode and explicit download opt-in. Reject alphabet-only proof and Tesseract fallback. Reversal cost: adapter/registry change.
- Model provisioning uses artifact registry metadata to resolve immutable revisions, then downloads only commit-addressed artifacts and licence/config files. No general web browsing. This is necessary for the lane's explicit model-download requirement, overriding the earlier waves' development download ban.
- Use generated French/German/Spanish fixtures with font licence receipts; declare CER/WER and exact accent expectations before inference, with accent-stripping negative control. Generated contracts remain pending coordinator review.
- Mistral wire mapping and execution will use constructed fixtures only. Public-document shape supplied in the brief plus unverified protocol details remain labelled; no live API qualification or inferred price/revision claims.

## Processor and artifact decisions

- Official ONNX detector `PaddlePaddle/PP-OCRv5_mobile_det_onnx` revision `e6f4fa85f00e168c862bc462aebca69eef9b3d3d`: 4,826,518 bytes, SHA-256 `a431985659dc921974177a95adcfbb90fd9e51989a5e04d70d0b75f597b6e61d`.
- Official Latin recognizer `PaddlePaddle/latin_PP-OCRv5_mobile_rec_onnx` revision `89d3a50e2c27e2e7cceeab0e944c25c807d5db4f`: 8,042,023 bytes, SHA-256 `7888113072263cb471b93f66dd5e2ad70548dc526fa1ace760d0d973dd121498`.
- Recognition inference.yml supplies 836 ordered entries (duplicates are deliberate), plus CTC blank and appended space: 838 graph classes. Dictionary/config SHA-256 `0bbe984570f597af3638e50bdf2e8276f3ab26a61966096538b3b0d1849f5c84`, 6,817 bytes. Older upstream latin_dict.txt is incompatible and rejected.
- Model Apache-2.0 declarations and upstream LICENSE/source headers checked at immutable revisions; all receipts in `scripts/models/paddle-ocr-provenance.json`. Provisioning script rechecks bytes and SHA before cache writes. Runtime uses already verified ONNX Runtime 1.22 cache. No export/conversion environment required.
- Processor parameters are recorded in `docs/text.md`. Use standard quad DB scoring/unclip/minimum sides and perspective crops. The minimum rectangle of a round-offset rectangle has the same expanded side extents, so use that exact geometry rather than a general clipping dependency. Rust bilinear resizing/integer rectangle output is not asserted to be OpenCV parity. Reversal cost: replacing geometry/resize implementation requires rerunning frozen contracts.
- Added imageproc/serde_yaml and their small dependencies: source licence hashes retained in `scripts/models/ocr-dependency-licenses.json`; THIRD_PARTY updated. The YAML decoder reads only SHA-verified official configuration. Rejected a handwritten YAML parser as unnecessary ambiguity. No default Cargo feature gains model dependencies; local OCR remains opt-in `ocr`.
- DejaVu Sans and Liberation Sans installed fonts have permissive Bitstream/public-domain and SIL OFL licences respectively. Font hashes and full licence receipts accompany generated fixtures. No fonts/models vendored. The OCR spec's permissive-font instruction governs this fixture choice.

## Provider decisions and unresolved external facts

- Mistral is an optional `ocr-provider` feature and explicit `--ocr-provider mistral` selection. Images/PDFs are encoded inline, selected pages are explicit, Markdown/page structure is preserved, no boxes or confidence synthesized. MCP mirrors only request-bound fixture comparison under existing file-root checks.
- Live transport uses the wave 4 user-owned endpoint/root authorization, shared attempt ledger/pacing, fixed `~/.config/saccade/mistral.env` / `MISTRAL_API_KEY` binding and pre-dispatch money reservation. Unknown actual cost retains the full reservation; malformed or failed responses remain charged and have final attempt/money outcomes. No calls made by this lane.
- Because general browsing/live calls are excluded, the dated fixture model `mistral-ocr-2505`, inline PDF/image wire details, exact page selection behavior, response dimensions/usage/revision semantics and current per-page billing remain unconfirmed. Constructed fixture tests establish the mapper and policy behavior only. No aliases or built-in guessed pricing. Live CLI requires a user-confirmed conservative per-page ceiling and a price-policy revision; the transport remains behind the generic interface. Reversal cost: wire mapping/price policy changes require new fixtures and receipts.
- Historical `ocrs` observation kind and schema remain for persisted readers; the executable adapter, graph pins and Cargo dependencies are removed. External explicitly pinned Tesseract remains available; it is never a fallback from PP-OCRv5 failure.

## Evidence so far

- Frozen generated contracts: `scripts/models/ocr-accent-contracts.json` and `/mnt/linux-extra/moss-scratch/saccade-ocr/accents/contracts.json`. French/German/Spanish, DejaVu Sans/Liberation Sans, 32/40/48 px: 18 cases. Declared before inference: CER <= .02, WER <= .10, exact accented words required. **Generated, pending coordinator review.**
- First Rust CPU inference: 18/18 PASS, every CER=0 and WER=0; all 18 accent-removal controls rejected. Receipt: `/mnt/linux-extra/moss-scratch/saccade-ocr/accent-results.json`; run log: `accent-run.log`. The negative control is additionally being strengthened to replace accented letters with plain counterparts rather than deleting them.
- Core targeted general tests: 28 passed, four existing unrelated heavy tests ignored. Fixture Mistral request/response and mock transport tests pass, including egress denial, monetary exhaustion and retention of unknown cost.
- CLI fixture tests: two passed, local CLI/media inference test intentionally heavy-gated pending focused execution. Genericity: PASS (external denylist present). Docs/schema discriminators: PASS.
- First core check passed. First CLI check exposed a private run-token helper; replaced with a local random tempfile identity. Existing no-MCP `grounded_cmd::page` warning is avoided by checking the normal MCP-enabled feature set; no unrelated cleanup.


## Serif failure and processor correction

To interpret "several fonts" strictly, added DejaVu Serif as a third font before
running its nine new contracts; retained the same thresholds and all prior cases.
The resulting 27-case run had one real failure: French/Serif/48 returned `cæur`
for `cœur` (CER 1/39, WER 1/8), saved in `accent-results-before-linear.json`.
No threshold, fixture content, font, expectation or dictionary index was changed
to remove this failure.

Found a processor mismatch: image::Triangle resize antialiases downsampling,
whereas the pinned PaddleOCR processor uses cv2 INTER_LINEAR pixel-centre sampling.
Implemented bilinear centre sampling with edge replication and no antialiasing,
with an impulse downscale regression. This is a source-grounded processor repair,
not an accent substitution/lexicon trick. Its first run exposed a real detector
float output of 1.0000001 (one f32 ULP above 1), prematurely refused by the original
strict probability guard. Validation now allows four f32 epsilons of numeric
roundoff and clamps only reported confidence; a score of 1.01 and nonfinite values
remain invalid. Source/rounding parity is still not claimed.

Disk availability fell below 25 GB during external activity. No further build
was admitted. Removed only this lane's obsolete incremental cache and binaries/core
artifacts. Final corrected inference/tests are pending sufficient disk admission.

## Final disposition — resource blocked, acceptance not complete

- Last completed Rust corpus: **26/27 PASS**, one French DejaVu Serif/48 `œ→æ`
  failure; **27/27 accent-removal controls rejected**. The original two-font
  subset remains 18/18 with CER=WER=0. The failing case remains in the frozen
  corpus and its failed receipt is retained; it was not dropped or reclassified.
- The corrected linear sampler was compiled and attempted, but a detector value
  `1.0000001` stopped the run at the output-range guard. The subsequent numerical
  guard correction and its regression test are **not rebuilt/retested**, because
  the disk dropped persistently below the mandated 25 GB floor (about 14 GB at
  handoff). No build admission was bypassed.
- A CPU Python diagnosis of French Serif/48 with centre-sampled linear resizing
  recognized `cœur`. It uses an approximate horizontal detector box and is
  **diagnosis only, not Rust DB/rectification or accent acceptance**. Receipt:
  `/mnt/linux-extra/moss-scratch/saccade-ocr/linear-python-diagnosis.json`.
- Before the final sampler/numeric edits: minimal check PASS (four pre-existing
  feature-disabled warnings in local_cmd/grounded_cmd); OCR/provider/MCP clippy
  with `-D warnings` PASS; core general tests 28 PASS, four unrelated heavy
  ignores; CLI provider/MCP tests 3 PASS; focused real default OCR/expect-text,
  inspect-image and media inference 1 PASS. These receipts do not transfer to
  the last unbuilt processor edits. Final fmt, schema/doc check and genericity
  remain independently verifiable without builds.
- Evidence and prior/current source fingerprints, exact binary/runtime hashes
  and qualification boundaries are recorded in
  `/mnt/linux-extra/moss-scratch/saccade-ocr/validation-receipt.json`.
- Resume with sufficient disk: `SACCADE_OCR_PYTHON=/mnt/linux-extra/saccade-models/venv/bin/python scripts/gates-ocr.sh`.
  It runs minimal check, strict OCR/provider/MCP clippy, targeted core/CLI tests,
  generated fixtures before inference, focused CLI/media inference, the full
  27-case accent corpus with unchanged thresholds/negative controls, docs and
  genericity. No live APIs or GPU jobs. Models are already verified/cached.
- PP-OCRv5 integration and Mistral fixture/policy adapter are implemented. The
  **done-when accent acceptance is not established** for final source. Coordinator
  owns generated-contract review, live API/billing facts and integration.
- README, CHANGELOG and generated docs/cli.md were intentionally preserved per
  shared-file protocol. Coordinator should add the local engine/accent replacement
  and optional document provider, then regenerate CLI help/reference after integration.

Implementation commit: `9c6a63f`. Final owned target cleanup: `/mnt/linux-extra/moss-cargo-targets/codex-saccade-ocr` removed and verified absent. Model pins, generated fixtures and failed/diagnostic evidence retained. No push or integration. Final genericity PASS includes newly staged files.
