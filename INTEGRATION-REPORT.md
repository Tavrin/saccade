# Saccade integration round 2

Round 2 is executed. Wave 7 is merged and wired, opt-in pre-filter mask neutralization is
implemented, real pinned qualification inputs are produced, and the required gates ran
through shared admission. Wave 5 passes; Wave 6 fails only for the missing accent-capable
Rust OCR contract; Wave 7 passes with explicit research deferrals. The local Linux CI
matrix, package verification, release script, hosted-mapping regression and SAM 2.1 export pass.

Worktree `/home/etienne/dev/saccade-wt/integ`, branch `integ/waves-4-6`; clean starting
HEAD `c52416d6755fba4e0981f24c29b59d285c678b7a`. Merge `63e91c6` preserves wave 7
`5753506302b4bb971c557bbbb3a54943cd70e9df`. Corrective tested HEAD `8a6a169c3c415d922e8eb1cd61d9adf7e588e7b9`;
the final report commit changes documentation only. No push, main/other-worktree changes,
subagents, live provider calls, or `qualify-wave4.sh`.

[Previous round 1 report](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/round1-report.md) is retained as historical evidence;
the current results below supersede its mask and absent-asset failures.

## Decisions and implementation

- `MaskMode::Exclude` remains the core/config/historical-reader default. `Neutralize`
  replaces masked test RGBA samples with reference samples before spatial filtering, then
  excludes masks from scoring. HDR uses the same seam for RGB samples. Browser matcher,
  sweep and generated dynamic-content UI exclusions select neutralization; every compare
  report records the mode. Raw excluded-error and no-mask audit metrics retain the original
  comparison. Wrong-size and entirely excluded masks remain errors. The new regression
  checks masked alpha/color changes and preserves a real adjacent unmasked change. All
  four original browser tests pass without altered expectations. Rejected mask growth and
  looser thresholds; reversal is a small mode/config change before integration upstream.
- Hosted Claude/GPT mappings attach to assist's immutable catalog and exact encoded PNGs;
  the focused regression exercises both mappings and rejects changed bytes. Preview and
  recorded observations are advisory, with no live transport or fabricated cost. `check-ui`
  attaches image/phrase-bound locate evidence separately from deterministic measurements;
  root-bound execution requires a contained replay. Faces, declared face crops and named
  watermark inspection attach to `assess`/`inspect-image`. TrustMark inference alone is
  never represented as successful ECC decoding or source parity.
- One supplied registry, `cache/r2/registry.json`, combines wave 7 model pins with typed
  wave 6 embedding/Tesseract contracts. Legacy per-model files remain checked projections
  for the original heavy-test interfaces. Embedding, text and review tools also accept the
  shared registry. Distributed builtin pins contain no host-specific contracts. Model
  inference never downloads implicitly. The capability router records availability and
  partial/fixture-only authority. Registry/model/projection hashes are in
  [asset pins](scripts/models/integration-r2-asset-pins.json).
- Canonical schemas sort object keys across feature combinations while preserving array
  order and strict equality. Generated CLI docs and bounded Codex/Claude packs match the
  all-features binary. README is byte-identical to the starting checkout. Public contract
  docs use portable descriptions; machine-local receipts stay in this report/cache.

## Real assets, provenance and limits

Cache means `/mnt/linux-extra/saccade-models`; no global Python/system package install.
The isolated CPU tooling installation and exact scripts/logs are retained. PyTorch CPU
wheels came from the official version-specific CPU index, an explicit interpretation of
this round's CPU-wheel authorization; remaining export tools use pinned PyPI versions.
No GPU execution ran. Weights downloads use frozen official revisions and checked hashes.
Licences were checked in fetched files and recorded in `THIRD_PARTY_NOTICES.md`.

