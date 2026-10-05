# Wave 2 decisions

## 1. Snapshot variation and drift

Extend the local history index additively. A producer-assigned run ID and an explicit frozen environment identity distinguish independent trials from artifact copies. Identical pixels across declared runs count as independent observations. Legacy rows remain readable and retain their provisional artifact-variation advice. They cannot establish independent normal variation.

Only explicitly declared unchanged-build repeats contribute to normal variation. Ten are required for advice. The supplied environment identity must cover browser/device, fonts, viewport, readiness, warmup, temporal phase and cache protocol. The same baseline and configuration remain the anchor across revisions. Recorded sequence orders observations; report timestamps do not establish capture chronology. A conservative median-window and monotonicity heuristic flags cumulative drift beyond the observed repeat range. It reports a candidate and fresh-repeat advice, never commit attribution. Drift suppresses tolerance advice; no command edits policy.

Residual: scalar history cannot localize timestamp/font defects. Real browser nuisance-alert and capture-fix evaluation remains pilot work. Producer declarations do not independently prove capture independence.

## 2. Compression quality

Pin the published `ssimulacra2 = 0.5.1`, not repository HEAD. The downloaded crate manifest declares Rust 1.65 and BSD-2-Clause; its LICENSE permits source/binary redistribution with notices. Its transitive yuvxyb family uses MIT. No toolchain minimum is raised. Butteraugli 0.9.3 requires Rust 1.89 and is unnecessary for this bounded implementation. Compression is a separate Cargo feature, enabled in the default CLI and absent from minimal core.

The sweep ingests externally encoded outputs, records exact bytes and hashes, and selects the smallest delivered file satisfying the frozen policy. Lowest encoder quality is reported separately and only within one encoder/settings family. Every stage must meet cumulative score policy; incremental scores compare to the preceding stage. Scores are never added. Missing candidates suppress selection. The immutable original and resized reference are separate hash-bound inputs. Pixel policy requires externally normalized opaque RGB8 sRGB; embedded EXIF/ICC, alpha, HDR, and dimension changes are rejected instead of silently reinterpreted. Viewing conditions are required and human review remains pending.

Residual: independent C++ metric parity and blind human/pipeline evaluation on permissioned images remain unrun. Protected-detail policy can use the localized measurement command from item 4; a whole-image score does not qualify data textures, normals, GI, or alpha coverage. No encoder or live Drupal/Imgix adapter is invoked.

## 3. Capture inventory

Use a separate typed inventory and JSON output so historical measurement reports remain readable. The generic command consumes an expected/supplied manifest and the exact comparison report. Stable case IDs establish correspondence; image equality cannot infer it. Every unique expected ID receives one outcome; outcome counts sum exactly to that count. Raw duplicate declarations, shared filenames, unknown supplied IDs, unassigned comparisons, stale/skipped/quarantined cases and unusable captures remain explicit. A failing measured pair can still have complete capture coverage. A captured case counts as compared only with a matching encoded capture hash and valid pass/fail measurement.

Playwright's reporter declares every enumerated test and records all attempts. A passed test without a complete expected/actual attachment pair is missing assurance. It assumes one required snapshot per declared test, with additional snapshots discovered as separate indexed cases. Retries retain duplicate stable identities and therefore incomplete coverage rather than concealing flakiness. Ingest emits `inventory.json` alongside the ordinary report; incomplete coverage exits 1. Older attachment-only manifests remain usable but cannot establish suite completeness.

Residual: passing tests must attach their expected and actual images explicitly. The reporter does not execute screenshots or infer baseline files. Integrators needing multiple predeclared states should use a generic suite manifest instead of treating discovered attachments as a complete state declaration. Live Playwright/browser qualification remains unrun.

## 4. Localized change

A separate diagnostic accepts an explicit pixel box, binary inclusion mask, frozen region, or reference-bound DOM selector geometry. The full frame is measured before region aggregation; pixels are never blacked out before FLIP. Record the inclusion mask on disk before measurement, then compute independent intended, protected-complement and one-pixel boundary statistics. Exact complement preservation uses native sample bytes, including alpha and 16-bit precision. Ordinary config exclusions are not consumed. An optional perceptual policy uses maximum complement FLIP under a declared PPD; neighborhood support may cross the boundary.

