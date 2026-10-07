# Timed text against frames

Check plain UTF-8 SRT or WebVTT subtitles/captions against timestamped image
samples. Use the same command for captioned footage, presentation recordings or
illustrated sequences. Text and timestamps are data; no provider is called.

```sh
saccade timed-text cues.vtt frames/map.json --region 20,280,600,70 \
  --sources ocr-observations --json --out timed-report
saccade timed-text cues.srt frames/map.json --region 20,280,600,70 --ocr --json
```

Video extraction stays external, as in the [frame-map contract](frame-map.md).
For example, extract all frames from a small constant-rate video with
`ffmpeg -i video.mkv frames/%04d.png`, then create a `saccade-frame-map.v1` with
actual presentation timestamps and frame indices from your extractor. The
subtitle clock must share the map's origin. Do not infer timestamps from output
filenames or assume a fixed rate for variable-rate video. Sparse extraction
cannot establish what appeared between samples.

The fixed `--region x,y,width,height` must be inside every frame. Use one text
region per run; align layout changes externally. Build with `--features
text-quality` for pixel measurements and add `ocr` for cached PaddleOCR. Without
pixel support the report retains text/timing findings but cannot pass legibility.
`--ocr` never downloads: missing features, cached models or runtime produce an
explicit OCR skip reason on each frame. An inference error after loading is an
input/runtime error, not a successful skip.

`--sources DIR` imports `saccade-ui-source.v1` OCR files or versioned
`saccade-timed-text-source.v1` wrappers named `INDEX.json`,
where INDEX is the source frame index from the map. Each must bind the exact
encoded image SHA-256 and dimensions. Only OCR kinds (`paddle_ocr`, `ocrs`,
`tesseract_tsv`, `provider_ocr`) are accepted. DOM/accessibility text cannot show
that text appeared in a video raster. Nodes must have bounds entirely in the
region and confidence at least 80; clipped/low-confidence intersecting nodes
make that sample unknown. Nodes outside the region are ignored. Reading order
is geometric top-to-bottom, then left-to-right. Confidence is uncalibrated.
A missing source is unknown. Empty OCR never establishes absence by itself:
the unchanged UI source contract forbids complete OCR coverage. For an explicit
external empty-region annotation, use the timed-text source wrapper:

```json
{
  "schema": "saccade-timed-text-source.v1",
  "source": { "schema": "saccade-ui-source.v1", "kind": "provider_ocr", "complete": false,
              "capture_sha256": "EXACT_IMAGE_HASH", "dimensions": [640, 360],
              "producer": { "adapter": "YOUR_LOCAL_PRODUCER" }, "nodes": [] },
  "declared_empty_region_px": [20, 280, 600, 70]
}
```

The declaration must exactly match `--region`; it is a producer annotation,
not OCR confidence or complete source coverage. Conflicting text makes it
unknown. Use `null` when no absence annotation exists. Cached PaddleOCR emits
no such annotation, so empty detections cannot establish a missing cue.

SRT uses `HH:MM:SS,mmm`; WebVTT accepts `HH:MM:SS.mmm` or `MM:SS.mmm`.
BOM, CRLF, cue identifiers, multiline text, WebVTT NOTE blocks and standard
position settings are supported. Position settings do not change the declared
region. Start times must be nondecreasing; overlapping cues are retained.
Literal ampersands and comparison symbols are preserved. Rich markup/entities,
STYLE/REGION blocks, timestamp maps and header metadata
are explicitly rejected. Supply externally converted plain text for these
inputs; time origin conversion stays visible in your producer records.

The report is `saccade-timed-text.v1`. Every cue includes expected text/window,
matching frame indices, `first_seen_s`, `last_seen_s`, sampled `onset_offset_s`,
window coverage, legibility state, findings and abstention reasons. Every frame
retains its image hash, imported-source hash, observed text or explicit missing
reason, and reused [text-legibility](text-legibility.md) pixel measurements.
Caption matching uses observation-level OCR; the nested legibility baseline-OCR
field remains unavailable because this command compares against cue text.
The map and caption-file hashes bind the inputs. `--out` saves the JSON, escaped
HTML evidence view and artifact manifest.

Matching is exact Unicode after collapsing whitespace. Misspellings are
`text_mismatch`, not silently corrected. Identical repeated text is assigned
to the closest expected time interval within `--search-s` (default 2 seconds).
A unique active cue takes priority over an adjacent cue ending at the same
timestamp. Equal-distance overlaps (including clock roundoff) abstain. Unexpected nonempty text is listed in `extra`
in contiguous sampled runs, retaining frame indices and first/last sightings
under the gap policy. A shifted cue's text stays assigned within the search window;
it is not also counted as extra. Text outside that search horizon is extra.

`--timing-tolerance-s` defaults to 0.3 seconds. Timing and gap comparisons allow
only floating-point roundoff (eight machine epsilons at the presentation-clock
scale); a zero timing tolerance stays strict. Clocks too coarse for the policy
abstain: supply a sequence-relative origin. Findings include `early`, `late`,
`persists_after_end`, `ends_before_end`, `interrupted`, `missing`, `text_mismatch`
and `illegible`. Early/persistence findings have direct sampled sightings.
Lateness requires known samples from expected onset to first sighting; unknown
OCR or sampling gaps do not establish late onset. `last_seen_s` is a sighting,
not disappearance time. `onset_offset_s` is a sampled estimate, not precise
continuous-video synchronization.

`--maximum-gap-s` (default 0.5 seconds) bounds inter-sample gaps and unsampled
edges of each expected window. Missing/mismatch/interruption findings require
known unambiguous OCR throughout that sampled window under this policy. Pixel
legibility thresholds are the text-legibility defaults (contrast 4.5, component
height 8 pixels, sharpness 0.35, stroke width 1 pixel), with the same overrides.
Per-cue legibility aggregates exact matched frames; the full measurements stay
in `observations`. These are pixel proxies, not human readability certification.

Exit 0 means all supplied cues passed text, sampled timing and pixel checks,
with known OCR for all supplied frames. Exit 1 means a measured cue failure or
extra sampled text, including when other evidence is unavailable. Exit 4 means
insufficient evidence with no measured failure. Exit 2 means invalid input or
runtime error. Always inspect cue reasons and coverage as well as the exit code.
Limits: 4 MiB each for captions/map/source files, 4,096 cues or frames, 4 million
cue/frame pairs, and 512 MiB cumulative encoded inputs. Split longer sequences
with an explicit shared time origin. No video decoding, resampling, spelling
inference, network/model download or MCP mirror is added.

Reproduce generated truth and optional cached-OCR availability with:

```sh
python3 scripts/timed-text/fixtures.py --out generated-timed-text --binary /path/to/saccade
```

The generator creates lossless captioned video (including shifted, absent,
misspelt and extra cues), a two-slide presentation recording, an illustrated cinematic recording and
low-contrast text. It decodes each video and asserts exact pixel roundtrip before
binding the observations. Generated truth is an imported-source transport and
matcher proof, not an OCR accuracy claim. Receipts explicitly skip cached OCR
when unavailable. Provenance records generator, font hash/licence and Pillow
version; generated artwork/text uses the repository's MIT OR Apache-2.0 licence.
