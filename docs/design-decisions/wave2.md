# Wave 2 decisions

## 1. Snapshot variation and drift

Extend the local history index additively. A producer-assigned run ID and an explicit frozen environment identity distinguish independent trials from artifact copies. Identical pixels across declared runs count as independent observations. Legacy rows remain readable and retain their provisional artifact-variation advice. They cannot establish independent normal variation.

Only explicitly declared unchanged-build repeats contribute to normal variation. Ten are required for advice. The supplied environment identity must cover browser/device, fonts, viewport, readiness, warmup, temporal phase and cache protocol. The same baseline and configuration remain the anchor across revisions. Recorded sequence orders observations; report timestamps do not establish capture chronology. A conservative median-window and monotonicity heuristic flags cumulative drift beyond the observed repeat range. It reports a candidate and fresh-repeat advice, never commit attribution. Drift suppresses tolerance advice; no command edits policy.

Residual: scalar history cannot localize timestamp/font defects. Real browser nuisance-alert and capture-fix evaluation remains pilot work. Producer declarations do not independently prove capture independence.

## 2. Compression quality

Pin the published `ssimulacra2 = 0.5.1`, not repository HEAD. The downloaded crate manifest declares Rust 1.65 and BSD-2-Clause; its LICENSE permits source/binary redistribution with notices. Its transitive yuvxyb family uses MIT. No toolchain minimum is raised. Butteraugli 0.9.3 requires Rust 1.89 and is unnecessary for this bounded implementation. Compression is a separate Cargo feature, enabled in the default CLI and absent from minimal core.

The sweep ingests externally encoded outputs, records exact bytes and hashes, and selects the smallest delivered file satisfying the frozen policy. Lowest encoder quality is reported separately and only within one encoder/settings family. Every stage must meet cumulative score policy; incremental scores compare to the preceding stage. Scores are never added. Missing candidates suppress selection. The immutable original and resized reference are separate hash-bound inputs. Pixel policy requires externally normalized opaque RGB8 sRGB; embedded EXIF/ICC, alpha, HDR, and dimension changes are rejected instead of silently reinterpreted. Viewing conditions are required and human review remains pending.

Residual: independent C++ metric parity and blind human/pipeline evaluation on permissioned images remain unrun. Protected-detail policy can use the localized measurement command from item 4; a whole-image score does not qualify data textures, normals, GI, or alpha coverage. No encoder or live Drupal/Imgix adapter is invoked.