DOM selector geometry must bind to the reference file hash and dimensions. Exactly one selector and one box are required; disappearance/ambiguity abstains. Reference regions survive candidate deletion. Playwright ingest retains producer-supplied DOM metadata; it never runs a browser or invents geometry. RGB/alpha SDR types must match, and HDR/float inputs are explicitly rejected. Full or empty masks are rejected because both region and complement need measured coverage. The diagnostic reports spatial changes and collateral, never semantic edit success or approval.

Residual: live selector/scroll/device-scale capture and reflow pilot are unrun; the producer must convert CSS boxes to screenshot pixels. Semantic success still needs an independent assertion. Perceptual 16-bit reduction is documented beside exact native evidence.

## 5. Grounded explanations

Freeze a source-hash-bound region/fact catalog before evaluating proposals. Deterministic templates explain full-frame and hotspot FLIP statistics, thresholded hotspot areas, and independent localized native-change measurements. Every fact carries an exact JSON pointer. Every atomic claim must cite exactly one region and one compatible fact, use a finite supported kind and quote the exact observed number. Unknown IDs, wrong numbers, unrelated citations, compound observations and semantic/causal kinds are dropped with index/reason diagnostics. Rejected wording is not echoed. Source and catalog hashes bind the result.

Optional offline model proposals use the same finite typed contract and verification. Arbitrary prose and prompt-only semantic checks are rejected; application-controlled templates render accepted facts. No provider call or model approval authority is added. MCP inspection exposes paginated claims plus their region/fact citations and source pointer, not prose alone. A valid numeric claim establishes consistency with its source measurement, not physical correctness, requested-edit success or cause.

Residual: semantic/OCR verification and independently labeled human support/coverage benchmark are unrun. The finite numerical vocabulary therefore abstains on semantic observations and all causal assertions. Masked/thresholded hotspot facts retain their measurement limits.

## 6. Regions described in words: qualified fallback

Implement the explicitly allowed mask-import fallback and optional model plumbing. On this host Python `torch`, `onnx` and `onnxruntime` are absent, no ONNX Runtime library is registered, and the official SAM exporter `convert_to_onnx.py --help` fails at `import torch` with `ModuleNotFoundError`. The lane has no checkpoint-specific ONNX export, pinned preprocessing/tokenizer or source-parity witness. Source-to-ONNX qualification could not be completed here; text-to-mask remains unavailable rather than returning unverified detections.

The shipped model manifest pins Grounding DINO Tiny and SAM 2.1 Hiera Tiny checkpoints by exact Hugging Face revision, upstream LFS SHA-256, byte count and Apache-2.0 model-card license. These are checkpoint provenance, not ONNX/export qualification. Weights are never committed. Explicit `regions cache` downloads declared artifacts at runtime into a content-addressed cache, checks size/hash before atomic persistence, and rechecks cached files. Ordinary import/comparison never downloads. A manifest may later carry self-contained detector, SAM encoder/decoder, tokenizer and parity artifacts.

`semantic-regions` is optional. Pin `ort`/`ort-sys` 2.0.0-rc.10 (MIT OR Apache-2.0, Rust 1.81), with `load-dynamic` and no build-time runtime downloads. The runtime boundary loads an explicitly supplied ONNX Runtime 1.22 library (MIT) and cached self-contained graphs, with one CPU thread. Loading proves neither inference nor parity. Dynamic loader failures become capability errors. The existing ureq (MIT OR Apache-2.0), fs2 (MIT OR Apache-2.0) and tempfile (MIT OR Apache-2.0) handle bounded explicit downloads, locks and temporary persistence.

Import records the original phrase, explicit selection, model-not-run status and source-mask digest in the frozen inclusion region. Measurement reuses the reference mask when an object disappears; no surviving-object selection can hide deletion. Model ambiguity and absent-target handling are residuals until inference exists.