| Input | Produced evidence | Boundaries |
| --- | --- | --- |
| DINOv2-small | Official Facebook HF revision `ed25f3a31f01632728cabb09d1542f84ab7b0056`; checkpoint `ae1e99fcefd534ed978cdeb8326f08030c96e28b7a81ffcbc98a857c84d14be1`; Apache-2.0 model card/LFS hash verified | Genuine transformer checkpoint, not random weights |
| DINOv2 ONNX | `scripts/models/prepare-r2.py`; 56×56 RGB/ImageNet normalization, pooled 384-vector, opset 17; graph `2fd91c205910154583815783892aa74f5414b88505ecda8e3cab8cf7f033822b` | Cache-only local export; no remote distribution endpoint is asserted |
| Frozen corpus | Eight generated colored rectangle/ellipse images; disjoint fit/holdout; SHA `f58f12e4db3de8d89923d8ccea500e61cdf7c77fb8feed71fed28af7f2019983` | Scope is generated geometry only; no natural-image/domain semantic guarantee |
| Embedding qualification | [Actual receipt](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-batch3/embedding-qualification.json): source-vector parity maximum component error <7e-7; four holdout pairs, zero observed false positives/negatives | Source execution is recorded by export script, not independently attested; finite holdout only |
| C2PA | Actual `c2pa` signing with a local self-signed test identity; signed, stripped and transplanted JPEGs; [pins](scripts/models/c2pa-r2-pins.json) | Cryptographic integrity passes; local certificate is not a trusted real identity or depicted-truth proof; no TSA/OCSP/remote fetch |
| Tesseract accents | Version-pinned Ubuntu 5.3.4 binary/libs and French traineddata extracted under cache; hash-pinned wrapper/version/language; original CAFÉ fixture recognized | Apache-2.0 Tesseract/French data, BSD-2-Clause Leptonica; generated contract and expectations are **generated, pending coordinator review** |
| Rust OCR | Generated accent image/expected-text contract retained | No reviewed accent-capable RTen exports; default `ocrs` alphabet is ASCII. No relabeling, invented pins or fake recognizer |
| SAM 2.1 tiny | Official checkpoint/tagged source/exporter pins; self-contained encoder `d8427d60c388fd7bc1fbc211f9cc0b837e7a44879e2a7a6c0ac2c39554d13304`, decoder `70063a399155d9496f383c8b33005cb81f1e863e63e9acfd68684adfab8876d7` | Repeated CPU export matches [committed pins](scripts/models/sam2-r2-export-pins.json); ONNX checker passes; no native SAM2 adapter or source-parity qualification |

Signed C2PA fixture SHA is `55f8f54f02a96c004a9b1245d93cb4e0d1f52b4d786b9620c0ac1f224bae958b`.
Private signing key stays mode 0600 in the fixture cache; it is neither committed nor
included in retained evidence. The original malformed-action fixture is separately
preserved under batch 1; the valid repinned fixture names its algorithmic creation action.

## Gate receipts

All receipts contain command, source HEAD, empty source-diff SHA, elapsed time, exit and
log path. [Latest receipt index](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/latest-receipts.json) contains exact commands.
The required full batch was attempted three times through `moss-heavy.sh 8`; later
refused/failing gates were rerun in one admitted corrective batch. No exit 75 disk refusal
is counted as execution or acceptance. Shared disk/RAM controls were never changed.

Wave 5/Wave 6/MSRV receipts below use `0f82c75`. Production Rust is byte-identical to that
source in final `8a6a169`; later changes are feature-aware test assertions, gate tooling and
SAM export pins. Corrective CI, release and native smoke receipts bind `8a6a169` explicitly.

| Gate | Latest result / exit | Tested HEAD | Evidence |
| --- | --- | --- | --- |
| msrv-default | PASS / 0 | `0f82c75` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-batch3/msrv-default.log) |
| msrv-compression-reference | PASS / 0 | `0f82c75` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-batch3/msrv-compression-reference.log) |
| wave5 | PASS / 0 | `0f82c75` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-batch3/wave5.log) |
| wave6 | FAIL: Rust OCR contract absent / 1 | `0f82c75` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-batch3/wave6.log) |
| wave7 | PASS; research deferrals / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/wave7.log) |
| ci-fmt | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-fmt.log) |
| ci-clippy-default | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-clippy-default.log) |
| ci-tests | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-tests.log) |
| ci-build-core-minimal | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-build-core-minimal.log) |
| ci-clippy-core-minimal | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-clippy-core-minimal.log) |
| ci-test-core-minimal | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-test-core-minimal.log) |
| ci-build-cli-default | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-build-cli-default.log) |
| ci-clippy-cli-default | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-clippy-cli-default.log) |
| ci-test-cli-default | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-test-cli-default.log) |
| ci-build-cli-all | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-build-cli-all.log) |
| ci-clippy-cli-all | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-clippy-cli-all.log) |
| ci-test-cli-all | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-test-cli-all.log) |
| integration-regressions | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/integration-regressions.log) |
| genericity | SKIP: private denylist absent / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/genericity.log) |
| ci-packages | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-packages.log) |
| ci-package-verify | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/ci-package-verify.log) |
| release-check | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/release-check.log) |
| sam2-export | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/sam2-export.log) |
| integration-vision-mapping | PASS / 0 | `8a6a169` | [log](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/integration-vision-mapping.log) |

