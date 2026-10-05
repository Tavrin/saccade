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
