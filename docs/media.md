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
Opt into Wave 9 tile descriptors with `"quality_tile_size": 32` in the options JSON
(4..=4096). `quality.data.spatial` retains edge tiles, normalized sRGB luminance
over black, population variance, contrast and absolute Laplacian detail energy.
Its policy version and provenance pin bind the shared spatial basis. These are
single-image descriptors; paired FLIP, bias intervals and change verdicts require
the [spatial comparison policy](render-evidence.md). Invalid grids fail the quality
section and are rejected by `--strict`. Default output omits this optional field.
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

## Video keyframes

`saccade keyframes video.mp4 --out frames --json` invokes the user's external ffmpeg
and ffprobe; Saccade neither links nor distributes FFmpeg, whose build licence belongs
to its distributor. Missing executables return `video_decode_unavailable`, including a
pre-extracted directory alternative: `saccade keyframes input-frames --out selected
--sample-fps 2 --json`. Directory filenames sort lexically; timestamps and duration are
explicitly derived from the sampling grid, not guessed from names or source metadata.

Video files are limited to 4 GiB / 600 seconds, sampled at <=300 frames and scaled to
fit 640x640; subprocesses have a 120-second terminating deadline and bounded diagnostics.
`--sample-fps` defaults to 1 and is lowered for long inputs to stay within the sample
limit. Histogram SSE uses the existing deterministic penalized change-point recurrence;
`--shot-penalty` defaults to 0.15 (content-dependent, uncalibrated). One midpoint per shot
is retained, followed by hash candidate checks and histogram-verified deduplication.
No more than 64 unique keyframes are accepted; excessive shots fail rather than disappear.
Fast shots between samples or same-colour shots can be missed. Exported timestamps use the
normalized sample grid. Original video dimensions remain in decoder provenance.

`analyze-media video.mp4 --json` (or a frame directory) analyzes every unique keyframe through
the same image record path. Top-level image-only sections point to the video section;
keyframe records bind their encoded frame hash and timestamp. No temporary pathname is
presented as a persistent artifact. Existing output directories/files are never overwritten.

## Find usage

`saccade find-usage image.png target.png captures/ --json` accepts an image or a saved
image media record with available `FAST-oriented-BRIEF/1` fingerprints. Targets are
generic raster files or directories; failed files remain failed rows and produce exit 2.
A source record does not need the original pixels to estimate registration.

Hashes prioritize candidate evidence; a high full-frame Hamming distance cannot reject a
crop or page-capture match. The existing reciprocal ratio test and deterministic RANSAC
fit the least flexible successful translation/similarity/affine/homography. Matches report
source-to-target transform, source crop rectangle, inliers, RMS residual and confidence.
The crop is a source-space bounding rectangle of target coverage (an approximation for
rotation/projective edges). Confidence is an uncalibrated consensus score, not probability.
Exact encoded equality is separately labelled. Insufficient texture is `no_match` with an
explicit consensus reason, never a guessed transform. Matching does not establish rights.