Wave 5: every subgate passes, including AVIF, product heavy tests, all four unchanged
browser tests and CLI fixtures. Private genericity policy itself is unavailable; its
constructed regression passes.

Wave 6: fmt, minimal check, optional-feature MSRV, strict Clippy, core/CLI tests,
registration, embeddings, export parity/holdout, C2PA integrity/tamper negatives,
forensic history, export help, hashing and docs pass. CLI-heavy is 11 pass / 1 fail:
`rust_ocr_recognizes_accents_without_inventing_confidence` requires
`SACCADE_W6_RUST_OCR_CONTRACT`. Tesseract CAFÉ and pinned embedding index tests pass.

Wave 7: fmt, strict Clippy, complete default/minimal suites, model pins/fixtures,
provisioned runtime CLI, native test build, six bounded CPU smokes, rectangle coordinate
mapping and schemas pass. [Native receipts](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/rust-smokes.json) bind retained test
binary, source and runtime hashes. Source parity, explicit local-VLM server and selected
native adapters are DEFERRED; a zero script exit does not close those qualifications.

Release check: complete local script exits 0; all showcases reproduce their expected
results and all 40 JSON documents validate. Generated docs have no drift. No publication.

## Failures fixed forward and retained attempts

- Preparation: dav1d path prerequisite, local variable/initializer integration diagnostics,
  constant-size HDR chunk Clippy, and OCR test constructors. Original development/prep
  logs are retained in the [corrective evidence directory](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective).
- Batch 1 (`894041c`): command count, collapsed HDR condition and helpers after test modules;
  C2PA auto-created action malformed. Fixed by `dde067a` / `21fc955`; validation and
  every original assertion remain. The C2PA signer now declares an explicit created
  action/source type. Strict Clippy uses explicit builder context, not deprecated globals.
- Batch 2 (`21fc955`): maintained-public-doc local path. Fixed by `0f82c75`, moving host
  location information here. No documentation test was weakened.
- Batch 3 (`0f82c75`): two optional wave 7 operations absent from command enumeration test.
  Fixed by `3dbf466`, retaining exact counts and adding feature-presence/absence assertions.
  `8a6a169` records the successful SAM export while retaining actual adapter deferrals.
- Batches 1/2, and batch 3 SAM, hit shared disk admission. Refusals are retained, then
  actual commands reran after space recovered. This lane's incremental files were removed
  under Cargo locks; future builds disable incremental compilation. Before the last
  admitted batch, `cargo clean -p saccade -p saccade-core` reclaimed only lane artifacts.
  No private target, another lane's cache deletion, threshold change or timeout relaxation.

[Batch 1](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-batch1/receipts.json),
[batch 2](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-batch2/receipts.json),
[batch 3](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-batch3/receipts.json), and
[corrective batch](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/receipts.json) retain all outcomes.

## Remaining acceptance and cleanup

The only executed required gate still FAIL is Wave 6 Rust OCR: supply pinned,
licence-verified accent-capable RTen exports and the reviewed contract, then rerun the
original heavy test. Coordinator review of generated OCR contracts remains before release.
Wave 7 reviewed source-parity bundle/local VLM, SAM2 native adapter, LPIPS/DISTS weight
grants, MUSIQ checkpoint and TrustMark ECC/resize parity remain deferred. Hosted fixture
mapping and finite generated evidence do not establish live model accuracy or human acceptance.

