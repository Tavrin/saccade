# Wave 7 decisions

- Authority: SPEC-wave7.md and shared rules in SPEC-waves-4-6.md; single agent,
  feat/wave7 only; item commits authorized; no web, downloads, live providers or heavy runs.
- Artifact pins: the supplied research contains no exact artifact SHA-256/bytes,
  immutable export revisions or parity receipts. Do not invent them. The selection
  catalog is distinct from the executable registry. Registry entries require real
  pins for every graph/tokenizer/backbone/calibration artifact. Until supplied,
  selected families are unavailable. Reversal cost: supply reviewed manifests;
  checkpoint-specific preprocessing/export qualification is still required.
- Runtime: use optional ort load-dynamic CPU runtime for explicit, documented tensor
  contracts, with independently pinned preprocessing metadata. Export-specific
  detector tokenizers, SAM encoder/decoder and Florence autoregression are not
  established in the research. Provide clean inference traits, validated replay
  observations and normalized export adapters; never label a replay as inference.
  Rejected guessed tokenizer/export interfaces. Reversal: implement qualified native
  adapters behind the existing traits; no CLI/observation contract changes needed.
- General VLM: Qwen3.5-4B has no qualified complete ONNX path. Use feature-gated
  loopback HTTP adapter with explicit model/runtime identity; Florence bounded
  extraction uses the same observation interface until qualified native export.
  Rejected a new native model dependency/port. Reversal: replace the provider behind
  the trait; processor/template/generation identity must remain recorded.
- Integration: wave 4/6 commands are absent. Coordinator maps observation provider
  to wave 4 catalog/evidence and registry/quality/watermark/faces to wave 6
  assess/inspect-image/check-ui. Learned pair metrics remain separate evidence and
  never change FLIP/SSIMULACRA2 status or baseline authority.
- Resource receipt (2026-10-05): df -BG /mnt/linux-extra reports 18 GB available,
  below 25 GB. Cargo check/clippy/test are prohibited until coordinator restores
  headroom. No alternate target or cache deletion. Source/format/script checks may
  run; compilation and runtime acceptance remain unverified.
- Coordinator restored disk headroom and admitted light builds (86 GB verified).
  No extra dependency was needed; reused crates already present in lockfile.
- Locate/segment: generated/frozen receipts and the full detector/segmenter API
  are implemented. Native selected pipeline is explicitly deferred: research has
  neither an exact tokenizer nor pinned graphs/SAM preprocessing/parity. A fake
  detector is used only in tests and explicit replay, never as a default model.
- Local observations: closed tasks and statements, exact image/order/transform and
  request/response/model identity, unknown-cost usage receipt. HTTP adapter only
  accepts literal loopback with no redirects. Rejected implicit remote endpoints
  and fallback keys. Reversal cost: add an explicitly authorized remote transport
  in wave 4; observation contract remains stable.
- Reused dependency licences checked in local registry Cargo.toml: ort rc.10,
  ureq 3.3.0, base64 0.22.1, sha2 0.10.9, tempfile 3.27.0 MIT OR Apache-2.0;
  fs2 0.4.3 MIT/Apache-2.0. No new dependency/lockfile change.
- Learned metrics: normalized scalar ONNX export adapter computes separately named
  LPIPS/DISTS/MUSIQ measurements with explicit resolution handling. Source native
  LPIPS backbone/calibration and MUSIQ multiscale parity remain unqualified until
  supplied pinned self-contained exports. Coordinator attaches named metrics to
  normal compare reports; standalone paired-file quality-score is implemented.
  Rejected guessed native model ports or a fused verdict. Reversal cost: one
  integration seam and a qualified scalar export; no metric contract changes.
- Watermark: native bounded known-message DWT/DCT decoding plus primary TrustMark
  trait/replay. TrustMark native decoding/ECC is deferred: supplied research leaves
  decoder/version/ECC/pin unresolved (0.4.0 docs vs 0.2.2 artifacts). Rejected an
  invented TrustMark pin or interpreting random recovered bits as a marker.
  Reversal: install a reviewed decoder adapter. Legacy exact upstream parity is a
  heavy gate; generated native-marker roundtrip is only focused proof.
