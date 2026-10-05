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

## Small local VLM (`local-vlm`)

`saccade observe-local request.json --endpoint
http://127.0.0.1:9000/v1/chat/completions --runtime-revision REV --json`
uses an explicitly operated OpenAI-compatible server. Only literal IPv4/IPv6
loopback HTTP endpoints are accepted; redirects are disabled. There is no
ambient key, remote endpoint or weight download. `--response recorded.json`
decodes a recorded response instead, clearly labelled replay.

`ObservationRequest` records the closed task (`caption`, `ocr`, `grounding`,
`reasoning`), bounded data, model, encoder revision, token cap, and up to four
images. Each image has a catalog id, encoded PNG/JPEG bytes, original/presented
sizes and resize/padding transform. Returned boxes/points map back to original
pixels; invented ids, invalid geometry, truncated output, refusals and model
identity changes fail closed. Usage is preserved; unknown monetary cost is null.

Florence-2 Base-FT supports bounded caption/OCR/grounding through a local server
implementing this contract. Qwen3.5-4B supports general advisory reasoning.
Deploy the selected processor/template/quantization/runtime yourself and record
its revision. Neither model has a qualified complete native ONNX path in this
lane; HTTP is the selected adapter, not a claim of source/export qualification.

## Learned quality (`local-models` for inference)

`saccade quality-score image.png --metric musiq --json` measures the technical
MUSIQ checkpoint. `--metric lpips --reference reference.png` selects LPIPS-Alex
v0.1; `--metric dists --reference reference.png` selects the full-reference
fallback. Supply `--registry`, `--cache`, and `--runtime-library` for actual
inference; `--allow-download` is the explicit per-run download opt-in.
`--observations measurement.json` validates/replays a generated or frozen
`saccade-learned-quality.v1` report without loading a runtime.

Exports must be **self-contained** scalar graphs under `scalar-pair-v1` or
`scalar-image-v1`. Inputs are named NCHW floats; the manifest declares RGB/BGR,
scale/mean/std, size and exact preprocessing. A `[0,0]` size preserves original
resolution. Otherwise triangle resizing is recorded explicitly. Actual LPIPS
backbone/calibration and MUSIQ multiscale processing must live in the pinned
qualified export, not be guessed from its name. One finite scalar is required.

The report carries independently named metrics, model id/version and original
resolution handling. Distances are lower-is-better; technical quality is
higher-is-better. These numbers are not fused into FLIP/SSIMULACRA2 and do not
change their verdict. The coordinator attaches `QualityReport.named_metrics`
to paired compare reports and MUSIQ to wave 6 assess; this lane's standalone
command accepts paired files and emits its own comparison report. No calibrated
UI-quality thresholds or native published exports are claimed.

## Watermarks

`saccade watermark image.png --json` reports independent scheme outcomes and
absence limits, without a real/fake or AI-origin verdict. TrustMark's exact
variant/ECC/export is unavailable until pinned and qualified. `WatermarkDecoder`
is the primary integration boundary; `--observations report.json` explicitly
replays generated/frozen decoder observations with unqualified provenance.

`--expected-payload HEX --quantization-step 36 --minimum-agreement 0.9`
adds the weight-free configurable legacy invisible-watermark DWT/DCT decoder:
U-channel, Haar LL, 4×4 orthonormal DCT, maximum AC magnitude, modulo-step bits,
repeated-bit vote. It requires at least three repetitions and reports detection
only for the exact caller-declared message with sufficient per-bit agreement.
An arbitrary recovered byte string is never detection. Record embedding settings
and verify upstream parity for the exact workflow before relying on matches;
this lane tests its own generated marker and makes no universal compatibility
claim. Crop/resize/flat content can prevent detection. Payloads do not authenticate
signers or generators. No detection never establishes human origin.

The coordinator combines `watermark::inspect` with wave 6's C2PA indicators;
watermarks remain separate from cryptographically signed provenance.

## Faces and crop safety

