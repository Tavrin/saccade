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

**Implementation complete; full qualification BLOCKED by shared disk space.**
The paired dependency probe is positive evidence, but existing Saccade reference
tests and the full build matrix did not execute. No broad acceptance is claimed.
Each admitted component has a 900-second limit; release-check's opt-in admission
releases shared resources between components. Builds stop below the inherited
25 GiB free-space floor. A 900.016-second recovery wait held no queue resources;
free space was only 14.609 GiB at expiry. The installed safe reaper could not
recover its goal while other build targets were live/protected. Neither its
safety checks nor the lane's floor were bypassed; other targets were untouched.

| Gate | Result / exit | Evidence |
| --- | --- | --- |
| Paired 0.4.0/0.9.3 probe, Rust 1.89 | PASS / 0; all nonidentical errors improved | [log](/mnt/linux-extra/moss-scratch/saccade-integ-r3/probe-1.89.log), [binary/source pins](/mnt/linux-extra/moss-scratch/saccade-integ-r3/probe-identity.json) |
| Official PNG fixture hashes / unchanged tolerances | PASS | Committed values.json and [milestone](/mnt/linux-extra/moss-scratch/saccade-integ-r3/MILESTONES.md) |
| fmt / actionlint / shellcheck / diff | PASS / 0 | [receipt](/mnt/linux-extra/moss-scratch/saccade-integ-r3/static-gate-receipt.json) |
| Constructed corpus source hash after vendor removal | PASS | [milestone](/mnt/linux-extra/moss-scratch/saccade-integ-r3/MILESTONES.md) |
| Generated notices / release tar and zip notices | PASS / 0; full 0.9.3 notice verified | [archive test](/mnt/linux-extra/moss-scratch/saccade-integ-r3/release-notices.log), [retained generated notices](/mnt/linux-extra/moss-scratch/saccade-integ-r3/THIRD_PARTY_NOTICES.md) |
| Rust 1.89 default check / existing quality_reference | REFUSED / 75; not executed | [receipts](/mnt/linux-extra/moss-scratch/saccade-integ-r3/msrv-refused/receipts.json) |
| Stable existing quality_reference / paired probe | Existing test REFUSED / 75; stable probe NOT RUN | [receipts](/mnt/linux-extra/moss-scratch/saccade-integ-r3/ci-attempt/receipts.json) |
| Default clippy / workspace tests | REFUSED / 75; not executed | [receipts](/mnt/linux-extra/moss-scratch/saccade-integ-r3/ci-attempt/receipts.json) |
| Core minimal / CLI default / CLI all features: build, clippy, tests | REFUSED / 75; not executed | [receipts](/mnt/linux-extra/moss-scratch/saccade-integ-r3/ci-attempt/receipts.json) |
| Package inventory / all-feature package verification | REFUSED / 75; not executed | [receipts](/mnt/linux-extra/moss-scratch/saccade-integ-r3/ci-attempt/receipts.json) |
| release-check.sh | REFUSED / 75; not executed | [receipt](/mnt/linux-extra/moss-scratch/saccade-integ-r3/ci-attempt/release-check.log) |
| Prescribed target cleanup | PASS | [receipt](/mnt/linux-extra/moss-scratch/saccade-integ-r3/target-cleanup.json) |

Production migration commit is `f5b7256`; later changes are documentation only.
The CI attempt binds clean `f5b7256`; refusal receipts are scheduling evidence,
not executed-test proof. Rust 1.89 is installed; stable is Rust 1.98.1. All owned
jobs finished before target cleanup. The paired binary, source, old-source hashes,
registry versions, fetched manifests, generated notices and failure logs remain
outside the target; target absence was verified (15.348 GiB free afterward).

Remaining acceptance: re-run the existing reference tests on Rust 1.89 and stable,
then the full local CI matrix and release checker once space permits. Neither the
probe nor future Linux gates establish macOS/Windows or release deployment.
Use the runner outside an outer admission wrapper so each component releases
admission; release-check gets its own per-component mode:

```sh
PATH="/mnt/linux-extra/saccade-models/toolchain/bin:$PATH" RUSTUP_TOOLCHAIN=stable MOSS_HEAVY_GPU=0 \
python3 scripts/gates-integration.py --admit-gb 4 \
  --evidence /mnt/linux-extra/moss-scratch/saccade-integ-r3/retry \
  --only msrv-default msrv-compression-reference stable-compression-reference \
  ci-fmt ci-clippy-default ci-tests \
  ci-build-core-minimal ci-clippy-core-minimal ci-test-core-minimal \
  ci-build-cli-default ci-clippy-cli-default ci-test-cli-default \
  ci-build-cli-all ci-clippy-cli-all ci-test-cli-all \
  ci-packages ci-package-verify release-check
```

Delete the same prescribed target after the retry completes. No tolerance relaxation,
fixture regeneration, source fork or upstream PR is needed. Reverting the migration
restores the old vendor/lock/MSRV contract without regenerating reference evidence.

# Round 3b — disk-floor retry (2026-10-05)

Retried exactly the gate list above from clean `efb8545` (production source unchanged
from `f5b7256`), using the recorded command with evidence destination
`/mnt/linux-extra/moss-scratch/saccade-integ-r3b/retry`. Initial `df -BG
/mnt/linux-extra` showed 35G available. Each component used the installed
`/mnt/linux-extra/moss-coord/bin/moss-heavy.sh`, CPU-only admission, the prescribed
`CARGO_TARGET_DIR=/mnt/linux-extra/moss-cargo-targets/codex-saccade-integ`, and
`CARGO_BUILD_JOBS=4` (Cargo's `-j 4` equivalent). Only one Cargo command ran at a
time; incremental compilation and dev/test debug information remained disabled.

| Gate | Result / exit |
| --- | --- |
| Rust 1.89 default check | PASS / 0 |
| Rust 1.89 existing quality_reference | PASS / 0 |
| Stable existing quality_reference | PASS / 0 |
| CI formatting | PASS / 0 |
| Default workspace clippy, warnings denied | PASS / 0 |
| Workspace tests, no-fail-fast | PASS / 0 |
| Core-minimal build | DISK-INTERRUPTED / 143; no completed build result |
| Core-minimal clippy / tests | DISK-REFUSED / 75 each; not executed |
| CLI default build / clippy / tests | DISK-REFUSED / 75 each; not executed |
| CLI all-features build / clippy / tests | DISK-REFUSED / 75 each; not executed |
| Package inventory / all-feature package verification | DISK-REFUSED / 75 each; not executed |
| release-check.sh | DISK-REFUSED / 75; not executed |
| Prescribed target cleanup | PASS; absence verified |

[Per-gate receipts and log paths](/mnt/linux-extra/moss-scratch/saccade-integ-r3b/retry/receipts.json)
bind all results to the clean starting revision. Both existing reference tests
passed without fixture or tolerance changes. The separate stable paired probe was
not in the recorded retry list and was not run. No source failure requiring a
forward fix was observed; no test was weakened.

Shared admission delayed workspace tests and the core-minimal build. During the
latter, free space crossed the inherited 25 GiB floor: an external monitor paused
the runner and terminated its component at **24.667 GiB**. The component exited
143; its partial compilation is not a pass. Recovery was bounded to three minutes
(the final check occurred after 220 seconds) and remained below the floor. At
24.625 GiB, the runner resumed solely to collect the remaining pre-execution
refusals, then exited 1. No admission settings, thresholds, timeouts, or other
lanes' artifacts were changed.

[Milestones](/mnt/linux-extra/moss-scratch/saccade-integ-r3b/MILESTONES.md),
[disk pause](/mnt/linux-extra/moss-scratch/saccade-integ-r3b/disk-pause.json),
[recovery receipt](/mnt/linux-extra/moss-scratch/saccade-integ-r3b/recovery-end.json),
[source identity](/mnt/linux-extra/moss-scratch/saccade-integ-r3b/source-identity.json),
and [reference binary hashes](/mnt/linux-extra/moss-scratch/saccade-integ-r3b/reference-binary-pins.json)
remain outside the target. After all lane jobs exited, canonical/nonsymlink guards
and an exclusive nonblocking Cargo profile lock protected target deletion.
[Cleanup](/mnt/linux-extra/moss-scratch/saccade-integ-r3b/target-cleanup.json)
records 25.475 GiB before removal and 29.037 GiB afterward; this later recovery
does not turn refused gates into executed results.

**Reference qualification, default clippy and workspace tests now pass; the full
local CI matrix and release checker remain incomplete due to disk pressure.**
Retry the interrupted/refused rows when space permits. Linux results do not
establish macOS/Windows, release deployment, or the previously deferred live/model
acceptance. Repository author configuration was used; no push.

## Wave 8 integration — 2026-10-06

Executed in `/home/etienne/dev/saccade-wt/wave8`, branch `feat/wave8`, from
`fe2ca95`. Merge `555a4ec` retains both wave 8 and `origin/main` `51f5bb0`,
including the dav1d CI dependency fix. No push, subagents, live provider calls,
or edits to another worktree. README is identical to merged `origin/main`.

### Implemented and fixed forward

- 8.3 uses official `google/siglip2-base-patch16-224` immutable revision
  `75de2d55ec2d0b4efc50b3e9ad70dba96a7b2fa2`. The pinned upstream README
  explicitly declares Apache-2.0; checkpoint SHA-256 is
  `612923381c76ec5a9bed335d1c48827e3f2e506ac31b044b63b2031fadee6a0b`,
  tokenizer SHA-256 is
  `cb9140fae3ac5122c972d37adf83e1248471a38147ad76f8215c8872c6fd8322`.
  Downloads were restricted to that revision under `/mnt/linux-extra/saccade-models`.
  [Pinned licence evidence](https://huggingface.co/google/siglip2-base-patch16-224/blob/75de2d55ec2d0b4efc50b3e9ad70dba96a7b2fa2/README.md).
- `scripts/models/export-siglip2.py` pins the CPU export environment and checks
  checkpoint/ONNX parity. Image graph SHA-256
  `50319b38e8350a79e720ca3e11aee99020538bbd5c5d429a30a7f53f9a9134db`,
  float16 text graph SHA-256
  `331a15b3bf3c4bad090ff2a8d6dccaaf07b98a0406eafd102febb929c2e184f3`.
  Text weights use float16 with float32 output to respect the existing 1 GiB
  artifact limit. Fixed normalized error bound remains 0.02: measured image
  maximum 1.19e-7 and text maximum 9.66e-5. Dynamic-int8 failed at 0.06032
  and was rejected without relaxing the gate. Older upstream revision `a7d042...`
  was rejected because its automatic card did not explicitly name the licence.
  These are cache-only local exports, not official hosted ONNX releases.
- Lazy CPU inference binds both graph hashes, tokenizer, preprocessing, padding
  and context length into the index identity. CLI `index query --text` uses the
  pinned joint model; image-only models fail explicitly. Historical DINOv2
  serialization/identity is preserved. Persisted indices round-trip in Rust
  and Python, retrieve two generated red/blue squares correctly, and reject a
  changed tokenizer identity. Scores remain uncalibrated with no acceptance verdict.
- Fixed gate failures without weakening assertions: CLI command inventory now
  requires the three new commands; core gates project only real core features;
  full suites use `--no-fail-fast`; admission estimates vary by actual component.
  Compact Python/library/HTTP hits now have `saccade-media-index-query.v1`, with
  a validating regression fixture. The established CLI query schema is unchanged.
  `saccade-py` is excluded from Rust publication and remains a wheel package.
- Regenerated CLI docs, schema index, and agent packs with an all-features binary;
  added the missing `media-http` compiled-feature receipt; retained the existing
  agent-pack size guard by condensing prose. CHANGELOG and third-party notices
  describe the actual additions and licence evidence. README remains untouched.

### Local gates and retained evidence

| Gate | Result | Scope / evidence |
| --- | --- | --- |
| Merge main / README preservation | PASS | `555a4ec`; README equals `origin/main` |
| Wave 8 fmt/docs/strict feature Clippy, Python Clippy | PASS | `wave8-final.log` |
| Full core/CLI/minimal tests, Linux workspace Clippy/tests | PASS | `wave8-final.log`, `release-check.log` |
| CI minimal/default/all-features builds/tests | PASS | `release-check.log`, `all-features-tests.log` |
| MSRV 1.89 default check / quality reference | PASS | Retry and quality-reference logs; initial disk refusal retained |
| Pinned SigLIP2 licence / export parity / Rust+Python+CLI text retrieval | PASS | `siglip-final-revalidation.log`, `python-models-corrective.log`, `cli-text-proof/receipt.json`; bounded generated inputs |
| Runtime and face cache verification | PASS | `cached_verified` in `wave8-final.log` |
| Combined Rust / Python installed models | FAIL | Missing pinned Rust OCR contract; original assertions retained |
| FFmpeg / light Python | PASS | `wave8-final.log` |
| Local release abi3 wheel build / install | PASS | x86_64 manylinux_2_39; corrective wheel/install logs |
| Dual-architecture manylinux_2_28 archives | CI-ONLY | Required x86_64 + aarch64 archives not supplied |
| Docker build / unprivileged health smoke | PASS | `wave8-final.log`; health endpoint only |
| Historical readers / release-notice and package-README regressions | PASS | `ci-remaining-summary.log` |
| Package file inventory / README / dependency notices | PASS | Corrective package-inventory and dependency-notices logs; inventory uses `--no-verify` |
| actionlint / shellcheck / generated docs | PASS | Corrective lint logs; `docs-retained-check.log` includes README check |
| Showcase expected stdout / shipped schemas | PASS | Retained all-features debug CLI; 40 JSON reports; release CLI identity remains unrun |
| Full Cargo package verification / all-features release CLI build | BLOCKED | Exit 75 preflight: disk below 25 GiB; no compilation executed |
| Genericity | PASS | Real external private denylist; `genericity.log`; repeated at final documentation state |


`scripts/gates-wave8.sh` ran twice. The final complete batch exited 1; subsequent
corrective wheel/install checks passed, while the combined model failures remain
real. The Rust and Python combined model tests fail at OCR status because the
supplied registry lacks an accent-capable, licensed and hash-pinned RTen OCR
contract. Face and image embedding sections succeeded in the Python check. No
fake recognizer, guessed artifact or changed OCR assertion was introduced.

The original release-check execution passed workspace strict Clippy/tests,
minimal/default builds/tests and the all-features build before the disk floor
refused later components. Those components were resumed separately under the
same environment and command limits; logs retain the original failures as well
as the resumed results. MSRV default similarly retains its initial disk refusal.
Each admitted component used at most 900 seconds, jobs 4, disabled debug symbols
and incremental compilation, the prescribed target and shared admission. Only
one Cargo command ran in this lane at a time. These CPU fixtures do not establish
GPU or native macOS/Windows qualification.

Disk crossed below 25 GiB after a build had already finished; subsequent
preflights paused automatically. The attempted stop sent no signal because the
process had exited. After shared recovery, retaining the all-features CLI and
joint test binary and removing only this lane's obsolete debug variants restored
29.21 -> 38.97 GiB. No other target or shared cache was removed.

Evidence root: `/mnt/linux-extra/moss-scratch/saccade-integ-w8/` contains
`MILESTONES.md`, original and corrective gate logs, per-command source/hash/exit
receipts, local wheel, retained binaries, model source/export receipts and final
cleanup/identity receipt. Model cache and immutable artifacts remain under
`/mnt/linux-extra/saccade-models/siglip2-base/` and content-addressed cache paths.
`FINAL-RECEIPT.json` binds tested source HEAD `ac4ff02`, Rust source/manifest/lock
hashes, retained binary/wheel/archive hashes and final capabilities. The final
report commit changes documentation only. Exact prescribed target was removed
under lane/Cargo locks: 18.76 -> 19.41 GiB; no build followed cleanup. Full package
verification and release CLI compilation were refused in two guarded sequences;
only 0.66 GiB of this lane's build cache remained, insufficient to restore the
25 GiB floor. Other owners' targets/caches were left intact. Functional showcases
and documentation checks therefore used the retained all-features debug binary.
Static lint input files contained Cargo/GPU-looking text; the wrapper classified
lint as GPU work. Queued lint attempts were canceled and repeated with its
supported CPU declaration, because actionlint/shellcheck inspect those files
without executing their contents. Admission remained in force.

### Remaining qualification

- After shared disk recovery above 25 GiB, rerun full Cargo package verification
  and the all-features release CLI build/showcases/docs from the final source.
  The retained debug showcases establish functional output only.
- Supply a reviewed pinned Rust OCR export/contract to pass the combined model
  gates. SigLIP2 broad natural-image retrieval/calibration remains unqualified;
  checkpoint parity and generated color-square retrieval have narrower scope.
- CI must produce/verify both x86_64 and aarch64 manylinux_2_28 wheel archives.
  The local installed abi3 wheel targets x86_64 manylinux_2_39 only.
- Native macOS/Windows jobs, four-target clean-machine/tagged-release identity,
  trusted/fork workflow behavior and publication remain CI/operator work.
- Browser layout/offline relocation, current Moss consumer acceptance, GPU
  execution-provider support and authorized live provider/pilot evaluation were
  not executed by this lane.

## Wave 9 integration — 2026-10-06

Integrated in `/home/etienne/dev/saccade-wt/wave9`, branch `feat/wave9`.
Merge `f778340` preserves original Wave 9 `4e8de95` and the required Wave 8
`ef04811`. Module registration, both rooted MCP dispatch paths/schema variants,
and both agent workflows are retained. No push, subagents, other-worktree Git
changes, or README edits.

### Implementation and decisions

- `53904eb` adds opt-in `quality_tile_size` to media options. The quality section
  exposes row-major edge-preserving mean luminance, population variance, contrast
  and interior absolute Laplacian energy using the Wave 9 spatial helpers and
  policy floor. It records policy/provenance and interpretation limits. Default
  media output remains unchanged. Invalid tile controls produce an attributed
  quality failure; strict mode refuses the record. Single-image descriptors do
  not invent paired FLIP, bias intervals, structural classes or acceptance.
- Capability discovery lists required-effect/spatial/layer/experiment evidence,
  fixed-camera sequence tiles with the graphics prerequisite, noise-aware offline
  references and preregistered blind trials; related-command navigation includes
  the reference/trial/media workflows. Existing comparison-question selection
  remains intact.
- New regressions assert partial-edge coverage, actual luminance/variance/contrast
  and alpha-over-black detail, invalid grids, strict rejection, opt-in omission,
  and feature-dependent router availability. No test or threshold was weakened.
- `7933ece` regenerates CLI help from the integrated all-features binary. Schema
  index and agent packs also regenerated; guide prose was condensed to preserve
  the original 4800-byte guard. CHANGELOG records the additions factually.
- Rejected self-comparing a single media input to manufacture change evidence;
  reversal removes the optional field/helper without changing default behavior.
  Paired image and temporal evidence retains its existing policies. Full decision
  and scout records live in this lane's scratch evidence.

- `e00caff` fixes the failing Wave9 schema/gate check: synchronize the existing
  explain change-class enum and use only real core features for core-only gates.
- `eb66626` fixes minimal-core Clippy by moving the byte-identical `layers::bundle`
  before test modules. No behavior or test changes.
- Coordinator steering: fetched origin and merged main `7e12dc8` as `e95889f`;
  `39ed8fe` is an ancestor. PyO3 extension-module is enabled only by maturin,
  preserving cargo-test linking. Affected workspace/Python checks are rerun.

### Gates and evidence

Evidence root: `/mnt/linux-extra/moss-scratch/saccade-integ-w9/`, containing
`MILESTONES.md`, `SCOUT.md`, `DECISIONS.md`, source/diff witnesses, per-command
logs and exit receipts. Each heavy component uses installed shared admission,
900 seconds maximum execution, jobs 4, disabled incremental/dev/test debug,
CPU-only execution and the prescribed target. The existing guarded command
supervisor is reused with its evidence destination explicitly set to this lane.
Both preflight and live monitoring enforce 25 GiB free; no private target or
manual deletion of another lane was performed. Shared admission may reclaim eligible shared caches through its installed safety policy.

The lane also reclaimed only its own obsolete saccade/core build outputs under
the lane Cargo lock (9.0 GiB); `own-cache-reclaim.json` records the operation.
A subsequent shared recovery request used the supported 35 GB trigger with the
unchanged 25 GB floor; the pass exited 0 and restored sufficient headroom,
without meeting every shared filesystem free-space goal.

Starting disk was below the floor (13 GiB); compilation paused while merge/wiring
proceeded. Shared admission requested recovery with its supported 25 GB floor;
the safe shared pass recovered enough space and the all-features bootstrap ran
successfully. Initial formatting failure occurred while the newly written code
was not yet formatted; `53904eb` contains the rustfmt correction. Bootstrap used
clean `53904eb`; later gate receipts bind their exact clean source revisions.

| Gate | Result | Receipt / scope |
| --- | --- | --- |
| Wave9 complete script, after main merge | PASS | `release-recovered-receipts.json`, all 8 subgates |
| Held-out spatial / required effects | PASS | 89/89 labels; 2/2 effect checks; hashes checked before/after |
| CI workspace Clippy / tests | PASS | `post-main-receipts.json` / `post-main-recovered-receipts.json` |
| CI feature matrix core-minimal / CLI-default / CLI-all | PASS | `ci-receipts.json`; minimal Clippy corrected in `minimal-corrective-receipts.json` |
| Rust 1.89 MSRV check / quality reference | PASS | `ci-receipts.json`; exact unchanged reference tolerances |
| Package inventory / all-feature archive verification | PASS | `ci-receipts.json`; three retained `.crate` archives |
| Python Clippy / Cargo package light fixtures | PASS | `post-main-recovered-receipts.json`; unchanged 3 fixtures |
| Maturin wheel / installed Python light fixtures + tile forwarding | PASS | local x86_64 `manylinux_2_39` abi3; `wheel-receipt.json` |
| Minimal CLI capability availability | PASS | `release-recovered-receipts.json` |
| All-feature debug/release docs + media/schema proof | PASS | `docs-media-corrective-receipts.json`, `artifacts/*-receipt.json`, `media-proof/receipt.json` |
| Full local release-check | PASS | `release-complete-retry-receipts.json`: all subgates PASS, 805.846s wall receipt, including admission |
| Genericity, actual external denylist | PASS | `genericity.log`; repeated for final report as `final-report-verification.log` (staged/worktree content) |

Initial Wave9 schema/invalid-core-feature failures and minimal-core Clippy failure
are retained with their corrective commits and PASS reruns. Post-merge workspace and first release-check
reruns were interrupted at the live disk floor; later gates were refused without
execution, then rerun after recovery. No interruption is treated as PASS. The full release script passed on its
unchanged retry after a second owner-only debug reclamation (9.5 GiB; release
artifacts preserved, `own-debug-cache-reclaim.json`).
The first retained-binary docs proof also failed: changing the executable basename
changed Clap Usage lines. The entire diff was basename-only; the scratch artifact
layout now preserves `saccade`, and the corrected invocation passed without
editing generated docs or weakening their check.

### Remaining qualification and cleanup

All final executable checks bind clean source `e95889f` (report-only final commit
will be newer). Feature-matrix/MSRV receipts also name earlier tested ancestors;
main's only executable delta is the PyO3 feature fix, covered by the affected
workspace and Python reruns. Retained all-feature artifacts, doctor/capability
JSON, wheel/native hashes, crate archives and showcase outputs remain outside
the prescribed disposable target.

Native macOS/Windows CI, aarch64 and manylinux_2_28 wheels, tagged distribution,
clean-machine installation, browser relocation, live providers/models, GPU,
renderer/performance acceptance and inherited Wave8 model-contract deferrals
remain unqualified by this CPU-only lane. No tests, thresholds or frozen spatial
policy were weakened; no README edits, push or publication occurred.

Cleanup completed at 2026-10-06 00:41 UTC: canonical, non-symlink target
`/mnt/linux-extra/moss-cargo-targets/codex-saccade-integ-w9` deleted under exclusive
owner Cargo and profile locks; absence verified in `cleanup.json`. Free space
rose from 23.751 to 37.221 GiB. Builds were refused or interrupted at the floor; reclamation/cleanup
continued while compilation was paused. A briefly held lock refused the first
cleanup attempt without deleting anything; the retry acquired all locks.
`showcase-receipt.json` retains 427 output hashes and verifies the showcase binary
matches the retained release SHA. `heldout-final-identity.json` verifies unchanged
policy, image hashes and all numeric pair results (only output locations differ).