- Faces: native YuNet stride-head and UltraFace boxes/scores decoders, deterministic
  NMS, bounded landmarks, crop geometry and strong privacy redaction are implemented.
  Actual selected artifact pin/parity remains external; fallback has no landmarks.
  Suggested crops retain all detected boxes or explicitly state impossibility.
  Rejected weak Gaussian blur for privacy; constant average box plus 15% margin
  is more destructive but removes facial detail. Reversal cost: change output
  redaction policy; detection/geometry/report interfaces remain stable.
- Provider adapters: Claude Sonnet 5.5 / GPT-6.1 Sol request/response mappings,
  explicit pixels/unit/thousand geometry transform, bounded structured statements,
  usage counters and model identity against generated recorded fixtures. No live
  transport. Keys have fixed user-file policy and redacted parse errors.
  Rejected assuming undocumented Sol image budgets or immutable dated snapshots.
  Reversal: coordinator binds actual authorized transport/revision receipts in
  wave 4; request fixture contract requires live qualification before promotion.
- HTTP loopback transport explicitly disables ambient proxies as well as redirects;
  the ureq 3.3.0 fetched source otherwise defaults to Proxy::try_from_env().
- MCP keeps the six-tool interface and canonical input/output containment. It
  mirrors local fixture/replay operations and native DWT only; CLI owns explicit
  downloads and selected model-runtime loading. Rejected tool-controlled network,
  HOME/config discovery and automatic model pulls. Reversal cost: coordinator
  adds startup-owned runtime/model authorization, not tool-granted authority.
- Validation catches malformed single-quote/double-quote credential values with
  redacted errors; no credential is ever serialized into a request mapping.

## Validation receipt (development gates only)

All Cargo invocations used CARGO_TARGET_DIR=/mnt/linux-extra/moss-cargo-targets/codex-saccade-w7,
nice -n 19 and -j 4 after checking >=25 GB free. Final observed headroom was
84–87 GB. The initial 18 GB refusal was resolved by the coordinator, not by
private caches or deleting other lanes' data.

- PASS: cargo check -p saccade with local-models/local-vlm/vision-providers;
  minimal core cargo check --no-default-features.
- PASS: cargo clippy -p saccade with defaults plus
  local-models,local-vlm,vision-providers,schema -- -D warnings.
- PASS: targeted core --lib wave7 with all wave features/schema: 22 passed,
  6 ignored. The final empty-phrase replay regression was then verified with
  the two targeted wave7::vision tests (both passed).
- PASS: targeted CLI --bin saccade wave7 with all wave features/schema: 4 passed,
  including generated command dispatch/PNG output, overwrite refusal, MCP
  containment/download refusal and positive MCP crop route/schema registration.
- PASS: cargo fmt --all -- --check, git diff --check,
  scripts/check-wave7-docs.py, bash -n scripts/gates-wave7.sh.
- Initial strict Clippy findings (constant chunk iterators, collapsible ifs,
  is_multiple_of) and one f32 inference error were fixed in the wave modules;
  final checks passed. Minimal CLI check has three existing local_cmd.rs
  unused-variable warnings without ai/workbench, so the strict CLI gate uses
  the normal defaults; no unrelated cleanup.
- NOT RUN: gates-wave7.sh, full crate/workspace tests, ignored model/network
  tests, real models/ONNX inference, HTTP servers, downloads, hosted providers,
  browser/showcases, release builds, GPU, cross-platform or merged integration.

## Remaining coordinator work (original snapshot; Wave 7b supersedes pins/adapters)

Supply reviewed immutable model manifests/pins and parity bundles (research has
no exact artifact hashes). Implement/qualify selected detector tokenizers and
SAM pipeline, and the exact TrustMark variant/ECC decoder. Wire wave 4 provider
catalog/authorization/identity/usage and wave 6 assess/inspect-image/check-ui;
attach independently named pair metrics to normal comparison reports. Native
model loading via MCP needs startup-owned runtime/model authorization.

