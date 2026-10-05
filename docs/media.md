# Media analysis

`saccade analyze-media IMAGE --json --output-size 1200x800` produces the same
`saccade-media-record.v1` as `saccade_core::media::Analyzer` and the Python API.
URL inputs require `media-http`, use a 30-second timeout and a 64 MiB encoded cap.
SDR decode is limited to 16 million pixels; orientation stays as encoded.

`--profile cpu-lite|cpu-full|gpu` selects defaults over individual section options.
`--options options.json` supplies `faces`, `text`, `embeddings`, `saliency`,
`description`, `include_gps`, `output_sizes`, `crops`, `strict` and optional `profile`.
`cpu-lite` disables model sections; `cpu-full` attempts supplied installed models.
Unsupported GPU execution is a typed section failure, never silently CPU inference.
`--registry` supplies the existing pinned model registry and embedding/OCR contracts;
`--model-dir` sets the cache. CLI and MCP inference never download models.

Identity binds the retained encoded input, dimensions, format and ICC presence evidence.
Metadata includes bounded EXIF, XMP, IPTC, unsigned credit/copyright candidates with
source fields, and offline C2PA state. Missing metadata never becomes invented credit.
GPS is withheld by default. ICC presence is not colour-profile validation.
Quality exposes content-dependent wave 6 measurements and geometric resolution fitness.
Faces use the installed wave 7 detector; deterministic colour-surround saliency supplies a
fallback focal point. Crops carry an explicit `face_crop_checks_available` flag; no face
inference does not certify absence. OCR confidence stays null when the engine lacks it.
Fingerprints retain aHash/dHash/pHash and bounded FAST/oriented-BRIEF keypoints.
Embedding payloads use base64 little-endian float32, model ID/pin and uncalibrated status.

Every section retains status, provenance and diagnostic milliseconds. `--strict` rejects
an attempted failure; disabled sections are skipped. The focal section can retain useful
saliency fallback data while its requested face detector failed.
Description is off by default. Library `analyze_with_provider` accepts an explicitly
bound caption request and existing observation provider, and marks its output as a draft.
No provider authority is obtained from media bytes, OCR or model text.