Sources: [ort release](https://crates.io/crates/ort/2.0.0-rc.10), [detector model card](https://huggingface.co/IDEA-Research/grounding-dino-tiny), [SAM checkpoint](https://huggingface.co/facebook/sam2.1-hiera-tiny), [official exporter](https://github.com/microsoft/onnxruntime/blob/main/onnxruntime/python/tools/transformers/models/sam2/convert_to_onnx.py). Registry licenses were inspected from published crate manifests; model hashes are explicitly upstream declarations until downloaded and verified.

Residual: full Grounding DINO → SAM 2.1 inference, checkpoint-specific export/parity, tokenizer/preprocessing, absent-object/ambiguity accuracy, model/device determinism, CPU/GPU latency and human correction benchmark. The fallback does not claim those gates passed.

## 7. RenderDoc divergence localization

Implement a version-pinned optional Python replay worker using RenderDoc's official 1.34 bindings (MIT). It opens captures, checks local replay support, requires Vulkan, uses conservative replay settings, enumerates draw/dispatch/clear actions, selects every bounded event and extracts native color/depth and writable texture/buffer resources. It records exact format, dimensions, subresource, capture-local IDs, payload bytes/hashes, marker ancestry and input IDs. It never compares display thumbnails. A second independent replay checks byte repeatability; missing/error resources leave qualification unknown. Explicit limits bound actions (2048), individual resources (64 MiB) and total extracted bytes (512 MiB by default). No replay runs implicitly in Rust or ordinary comparison.

Rust checks raw payload paths, lengths and hashes before alignment. Unique marker/action signatures align through an LCS with inserted/deleted actions retained; repeated/unmarked ancestry and duplicate keys abstain. Matching resource roles retain incompatible formats and missing resources as unknown. Capture-local event/resource IDs are evidence only. Scan all aligned resource observations rather than binary-searching a non-monotonic predicate; a later clear may restore equality. The first observed divergence is a candidate localization. Root cause and final-output relevance remain unproven; bound input IDs are follow-up evidence, not asserted causes. Incomplete coverage exits 2; complete divergent evidence exits 1.

Host evidence: `renderdoc` cannot be imported by Python, `renderdoccmd` is absent from PATH and no RenderDoc library is registered. The worker's actual unavailable-capability probe exits 2 without creating output. Python syntax and Rust synthetic-capture-shaped alignment/payload fixtures are validated. No live replay was run and no GPU capture was produced.

Sources: pinned official [replay API](https://github.com/baldurk/renderdoc/blob/v1.34/renderdoc/api/replay/renderdoc_replay.h), [pipeline-state API](https://github.com/baldurk/renderdoc/blob/v1.34/renderdoc/api/replay/pipestate.h), [descriptor types](https://github.com/baldurk/renderdoc/blob/v1.34/renderdoc/api/replay/common_pipestate.h). No capture parser or proprietary graphics dependency is embedded.

Residual: live Vulkan replay compatibility, extraction/self-replay, planted real GPU defects, final-resource relevance/dependency graph and controlled causal intervention. D3D12/GL, all-mip/all-layer/MSAA coverage and numeric texture tolerances remain unsupported. A bound buffer is extracted in full; a texture records one bound view mip/layer and sample zero. Uploads, indirect/unbound resources and synchronization need further instrumentation.

## Final history review

Restrict artifact-only tolerance advice to historical rows without trial declarations. Declared revision measurements must not also train that legacy advice. Drift monotonicity uses the recent ten runs while the effect retains the original first-five anchor; a long stable prefix no longer dilutes a sustained recent drift. Capture independence and unchanged-build qualification remain separate and unchanged.

History JSON is a bounded preview: at most six run witnesses per shown group, explicit omission counts and a full-witness artifact via optional `--out`. The file retains every run for the selected groups before preview trimming. Multiple groups are trimmed with counts when necessary, while `--entry` selects a test. A 20-run CLI fixture verifies bounded output, retained complete witness, no tolerance advice for drift and unchanged policy. This follow-up fixes practical output-budget and short-drift gaps found during final review.

## Final inventory review

With an explicit Playwright inventory, copy contained artifacts even when their image headers are unusable and let the comparison record the decode error. The inventory then retains that unusable case beside successfully compared cases instead of aborting the entire suite before accounting. Attachment-only historical ingestion keeps its existing header validation. A passing-plus-corrupt-capture CLI fixture verifies exact accounting and incomplete coverage. Path containment remains enforced before all copies.

## Final packaging review

The release archive guard initially rejected the new `models/semantic-regions.json` path. Allow exactly that core manifest, require it in the source archive, and retain rejection of weights and other model files. Regression checks exercise the manifest/weight boundary. Update the packaged library README to list compression and optional semantic-region plumbing with the inference residual. This is an archive-inventory correction; no runtime or model qualification is inferred from packaging.