README: list the standalone commands/features and link docs/wave7.md, with model
availability/qualification limits. CHANGELOG: record additive vision/model/
quality/watermark/crop and fixture-only provider contracts, explicit native
shortfalls and no verdict fusion. docs/cli.md: coordinator regenerates it after
registration conflicts are resolved. Those three files were deliberately not
edited. Cargo.lock was not changed; no new crate dependency was required.

## Wave 7b authority and artifact decisions

- SPEC-wave7b.md supersedes the initial no-download rule only for the exact pinned
  artifact URLs and their named companion files at the same immutable revision.
  crates.io additions are authorized; no other web access, providers, GPU, full
  gate or other worktree Git operations were performed. Item commits remain
  authorized by the shared rules. Native acceptance requires the installed runtime.
- The cache is `/mnt/linux-extra/saccade-models` (~1.6 GiB, below 6 GB). Each of seven
  downloaded artifacts matched the supplied SHA-256, with no integrity mismatch.
  Exact EfficientSAM bytes were measured locally (24,799,761 / 16,565,728).
  Thirteen named DINO/OWL tokenizer/config files were fetched from the same
  revisions; computed SHA-256 values are dated separately from host-reported
  graph hashes. `scripts/wave7/receipt.json` preserves all original receipts.
- The SAM archive really is ZIP with exactly config.yaml and encoder/decoder
  ONNX graphs. Inner SHA-256/bytes and tensor signatures were read without
  executing pickle/code. Its config lacks normalisation/prompt mapping and an
  exporter licence. Rejected inheriting a grant for the source checkpoint onto
  the unknown archive exporter. Reversal: supplied pinned exporter/processor grant
  plus preprocessing evidence, then implement the already defined Segmenter seam.
- LPIPS/DISTS/MUSIQ and TrustMark URLs cannot be completed under the allowed URL
  set: full ONNX exports/calibration/backbone licences, immutable KonIQ object,
  or Q decoder revision/hash are absent. Their existing backends remain available
  to reviewed manifests, but no selected-model measurement is fabricated.
  TrustMark ECC layout is specified by the report; it does not close neural
  decoder/resizer pins or supply an encoder/sample. Reversal: exact manifests and
  then native ECC/export qualification. Compare metric attachment is coordinator
  work after qualified LPIPS/DISTS exports; it was not smuggled into wave 4/6.
- DINO's real graph fixes RGB pixels/pixel_mask at 800×800. The supplied processor
  shortest-edge 800/longest-edge 1333 cannot fit a non-square image to that graph
  unchanged. Rejected stretching rectangles or silently claiming processor parity.
  Square path uses BERT; default rectangles route to OWLv2 with actual model identity.
  OWLv2 uses its pinned CLIP tokenizer (max16, pad0), per-channel mean square
  padding, 960 bilinear resize and explicit normalized-box original-pixel mapping.
  Detector score thresholds 0.4/0.1 and NMS0.5 are explicit example policy,
  not calibrated correctness. Reversal: one detector/export with a compatible graph.
- EfficientSAM consumes native RGB /255 and internally resizes/normalises. It
  transfers [1,256,64,64] embeddings, uses original-pixel box corners with labels
  2/3, selects highest-IoU of three masks and thresholds logits >0. No double
  ImageNet normalization or low-resolution output mistaken for the original mask.
- YuNet now pads bottom/right to multiples of32 without resizing original boxes;
  landmarks/boxes clip to original extents. UltraFace uses decoded boxes directly.
  Both retain explicit score0.6/NMS0.3 policy. UltraFace card MIT versus metadata
  Apache-2.0 discrepancy remains visible; no exporter/source parity is claimed.
- `local-models` alone adds tokenizers0.22.2 with default features off/fancy-regex.
  All 33 added resolved crate licences were checked in fetched source manifests;
  permitted license choices and notices recorded in THIRD_PARTY.md and the frozen
  dependency inventory. Default/minimal builds do not acquire this dependency.
