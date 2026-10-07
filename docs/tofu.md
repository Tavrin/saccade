# Missing-glyph triage

Build with `--features text-quality`. `saccade tofu IMAGE --json` emits
`saccade-tofu.v1`, with encoded image SHA-256, capture dimensions, candidate
rectangles `[x,y,width,height]`, shape scores and explicit evidence states.
`--out DIR` also saves the full report. The score is heuristic confidence,
not a calibrated probability or a font diagnosis.

```sh
saccade tofu capture.png --expected-text 'Sample text' --json
saccade tofu capture.png --mask text-mask.png --ocr --json
```

The mask includes nonzero red-channel pixels and must match capture dimensions.
The expected string is declared context, not evidence that glyphs exist.
Hollow tall rectangles and filled replacement diamonds are supported pixel
shapes. Square boxes can also be legitimate glyphs and remain ambiguous.
Components are eight-connected; foreground comes from actual opaque sRGB
pixels against a dominant uniform background. At most 16 million pixels,
4096 components and 64 MiB of encoded input are analyzed. Transparency, mixed
backgrounds, empty masks and component-limit exhaustion explicitly abstain.

`state: candidates` exits 1 and requests review. No detected shape yields
`insufficient_evidence`, exit 4: it never certifies complete glyph coverage.
Usage, input and feature errors exit 2. Default builds register the command but
require the `text-quality` feature for execution. `--json` emits the full report
without requiring an output directory; text mode emits its schema and state.

Optional `--ocr` reuses the cached PaddleOCR adapter in an `ocr` build without
provisioning or downloads. Missing runtime/models appear as `ocr.state:
unavailable` with the reason. `--source FILE` instead accepts image-bound
`saccade-ui-source.v1` OCR observations; malformed, stale or non-OCR sources
fail. Replacement-character OCR is separate corroboration, never the sole
pixel detection signal. The installed model does not establish support for
all scripts. Public fixture proof includes correctly rendered Latin, CJK and
Arabic text; it does not qualify arbitrary fonts, photos or scripts.
