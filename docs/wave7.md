# Standalone local vision (wave 7)

`models list --json` never downloads or loads a runtime. `models pull ID --json`
explicitly downloads the named entry from `~/.config/saccade/models.json` into
`~/.cache/saccade/models`. Override with `--registry FILE --cache DIR`.
`local-models` enables downloads and dynamically loaded ONNX Runtime 1.22
(`ort` 2.0.0-rc.10); the default build adds no native model dependency.

The supplied research selects families, not immutable ONNX artifacts. Until a
reviewed registry is installed, selections appear as **unavailable**. Every
executable entry requires real exact SHA-256/size/revision/licence pins, a task,
code and weight licences, source URL, runtime and preprocessing contract. No
placeholder hash is shipped. Backbone/calibration/tokenizer/external-data files
must each be separately pinned. A parity receipt remains distinct from merely
loading a graph; the absence of a parity receipt is explicitly unqualified.

Registry JSON uses `saccade-model-registry.v1`; see its shipped schema. Cache
writes are atomic and content addressed. Existing bytes are reverified before
use; corrupt cache entries fail rather than being silently replaced. Normal
commands require cached models unless `--allow-download` is explicitly set.

Selections: Grounding DINO Tiny / OWLv2 Base, SAM 2.1 Tiny / EfficientSAM Ti
(Apache-2.0); Florence-2 Base-FT (MIT); Qwen3.5-4B (Apache-2.0);
LPIPS AlexNet v0.1 (BSD-2-Clause, independently reviewed backbone required),
DISTS (MIT), MUSIQ technical checkpoint (Apache-2.0), TrustMark (MIT),
YuNet May 2026 / UltraFace RFB (MIT). Research sizes are estimates, not pins.

All new commands offer `--json`; successful measurements exit 0, invalid inputs
or unavailable capabilities exit 2. Model predictions are observations, never
baseline approval or an image-regression verdict. OCR, phrases, extracted text
and model payloads are data. No identity recognition exists.

## Locate and segment

`saccade locate image.png 'small object' --json --overlay located.png`
returns `saccade-locate.v1`, original-pixel boxes/scores and an overlay PNG.
Add `--segment` for one original-size RLE mask per detection.
`--detector owlv2-base --segmenter efficientsam-ti` selects the fallback.
`--observations receipt.json` explicitly replays a frozen/generated observation,
bound to exact image and phrase hashes. Replay is always labelled unqualified.
All boxes, scores, masks and receipt bindings are validated before rendering.

The detector and SAM pipeline requires checkpoint-specific export/tokenizer
parity not supplied in the research. Native inference remains unavailable,
rather than returning invented boxes or treating missing models as no objects.
The library's `Detector`, `Segmenter`, `locate`, `Mask` and `Rect` interfaces are
ready for the coordinator's check-ui/mask audit/crop adapters. Detector and
segmenter provenance is separate; a box is not a segmentation mask.