`saccade faces image.png --json` returns original-pixel face boxes, landmarks,
scores and model provenance (`saccade-faces.v1`). `--detector ultraface-rfb`
selects the fallback. YuNet `yunet-v1` decodes stride 8/16/32 cls/obj/bbox/kps
heads; UltraFace `ultraface-v1` decodes boxes/scores. Both run optional pinned
ONNX graphs on CPU, with threshold 0.6 and deterministic NMS IoU 0.3.
YuNet's export requires fixed BGR NCHW input; preprocessing must be explicitly
recorded in the registry. Raw boxes clipped at the image edge are mapped to
original pixels; malformed landmarks fail. UltraFace has no landmark head and
reports an empty landmark list. Exact May 2026 export/source parity is pending.

`saccade crop-check image.png --crop 16:9 --crop 0,0,320,240
--focal-point 160,120 --json` evaluates every declared ratio or rectangle:
face included, cut or excluded. Ratios use the largest inscribed crop around the
focal point (image center by default). Suggested safe translations preserve all
detected boxes if geometrically possible; impossible requests are explicit.
No detected face does not certify face absence or a safe publication crop.

`--observations faces.json` supports explicitly labelled generated/frozen face
receipts. `--blur-faces redacted.png` on either command writes a **new** PNG,
strongly redacting detected boxes plus a 15% margin with a constant average
colour. This sacrifices detail for privacy; it is stronger than a weak Gaussian
blur. Undetected faces remain a recall limitation. Originals are never changed.
No identity labels, face embeddings or identity recognition are present.

## Provider mappings (`vision-providers`, interface only)

`saccade provider-map request.json --provider claude --json` builds the Claude
Sonnet 5.5 structured request. `--provider gpt` builds GPT-6.1 Sol's Responses
request (`store:false`). `--response recorded.json` decodes a fixture only.
There is no live transport, implicit provider fallback, live qualification or
credential reading by this command. Generated fixtures have no real image data
or provider results. Caller-provided images are embedded in request output;
request JSON is evidence, so handle it under the input's privacy policy.

`--coordinates pixels|unit|thousand` declares the wire convention and maps
boxes/points through the exact presented-image resize/padding transform. There
is no coordinate guessing or silent clamping. Requested/returned model identities
must match; unknown ids, geometry, refusals, truncated responses and malformed
structured observations fail closed. Claude cache read is added once to ordinary
input; cache creation is separately captured. GPT cached/reasoning counters are
subsets of input/output totals. Missing usage/cost remains null.

The local `ObservationProvider` trait is implemented by `RecordedProvider`;
coordinator maps it to wave 4's catalog, immutable identities, egress authority,
budgets and attempt ledger. `load_credential` only reads
`~/.config/saccade/anthropic.env` (`ANTHROPIC_API_KEY`) or `openai.env`
(`OPENAI_API_KEY`) and never reads ambient key variables. Credential has no
Debug/Display/serialization, and parsing errors are redacted. No key is needed
for recorded mapping. Undated API model ids remain unqualified aliases; do not
invent a snapshot/revision or treat schema validity as localization accuracy.
`store:false` is not a promise of zero retention or ZDR.

## MCP and integration

The existing six MCP tools remain. `saccade_inspect` adds `models_list` with
explicit registered `registry` and `cache` paths. `saccade_measure` adds
`vision_locate`, `vision_quality`, `vision_faces`, `vision_crop`,
`vision_watermark`, plus feature-gated `vision_local` and `vision_provider`.
These consume explicit frozen/generated receipts; `vision_watermark` also runs
the native known-message decoder. Every image/receipt is resolved through
`RootPolicy`; overlay/redaction files require the separate startup `--out-root`.
No ambient HOME/config/cache access, provider HTTP call or model download can be
introduced by tool arguments. `models_pull` explicitly returns unavailable;
use the human-operated CLI to authorize downloads. Native model inference stays
on the CLI until the coordinator wires startup-approved runtime/model inputs.

Results use the standalone versioned schemas. Replay is explicit and source
parity is false. Tools never emit PNG bytes by default; files are the requested
output artifacts. The coordinator adds bounded paging when wiring these into
wave 4's evidence catalog; large masks should be handled as artifact files.

## Coordinator gates and qualification limits

Run `scripts/gates-wave7.sh` only in the coordinator's heavy queue. It checks disk
headroom before each build, fmt, strict Clippy, full touched-crate tests,
no-default core tests, schema drift, docs and every ignored model/network group.
The implementation agent does not run it. No live hosted provider gate exists.
The selected-native-adapters gate explicitly fails while detector tokenizer/SAM
and TrustMark ECC integrations are deferred; compatibility receipts cannot close
that gap. No showcase or browser code was touched, so no browser/showcase gate is
needed.