The local Linux CI matrix does not establish macOS/Windows, clean installation, tagged
release archive/fork-PR workflows, manual browser/relocation/Moss consumer or live pilot
qualification. The inherited Butteraugli workspace patch still does not propagate into
published archives: default-feature packaged Rust 1.88 needs the upstream/separately
published compatible dependency; stable archive verification passes. No live wave 4
provider qualification or external private genericity policy is asserted.

Final [binary pins](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/binary-pins.json) and
[identity verification](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/identity-verification.json) retain debug/release/native
binary hashes and verify assets against committed pins. Browser output, showcase reports,
C2PA fixtures/certificate, model manifests, parity/smoke/export receipts and failure logs
are outside the build target. Model cache remains for reproducibility.

After every admitted job finished, the exact prescribed target
`/mnt/linux-extra/moss-cargo-targets/codex-saccade-integ` was removed with exclusive
nonblocking debug/release Cargo locks held and canonical/nonsymlink guards.
[Cleanup receipt](/mnt/linux-extra/saccade-integ-evidence/2026-10-05-r2-corrective/target-cleanup.json) records removal and
39.1 GiB free afterward. No push or Git changes outside this worktree.

# Round 3 — Butteraugli 0.9.3, MSRV 1.89 (2026-10-05)

Starting HEAD `54a4bba`, branch `integ/waves-4-6`. The owner-approved round-3
spec supersedes the Rust 1.88 compatibility decision above. Current source,
consumer requirements, CI MSRV and gate scripts now use Rust 1.89; historical
wave/round-2 evidence and design decisions retain their original toolchain facts.
No push, other-worktree Git changes, subagents or GPU execution.

The registry resolver selected latest 0.9.x **0.9.3** and the workspace pins
`=0.9.3`. The fetched package declares `rust-version = "1.89"` and
`license = "BSD-3-Clause"`; its README agrees. Its archive omits LICENSE,
so the retained JPEG XL Project Authors notice keeps its recorded older upstream
source revision. [Fetched dependency pins](/mnt/linux-extra/moss-scratch/saccade-integ-r3/dependency-pins.json)
bind manifest hashes and new SIMD dependency licences (MIT OR Apache-2.0).
The vendor directory and crates.io patch are removed. Img/RGB8 and the function
signature remain compatible; call sites use the 0.9 builder to explicitly retain
80 cd/m2. Report and imgtune provenance now identify 0.9.3. The constructed
corpus source hash uses Cargo.lock rather than removed vendored paths.

## Reference qualification

The paired CPU-only probe uses the retained, compiler-gated 0.4.0 source from
starting HEAD and published 0.9.3. Same opaque RGB samples, 80 cd/m2, no default
dependency features. Both are compared against unchanged, hash-checked official
libjxl v0.12.0 fixtures; reference tolerance remains **0.02**, SSIMULACRA2 0.05.
The new version is closer for every nonidentical fixture. Retaining 0.4.0 was
rejected; no tolerance or golden score was changed. The fixture scope is synthetic
brightness/block/local-patch distortion, not broad natural-image qualification.

| Pair | Official | 0.4.0 | 0.9.3 (Rust 1.89) | Old signed error | New signed error |
| --- | ---: | ---: | ---: | ---: | ---: |
| reference.png | 0.0000000000 | 0.0000000000 | 0.0000000000 | +0.0000000000 | +0.0000000000 |
| brightness.png | 5.4179773331 | 5.4184951782 | 5.4179792404 | +0.0005178451 | +0.0000019073 |
| blocks.png | 2.1045861244 | 2.1048207283 | 2.1045837402 | +0.0002346039 | -0.0000023842 |
| patch.png | 54.2458076477 | 54.2357063293 | 54.2460327148 | -0.0101013184 | +0.0002250671 |

[Probe log](/mnt/linux-extra/moss-scratch/saccade-integ-r3/probe-1.89.log),
[scores](/mnt/linux-extra/moss-scratch/saccade-integ-r3/scores-1.89.json),
[milestones](/mnt/linux-extra/moss-scratch/saccade-integ-r3/MILESTONES.md).

## Round 3 gate receipts

Pending current-source qualification and CI/release gates. Each admitted component
has a 900-second limit; release-check's opt-in admission releases shared resources
between components. Builds stop below the inherited 25 GiB free-space floor.
