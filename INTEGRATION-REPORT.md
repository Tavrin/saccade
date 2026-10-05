# Waves 4–6 integration

Scope: `/home/etienne/dev/saccade-wt/integ`, branch `integ/waves-4-6`.
Base `cccd6e35d52f1472d7b51297cd20d75ea8127fe6`. Only this worktree's Git state
is owned; no push or changes to main/other worktrees. No subagents or live providers.

## Integration decisions

- Merge commits preserve Wave 4 (`2579457`), Wave 5 (`f8afd9e`) and Wave 6
  (`ae86d87`) in that order. Documentation/notices preserve all additions;
  registrations and feature manifests combine all waves. Lock resolution starts
  with Wave 6's pins and adds Wave 5's dependencies offline. Temporary markers
  are replaced by ordinary ordering and behavioral documentation.
- Discoverability lists assist and product commands, availability, prerequisites
  and authority. Assess/inspect-image reports include related commands. The
  generated agent guide is condensed to retain the existing 4800-byte pack limit;
  detailed contracts remain linked. README is preserved through --skip-readme.
- Matcher/sweep registration is explicit. It retains geometric exclusions and
  refuses ordinary config/masks, which the separate registration contract cannot
  honor. Matcher stability compares raw captures. Raw existing defaults remain.
- Butteraugli 0.4.0 is vendored with unchanged arithmetic. Its 12 multiversion
  attributes retain AVX2/SSE on Rust 1.88 and add AVX-512 for Rust >=1.89. The
  build script uses the actual compiler, refusing unrecognized versions safely.
  Rejected raising MSRV or removing newer fast paths. Reversal: remove workspace
  patch after a compatible upstream release. Cargo registry archives do not
  inherit patches: packaged Rust 1.88 needs the upstream fix or separately
  published compatible dependency; this branch does not publish either.
- dav1d development files (Ubuntu 1.4.1-1build1) are extracted under
  `/mnt/linux-extra/saccade-models/toolchain/dav1d`; system runtime 1.4.1 is reused.
  No system package installation. Browser/tool prerequisites use standard caches.
- Live qualification, provider/model conformance and actual unavailable model or
  credential fixtures are not fabricated. Failing prerequisite gates remain FAIL.

## Execution identity and limits

Initial coordinated batch began at `ff50243`; bounded fixes were committed as failures were diagnosed. The complete release check and final corrective rerun use source `c9da934f0aae01c45fe511db70ffc5541d6c0bc8`. The final report commit changes documentation only. Every rerun receipt records its exact source HEAD. No failing baseline was discarded.

Linux x86_64, stable Rust 1.98.1 and MSRV Rust 1.88.0. Cargo jobs=4, dev/test debug symbols=0, compiler wrappers empty. All heavy execution uses `moss-heavy.sh 8`; no GPU execution. Target is the prescribed `codex-saccade-integ` directory. dav1d uses the extracted development prefix; `SYSTEM_DEPS_DAV1D_BUILD_INTERNAL=never`. Playwright uses `@playwright/test@1.58.2` and its Chromium 1208; actionlint is v1.7.7. Model downloads were not substituted for absent reviewed contracts.

The 25 GB disk floor was enforced with admission checks and cache recovery. This target’s incremental caches, completed-release intermediates and obsolete workspace executables/core libraries predating the final source revision were removed under the corresponding Cargo locks; debug third-party dependency caches and current executables were retained until final target cleanup. The obsolete-file removal manifest is preserved outside the target. Final target cleanup and preserved evidence are recorded below.

Raw evidence: [final minimal/default clippy](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-final-matrix/receipts.json), [initial receipts](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/receipts.json), [corrective receipts](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-rerun/receipts.json), [environment](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/environment.json), [release binary identity](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/release-identity.json). Release showcases and initial browser attachments are preserved in the same evidence directory.

## Gate receipts

Final status selects the last actual execution of each exact command. `genericity` exit 0 is a documented SKIP, not private-denylist acceptance. Original failures and their names follow this table.