Set `WAVE7_HEAVY_FIXTURES` to an external frozen parity bundle containing:
`models.json`, content-addressed `cache/`, `faces.png`, `face-negative.png`,
`yunet-2026may.faces.json`, `ultraface-rfb.faces.json`, `quality.png`,
`reference.png`, the three `<model-id>.quality.json` receipts, `locate.png`,
`grounding-dino-tiny.locate.json`, `owlv2-base.locate.json`, `watermarked.png`,
`trustmark.watermark.json`, `legacy-watermarked.png`, `legacy-payload.json`
(byte array), and `local-vlm.request.json`. Supply `WAVE7_RUNTIME_LIBRARY`.
No gate downloads models or changes pins. Source-parity receipts must be
independently reviewed, separately pinned as artifact role `parity`, and match
the manifest's `parity_sha256`. The boolean reports this declared parity
provenance; graph loading alone never establishes it.

Real face and quality gates run cached graphs against frozen source outputs
(score tolerance 0.001, face edges 1 pixel); face tests include a negative image
and pair metrics include identity. Detection/SAM/TrustMark tests currently check
external receipt **compatibility only**, not native inference. Legacy DWT's
upstream workflow parity is separate from the generated marker roundtrip.
Set `WAVE7_LOCAL_VLM_ENDPOINT` and `WAVE7_LOCAL_VLM_REVISION` for the loopback
smoke test; it establishes contract/availability, not domain accuracy.

## Wave 7b pinned native adapters (2026-10-05)

When no user registry exists, the standalone CLI uses the bundled exact manifest
in `crates/saccade-core/assets/wave7-models.json`. An explicit `--registry` still
selects only that registry. `scripts/wave7/pull.py` pulls or verifies the frozen
artifact catalog in `/mnt/linux-extra/saccade-models`; graph and auxiliary hashes
are checked before use. Locally computed tokenizer/config digests are dated and
kept distinct from host-reported graph hashes. Weights are never stored in Git.

With `local-models`, `locate` uses the pinned BERT DINO adapter on square images.
The supplied graph is fixed at 800×800; preserving its processor on rectangles
requires the OWLv2 fallback, which pads with per-channel mean and resizes to
960×960. Detector identity always names the graph actually executed. Detection
thresholds are 0.4 (DINO) and 0.1 (OWLv2), with explicit NMS 0.5; these example
settings are not calibration. The query is one phrase, and the receipt binds it.
`--segment` selects EfficientSAM when the default SAM2 bundle is unavailable:
original-pixel box corners, encoder/decoder inference, highest-IoU candidate,
and positive-logit foreground runs. SAM2 remains unavailable because its verified
archive lacks a complete exporter licence and preprocessing contract.

YuNet uses native BGR pixels with bottom/right zero padding to multiples of 32;
UltraFace uses RGB 320×240 and `(pixel−127)/128`. Both apply the recorded score
0.6/NMS 0.3 policy. Successful face detection never certifies absence of other
faces. LPIPS, DISTS, MUSIQ and TrustMark remain unavailable until their incomplete
pins are supplied; they never produce fabricated measurements or watermark
presence. LPIPS/DISTS compare attachment remains coordinator work after exports
exist. The reason for each missing model is in `scripts/wave7/disposition.json`.

`gates-wave7.sh` prepares MIT-licensed generated bottle/portrait/blank fixtures
with Python Pillow, verifies frozen pins, and runs ignored native inference tests
with box/mask/negative-image/crop assertions. Set `WAVE7_RUNTIME_LIBRARY` to an
explicit installed CPU ONNX Runtime API22 (1.22+) library. Development reference
smokes used an existing 1.16.3 C API runtime and succeeded for five models, but
Rust inference failed at that ABI boundary before executing a graph. The complete
receipts and tensor declarations are in `scripts/wave7/smoke-receipt.json` and
`scripts/wave7/signatures.json`. These checks do not establish source/export
parity, native Rust runtime acceptance, cross-platform behavior or integration.
