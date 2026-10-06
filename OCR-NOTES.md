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

## Coordinator review and frozen expansion — 2026-10-06

Read `OCR-CONTRACT-REVIEW-2026-10-06.md`: approved with four additions.
The contract file now records **generated, coordinator-reviewed (2026-10-06)**.
Added uppercase French, rarer lowercase French, ligatures and French numeric
formats across DejaVu Serif, DejaVu Sans and Liberation Sans at 32/40/48 px.
The original 27 image hashes, expected text, thresholds and font receipts are
unchanged. Every new case uses CER ≤ .02, WER ≤ .10 and exact required Unicode
strings, frozen before any resumed inference. The numeric group includes the
review's exact quoted phrase plus nine separate U+202F narrow no-break-space
variants, because the quoted phrase has ASCII spaces but the review explicitly
requests thin spaces. Rejected replacing the quoted expectation.

Controls now derive from contract truth, rather than already erroneous OCR,
so an inference failure cannot falsely prove that a no-op control is effective.
NFD accent stripping plus explicit œ/Œ/æ/Æ/ß expansion covers uppercase and
rarer lowercase letters as well as ligatures. Controls and exact strings are
stored in the frozen contract. The unchanged accent-control bar requires both
missing exact strings and CER above the same threshold.

Numeric text has no accented letters: pure accent stripping is identically the
truth and cannot honestly be a rejected negative control. Its control stays
**FAIL** (changed=false), independently of numeric OCR accuracy. Also froze a
separate format-stripping control (comma→dot, euro→EUR, en dash→hyphen,
U+202F→ASCII space). This additional control cannot clear the ineffective accent
control. Rejected adding an accented word to the coordinator's numeric phrase,
calling symbol replacement accent stripping, or letting OCR errors manufacture
control rejection. Reversal cost: a coordinator-approved separate accented
numeric fixture/control would require a new pre-inference freeze and run;
existing expectations and failures must remain.

Gate hardening: generated contracts must byte-match the checked-in frozen file
before inference; generation/freeze failures prevent stale-fixture runs.
Inference errors are retained per case and processing continues. Builds remain
serial at -j 4, with admission paused below 25 GB; the resumed run also monitors
free space and suspends only its own process group below that floor.

## Resumed final disposition — 2026-10-06

This supersedes the earlier resource-blocked acceptance state. Full
`scripts/gates-ocr.sh` ran on the resumed source and exited **1** solely for the
accent gate. Final linear resize and numeric-roundoff regressions pass; core
29 PASS / four unrelated heavy ignores, CLI fixtures three PASS, real default
OCR/expect-text/inspect-image/media inference one PASS. Fmt, minimal check,
strict OCR/provider/MCP clippy, generation, frozen-contract byte match,
docs/schema and genericity PASS. No live API calls, GPU work or integration.

All 72 cases are recorded in `docs/ocr-contract-results-2026-10-06.md`:
**46 OCR PASS / 26 FAIL**. French 8/9, German 9/9, Spanish 9/9; uppercase 9/9,
rarer lowercase 2/9, ligatures 9/9, numeric formats 0/18. Original Serif/48
`cœur→cæur` persists with the repaired processor. Rarer lowercase fails on
curly apostrophe substitutions; numeric formats fail on en dashes and, in
thin-space variants, U+202F loss. These are documented known limitations in
`docs/text.md` and CHANGELOG; no failing case was removed or reclassified.
Accent/ligature controls reject 54/54; numeric accent controls are 18/18 no-op
FAIL, extra format controls reject 18/18. The gate stays FAIL.

Rejected another processor/lexicon change or a favorable repeat to force
acceptance: the requested revalidation is complete and known failures remain.
Contract SHA-256 is `99ca5ecfe32e9f884aed74aade131fbcf5ae0a8c206a98b592fe1cc6548b75e9`;
its pre-inference frozen copy still matches exactly. Exact source/runtime/binary
identities, complete JSON results, resource pauses and gate exit receipt are in
`/mnt/linux-extra/moss-scratch/saccade-ocr/validation-resume-2026-10-06.json`
and its referenced evidence files. Generated fixtures establish neither general
accuracy nor source/export/OpenCV parity. Coordinator review is complete;
integration and live Mistral wire/billing facts remain coordinator-owned.

## Coordinator post-run scoring disposition — after dbfea24

The post-hoc contract-design correction supersedes the numeric no-op FAIL
classification above: 18 accent-stripping controls without accents are N/A.
Strict scoring and all expected texts, thresholds and fixture identities stay
unchanged. A second declared typographic-equivalence view folds ’/‘ to ASCII
apostrophe, U+202F/U+00A0/U+2009 to space, and en/em dash to hyphen, on both
observations and expectations (including required strings). Omitted characters
remain errors; no trimming or whitespace collapsing is added.
Applicable negative controls must remain rejected in both views.

Known limitations remain: œ misread in serif at large sizes (DejaVu Serif/48
cœur→cæur), dash omission with some sans fonts (Liberation Sans), and omitted
spaces before € in some thin-space fixtures. See the updated
`docs/ocr-contract-results-2026-10-06.md` for the new run and both per-case scores.

The requested gate rerun exited 1 solely for accents: strict **46/72**, folded
**61/72**. Every strict rate, observation, required-string result and verdict
matches the previous run. Folded failures comprise the serif ligature case,
six Liberation Sans dash-omission cases and four DejaVu missing-space cases.
All other gate stages passed; 54/54 applicable accent controls and 18/18 format
controls were rejected in both views, with 18 accent controls N/A.
