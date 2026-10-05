# Remaining Wave 7c exports

All executable graph digests are in `crates/saccade-core/assets/wave7-models.json`.
Runtime archive and extracted-file digests are in `wave7-runtime.json` beside it.
No model bytes are vendored. `scripts/wave7/disposition.json` records accepted
Rust smokes and exact deferral evidence, and equals the bundled disposition asset.

- **SAM 2.1 Tiny:** run `scripts/models/sam2.1-tiny.sh`. The script downloads only
  pinned official HF checkpoint and hashed raw files at the SAM2 `sam2.1` and
  Microsoft ONNX Runtime `v1.22.0` tags. The matching SAM2.1 configuration is
  explicit; Microsoft's default SAM2.0 CLI is not used. README at the SAM2 tag
  explicitly covers checkpoints with Apache-2.0; exporter headers are MIT.
  `sam2-source-pins.json` records all fetched file identities. Tool installation
  uses `/mnt/linux-extra/saccade-models/venv`, offline wheels from `wheelhouse`,
  and `--no-index`. No GPU, automatic package-index fallback or CUDA extension.
  Actual export is **deferred**: no offline torch 2.5.1 wheel was available.
  Export calls, output signatures/parity and a Rust SAM2 adapter remain unrun;
  the script is a prepared recipe, not a claimed reproducible graph artifact.
- **LPIPS AlexNet / DISTS:** no export is run while independent ImageNet backbone
  rights are unknown. `quality-license-evidence.json` identifies the exact tagged
  Torchvision LICENSE and model definitions examined: they name separate
  `download.pytorch.org` checkpoints but give no explicit independent weight grant.
  That host is outside the lane download allowlist. Calibration/source licences
  cannot be inherited by backbone weights. Supplied research has no complete
  clearly licensed ONNX alternative. An export recipe requiring unlicensed weights
  would not close the acceptance criterion.
- **MUSIQ technical:** selected model is the official three-scale KonIQ checkpoint.
  Its GCS location has no supplied immutable generation/hash/checkpoint grant and
  is outside allowed downloads. No AVA/single-scale or unlicensed mirror substitution.
- **TrustMark Q:** the explicitly authorized mutable decoder URL was fetched once,
  47,401,222 bytes, SHA-256
  `ee3268f057c9dabef680e169302f5973d0589feea86189ed229a896cc3aa88df`.
  The pin records that mutability; future changed bytes fail integrity. Supplied
  research quotes the upstream MIT coverage for code and downloaded model files;
  attempted `v0.2.2` / `0.2.2` raw tags were absent, so exporter revision/licence
  statement were not independently refreshed. `trustmark-signature.json` comes
  from direct graph inspection, not executing upstream code. Rust runs 100 logits
  but full decoding stays unavailable until resizer/ECC/positive-sample parity.

`smoke-rust.py` runs one generated image per selected pinned model, each with
one CPU inference thread and an external 55-second process bound. These smokes
are behavioral evidence, not source/export parity or cross-platform qualification.
