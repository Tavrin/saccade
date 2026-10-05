# Wave 7 progress

Original wave 7 snapshot below; the Wave 7b continuation is the current disposition.

7.1 partial (a1c568c) — pinned registry/cache/status/pull implemented; selected real artifacts deferred pending supplied exact pins/export/parity. Disk headroom restored; light gates admitted.
7.2 partial (236230f) — validated locate/segment API, generated fixtures, replay/overlay CLI; native selected export/tokenizer parity deferred.
7.3 done (4754efc, external-runtime route) — local trait and feature-gated bounded loopback HTTP adapter; native Florence/Qwen ONNX unqualified.
7.4 partial (c555ce0) — scalar ONNX/replay quality-score and named metric reports; exact selected exports and normal-compare attachment await pins/coordinator.
7.5 partial (6ac275e) — watermark command/composable report, native known-message DWT/DCT, TrustMark trait/replay; exact TrustMark ECC/export/pin and upstream legacy parity deferred.
7.6 done (b604b4e, artifact qualification pending) — YuNet/UltraFace ONNX decoders, faces/crop-check/strong-redaction CLI, generated geometry/privacy tests.
7.7 done (13e3ef5, interface only) — Claude/GPT structured request/fixture-response mappings, explicit geometry/usage/key policy; no live calls.

Validation: light checks/fmt/strict Clippy/docs/schema pass; 22 focused core + 4 CLI/MCP tests pass, 6 heavy tests ignored. Gate script written, not run. Selected-native gate intentionally fails for the recorded detector/SAM/TrustMark deferrals. Common registrations/schemas/gates are in the final chore(wave7) follow-up commit.

## Wave 7b continuation

7b.registry done (84fc2c2) — five executable graph bundles with exact pins; 13 detector auxiliary hashes computed locally 2026-10-05; verified SAM archive/inner graph hashes retained separately, not promoted. No fabricated source-parity digest.
7b.detectors partial (04ef9ef) — native DINO square/OWLv2 fallback and EfficientSAM adapters implemented; reference CPU smoke passes; native Rust acceptance blocked by installed API16 versus required API22.
7b.faces partial (04ef9ef) — dynamic YuNet padding and UltraFace decoding corrected; generated portrait reference smokes pass; native Rust acceptance blocked by runtime ABI.
7b.deferred — SAM2 archive exporter licence/preprocessing; LPIPS/DISTS complete exports/backbone licences; MUSIQ immutable checkpoint/conversion; TrustMark immutable decoder/resizer/encoder or sample. Interfaces retained; no false measurements.
7b.gates partial (bc0362d) — generated inference/assertion tests and immutable pull/reuse gate implemented; full gate not run. Final development receipts follow below.

Final Wave 7b development receipt: PASS fmt/diff-check, check with lane features,
minimal core check, strict CLI/core Clippy, 26 focused core tests (10 ignored),
4 CLI/MCP tests, derived schema regression, documentation inventory, Python
syntax and gate shell syntax. Five bounded reference API16 CPU smokes passed;
Rust EfficientSAM smoke FAILED before inference (API22 required, installed
1.16.3). Full gate, ignored inference assertions on API22, source/export parity,
GPU, provider, cross-platform and merged integration were NOT run/qualified.
Mandated target directory removed; final observed headroom 64 GiB. Cleanup and
qualification limits are recorded in WAVE7-NOTES.md.

## Wave 7c continuation

7c.runtime done — official pinned ONNX Runtime1.22.0 pull/cache/ORT_DYLIB_PATH, retained notices and typed API16 incompatibility; Docker/wheel consumption documented only.
7c.inference done — one-image CPU Rust smokes pass for DINO, OWLv2, EfficientSAM Ti, YuNet and UltraFace; rectangle DINO IoU>0.8 after valid-mask-coordinate correction; no source parity claim.
7c.trustmark partial — mutable Q decoder hash pinned; Rust neural smoke pass; full decoder explicitly deferred on ECC/resize/positive-sample qualification.
7c.exports deferred with evidence — SAM2 official checkpoint/tagged sources prepared, offline torch wheel absent; LPIPS/DISTS independent backbone grants missing; MUSIQ official GCS checkpoint outside allowed sources/incomplete pin.
7c.gates done — bounded smoke runner, runtime provisioning/CLI regression, explicit deferrals and opt-in all-model release requirement; full gate not run.