- Standalone CLI defaults to the bundled registry only if the user registry is
  absent. Explicit registries still replace that set; there is no automatic pull.
  Missing SAM2 defaults to EfficientSAM; receipts name the actual segmenter.
  Rejected relabeling fallback as the primary or asserting source parity from pins.

## Wave 7b validation and limits

- Light checks PASS: lane-feature cargo check, no-default-features core check,
  strict Clippy for saccade and its core dependency, formatting/diff check,
  26 targeted core tests (10 heavy ignored), 4 targeted CLI/MCP tests, schema
  equality, documentation inventory, Python compilation and shell gate syntax.
  All builds used nice19/-j4 and the mandated target after headroom checks
  (at least64 GiB free at each build admission; pre-cleanup check60 GiB). No full gate was run.
- Generated fixture artwork is MIT; no downloaded photographs. Pillow generates
  a bottle, portrait and blank. Reference C API16 CPU smokes used one thread,
  one image per model, and timeout55s. DINO bottle score0.923584 / box normalized
  [0.501687,0.493426,0.312569,0.811830]; OWLv2 score0.244242 / box
  [0.502414,0.496212,0.308989,0.812759]. EfficientSAM yields embedding
  [1,256,64,64], masks[1,1,3,256,256], best IoU0.983409, area14,442 pixels,
  foreground at(128,100), no background at(5,5). YuNet portrait score0.874094;
  UltraFace score0.999582. Graph/fixture/runtime identities are in the committed
  smoke receipt; this is reference smoke evidence, not Rust/native/source parity.
- DINO output additionally has exactly226,800 intentional negative-infinity
  padded logits and zero NaNs. A focused regression now permits that padding
  only for DINO logits, while rejecting NaN, positive infinity and non-finite
  boxes/other outputs. Rejected blanket finite-output rejection (breaks the real
  graph) and blanket non-finite acceptance (permits invalid geometry).
- Both discovered installed runtime libraries report1.16.3. The individual Rust
  EfficientSAM smoke reached the ABI check and failed before graph inference:
  ort rc.10 expected1.22.x, found1.16.3; caught as typed dynamic ABI unavailable.
  No runtime download outside the authorized model/crates URLs was attempted.
  An earlier55s Cargo smoke cap expired during compilation, with no inference;
  the same targeted test with already selected lane features then established
  the actual ABI failure (runtime phase1.85s). This is not a successful Rust smoke.
- `gates-wave7.sh` now pulls/reuses only frozen immutable artifact pins, prepares
  generated fixtures, and invokes ignored real detector/segmenter/face assertions.
  Pair-metric identity/monotonic-distortion assertions are runnable after supplied
  reviewed LPIPS/DISTS manifests replace their explicit deferrals; no fake metric.
  Missing source-parity bundle/local VLM endpoint is reported DEFERRED. The final
  selected-adapter gate remains FAIL for SAM2/LPIPS/DISTS/MUSIQ/TrustMark gaps.
  Runtime path must be explicitly supplied with API22 support. Reversal cost:
  install/provide the allowed runtime outside this lane, then run the queued gate.
- README, CHANGELOG, docs/cli.md and wave4/6 integration were left to coordinator
  as requested. No push, merge, rebase, reset, stash or other-worktree operations.

## Final cleanup receipt

- Pipeline commit04ef9ef, registry commit84fc2c2, gate/behavior receipt commitbc0362d.
- The requested target `/mnt/linux-extra/moss-cargo-targets/codex-saccade-w7` was
  removed after all builds/tests completed. Exact directory identity/symlink
  guards were checked before Python shutil cleanup; absence verified afterward.
  A broad rm-style invocation was automatically rejected, then the guarded
  cleanup succeeded. No other lane cache was removed. Pre-cleanup headroom60 GiB;
  final observed64 GiB free. Generated Python bytecode cache also removed.
- Stop disposition: every selected model has an implemented pinned adapter with
  bounded reference evidence and an explicit native runtime prerequisite, or a
  recorded unknown artifact/licence/processor deferral. No outstanding question
  is needed to continue within this lane's allowed downloads/resources.