| Gate | Exact command | Initial exit | Final exit/status | Final log |
| --- | --- | ---: | --- | --- |
| msrv-default | `cargo +1.88 check --offline --locked -p saccade` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-rerun/msrv-default.log) |
| msrv-compression-reference | `cargo +1.88 test --offline --locked -p saccade --test quality_reference` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-rerun/msrv-compression-reference.log) |
| wave4 | `bash scripts/gates-wave4.sh` | 1 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-rerun/wave4.log) |
| wave5 | `bash scripts/gates-wave5.sh` | 1 | 1 FAIL | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-rerun/wave5.log) |
| wave6 | `bash scripts/gates-wave6.sh` | 1 | 1 FAIL | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/wave6.log) |
| ci-fmt | `cargo fmt --all --check` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/ci-fmt.log) |
| ci-clippy-default | `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/ci-clippy-default.log) |
| ci-tests | `cargo test --workspace --locked --no-fail-fast` | 101 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/ci-tests.log) |
| ci-build-core-minimal | `cargo build --locked -p saccade-core --no-default-features` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/ci-build-core-minimal.log) |
| ci-clippy-core-minimal | `cargo clippy --locked -p saccade-core --all-targets --no-default-features -- -D warnings` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-final-matrix/ci-clippy-core-minimal.log) |
| ci-test-core-minimal | `cargo test --locked -p saccade-core --no-default-features` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/ci-test-core-minimal.log) |
| ci-build-cli-default | `cargo build --locked -p saccade` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/ci-build-cli-default.log) |
| ci-clippy-cli-default | `cargo clippy --locked -p saccade --all-targets -- -D warnings` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-final-matrix/ci-clippy-cli-default.log) |
| ci-test-cli-default | `cargo test --locked -p saccade` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/ci-test-cli-default.log) |
| ci-build-cli-all | `cargo build --locked -p saccade --all-features` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/ci-build-cli-all.log) |
| ci-clippy-cli-all | `cargo clippy --locked -p saccade --all-targets --all-features -- -D warnings` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/ci-clippy-cli-all.log) |
| ci-test-cli-all | `cargo test --locked -p saccade --all-features` | 101 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/ci-test-cli-all.log) |
| integration-regressions | `python3 scripts/test-integration.py /mnt/linux-extra/moss-cargo-targets/codex-saccade-integ/debug/saccade` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/integration-regressions.log) |
| genericity | `bash scripts/check-genericity.sh` | 0 | 0 SKIP (external denylist absent) | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/genericity.log) |
| ci-packages | `python3 scripts/check-packages.py` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/ci-packages.log) |
| ci-package-verify | `cargo package --workspace --all-features --locked` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/ci-package-verify.log) |
| release-check | `bash scripts/release-check.sh` | 0 | 0 PASS | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/release-check.log) |

Every batch aggregates any nonzero constituent gate as exit 1. Individual command exit codes above are authoritative. The complete release check at the final source also reruns minimal/default builds/tests and normalized packaged-archive verification (with --allow-dirty); the standalone rows retain their exact initial argument-order command receipts.

### Wave subgates

Admission incident: the first corrective batch refused nine remaining commands, including Wave 6, with exit 75 at its stricter 25 GiB floor while cache recovery completed. Those commands were not executed; their raw refusal logs are retained in the corrective receipts. After recovering owned cache space, the remaining commands were resubmitted through the heavy wrapper. [Resubmission receipts](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-admitted-rest/receipts.json).

| Script | Subgate | Final status |
| --- | --- | --- |
| wave4 | fmt | PASS |
| wave4 | check | PASS |
| wave4 | clippy | PASS |
| wave4 | core-tests | PASS |
| wave4 | cli-tests | PASS |
| wave4 | wave4-heavy-core | PASS |
| wave4 | wave4-heavy-cli | PASS |
| wave4 | wave4-batch-routing | PASS |
| wave4 | qualification-preflight | PASS |
| wave4 | constructed-python | PASS |
| wave4 | docs | PASS |
| wave4 | shell | PASS |
| wave5 | fmt | PASS |
| wave5 | genericity | SKIP (script reports PASS for absent denylist) |
| wave5 | genericity-tests | PASS |
| wave5 | docs | PASS |
| wave5 | js-unit | PASS |
| wave5 | core-tests | PASS |
| wave5 | cli-tests | PASS |
| wave5 | clippy | PASS |
| wave5 | avif-prerequisite | PASS |
| wave5 | avif-check | PASS |
| wave5 | avif-clippy | PASS |
| wave5 | heavy-imgtune | PASS |
| wave5 | heavy-design | PASS |
| wave5 | heavy-history | PASS |
| wave5 | heavy-notifier | PASS |
| wave5 | heavy-sweep | PASS |
| wave5 | cli-binary | PASS |
| wave5 | browser | FAIL |
| wave5 | fixture-cli | PASS |
| wave6 | fmt | PASS |
| wave6 | check-minimal | PASS |
| wave6 | msrv-wave6 | PASS |
| wave6 | clippy | PASS |
| wave6 | core-tests | PASS |
| wave6 | cli-tests | PASS |
| wave6 | registration-heavy | PASS |
| wave6 | embeddings-heavy | FAIL |
| wave6 | embedding-qualification | FAIL |
| wave6 | credentials-heavy | FAIL |
| wave6 | forensic-history | PASS |
| wave6 | export-script-help | PASS |
| wave6 | hashing-scale | PASS |
| wave6 | wave6-cli-heavy | FAIL |
| wave6 | docs | PASS |

## Failures fixed forward

- `50f96b8` (Wave 4): `help_lists_active_commands_and_watch_alias_stays_hidden` now asserts all retained additive commands and the true integrated count. `wave4_report_batch_plan_binds_the_original_pair_and_report` removes the retained encoded capture by its actual digest and checks the smaller reference set before exercising rejection; production binding/reproduction checks remain unchanged.
- `9ffb89c`, `1406d2c`, `dde6322` (Wave 5): gate the assist-only test credential field, remove a needless borrow, and expose disjoint MCP product-operation schemas. Fixed `local_tools_and_preview_never_authorize_network_and_images_are_explicit` by implementing its existing oneOf contract.
- `8561567` (Wave 5): fixture server serves HTML with its correct Content-Type; dedicated MCP input/output siblings satisfy containment. This repairs `design driver records selector viewport and CSS values` and the original `scripts/test-wave5-cli.py` IndexError. No timeout, assertion or network authorization was relaxed.
- `661262f`, `2f226a3`, `1de2f3b` (Wave 6/workspace): sort generated JSON object keys canonically under serde_json/preserve_order, retaining full schema equality. Correct additive tool enumeration and assert every registered operation. Original failures: `committed_schemas_match_the_rust_types`, `local_tools_list_exactly_the_operations_this_binary_implements`, `six_tools_share_local_evidence_requests_proposals_and_human_escalation` (renamed without dropping its original checks).
- `c9da934` (all-features): preserve historical canonical object ordering for label hashes and judge plan identities. Fixed `bench_ordering_and_calibration_maths_count_missing_coverage` against frozen labels without changing those hashes. Added a regression that preserves array order while sorting nested object keys.

Other original failures were strict-clippy compiler diagnostics for the assist-only field and needless borrow; their commands and original logs remain in the receipts. No expectations or qualification thresholds were weakened.

## Remaining FAIL gates

### Wave 5 browser mask semantics

`bash scripts/gates-wave5.sh` exits 1 at subgate `browser`. Failing test: `generated page and locator use perceptual assertions and masks`, `integrations/playwright/test/matcher.spec.cjs:15`. It changes a 64×64 color swatch, masks exactly that rectangle, and requests threshold 0. FLIP spatial filtering runs before score exclusion, so differences remain immediately outside the mask. Initial retained report: masked fraction 0.05333333333333334, remaining mean 0.0012801999783578336 and maximum 0.25360971689224243; unmasked full-frame mean 0.03157899. The CLI produces valid measurement evidence and a failure verdict.

Current core semantics are visible in `crates/saccade-core/src/compare.rs` (compare, then masked_metrics). Pre-filter neutralization, automatic mask growth or changing exact-zero expectations requires a mask-contract/design decision. This integration does not make that decision, enlarge the mask, loosen the threshold or rewrite the test to pass. The corrected HTML fixture and other browser tests remain exercised.

### Wave 6 pinned qualification assets

`bash scripts/gates-wave6.sh` exits 1 on absent pinned inputs, not successful skips. These exact tests fail:

| Subgate | Failing test | Required input |
| --- | --- | --- |
| embeddings-heavy | `general::embedding::tests::supplied_pinned_model_runs_on_generated_images` | `SACCADE_W6_EMBEDDING_MODEL`, `SACCADE_W6_MODEL_CACHE`, `SACCADE_W6_ORT_LIBRARY` |
| embedding-qualification | `general::embedding_qualification::tests::frozen_export_parity_and_disjoint_holdout_qualify` | Frozen `SACCADE_W6_EMBEDDING_CORPUS` and SHA256, plus pinned model/cache/runtime |
| credentials-heavy | `general::credentials::tests::pinned_signed_asset_validates_and_tampering_is_rejected` | Generated signed fixture `SACCADE_W6_C2PA_ASSET` and `SACCADE_W6_C2PA_SHA256` |
| wave6-cli-heavy | `pinned_ocr_reads_generated_accent_glyphs` | Reviewed `SACCADE_W6_OCR_CONTRACT` |
| wave6-cli-heavy | `rust_ocr_recognizes_accents_without_inventing_confidence` | Reviewed `SACCADE_W6_RUST_OCR_CONTRACT` |
| wave6-cli-heavy | `supplied_embedding_index_build_and_query_preserve_pins` | Pinned model/cache/runtime |

No replacement corpus, fixture pin, runtime contract or success receipt was invented. Registration, hashing scale, forensics, generated document/MCP rendering and the remaining CLI-heavy cases are independently represented by their actual subgate/test results. Ignored model tests in ordinary CI do not qualify these failing heavy gates.

## MSRV and generated outputs

Both default Rust 1.88 compilation and the official nonidentical compression reference test (`compression_metrics_match_official_nonidentical_references`) pass. Wave 6’s optional-feature MSRV check is recorded independently. [Butteraugli provenance](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-batch1/butteraugli-provenance.json) verifies that removing the conditional multiversion attributes restores all 13 original source files and that compiler probing disables AVX-512 on 1.88 and retains it on stable. The workspace patch does not propagate into published Cargo archives: packaged default-feature Rust 1.88 support remains an upstream/separately-published dependency follow-up. Stable packaged-archive verification passes; no package was published.

`docs/cli.md`, schema links in `docs/contracts.md`, and Codex/Claude agent packs were generated with the all-features binary. The release check validates them without drift. CHANGELOG has one factual Unreleased section. README is unchanged from base (SHA256 `5356218f39cd14549094276123beea5f0992e0e0b3c445ccb6709950742b4149`).

## Acceptance boundaries and cleanup

External genericity denylist is absent: `scripts/check-genericity.sh` was run and reports SKIP with exit 0. Its generated temporary-repository regression passes in Wave 5. This does not claim the unavailable private-policy check passed.

No `scripts/qualify-wave4.sh`, live provider, live Figma API or webhook call ran. Wave 4 local constructed/preflight tests and source receipt do not establish provider accuracy, human acceptance or spend-capped qualification. Linux checks do not establish macOS/Windows or clean-machine installation. Release-script CI-only/archive/fork-PR and manual browser/relocation/Moss/pilot gates remain explicitly unrun.

The final-source [Wave 4 receipt](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-rerun/gates-wave4-receipt.json) is preserved and matches source hash `sha256:00ab0d44163ab0c7809b28bb4cf15697ec9241559b06063aae83aa88a67cf0ea`. README equality, branch and ordered merge ancestry are verified.

After every batch and cache monitor finished, the exact prescribed target was deleted with both debug/release Cargo locks held. [Cleanup receipt](/mnt/linux-extra/saccade-integ-evidence/target-cleanup.json) confirms removal and 45.3 GB free at that point. Logs, receipts, binary identity and showcase/browser artifacts remain outside the deleted target.

The report is the only final documentation change; source code remains the tested `c9da934` tree. No push or Git changes outside this worktree.

## Round 2 work in progress

Authority: SPEC-integrate-r2.md, starting clean at c52416d; merge 63e91c6 preserves
feat/wave7 at 5753506. This worktree alone owns Git state. No push, main/other-worktree
mutations, subagents or live providers. The prescribed target and heavy wrapper remain.

Decision: pre-filter neutralization is opt-in core behavior; browser/sweep/generated
UI dynamic masks select it. Raw excluded error is separately measured so the no-mask
audit remains truthful. Rejected automatic mask growth and looser assertions. Reversal
is a small mode/config change while unmerged. Historical core defaults/numbers remain.

Vision integration is explicit and attributed: hosted adapters prepare/decode fixtures
inside assist, grounding is advisory, and wave 6 image reports attach face/crop/watermark
contracts. The registry is shared, with typed legacy contract projections retained.
Default/minimal builds gain no native model dependency. TrustMark ECC/source parity and
unimplemented selected learned models remain unavailable; neural inference is not decoding.

Official immutable DINOv2-small revision ed25f3a31f01632728cabb09d1542f84ab7b0056
is Apache-2.0 by its official model card. A reproducible isolated CPU export and an
8-image generated corpus with disjoint fit/holdout are produced under cache/r2. Scope
is procedural geometry, not natural-image semantic qualification. CPU wheels use the
official immutable PyTorch CPU wheel index; remaining export tooling uses pinned PyPI
versions. No global install or GPU. Local exports have no remote distribution URL;
cache bytes and official checkpoint provenance are separately pinned.

Tesseract/French assets were extracted from version-pinned Ubuntu packages and locally
hashed, without system installation. The exact original accent fixture reads CAFÉ.
Contracts are generated, pending coordinator review. Rust OCR's default alphabet is
ASCII; no accent-capable RTen exports are supplied, relabelled or fabricated. This
remains a prerequisite gap. A local test-only C2PA certificate is generated; its signed
fixture and tampered variants will be produced by the crate in the admitted preparation.

Development all-feature cargo check passes after reusing the pinned dav1d tool prefix.
Initial dav1d-path and strict-Clippy constant-chunk diagnostics are retained under
/mnt/linux-extra/saccade-models/r2; forward fixes preserve every original assertion.
Heavy qualification and release receipts are pending the batch below.
