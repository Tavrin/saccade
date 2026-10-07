# Generated text-quality proof

Requires Python 3, Pillow, locally installed DejaVu Sans and Noto Sans CJK
Regular. No font, image, runtime or model is downloaded. The runtime commands
add no dependencies; these fonts are fixture-generation inputs only.
DejaVu uses the Bitstream Vera font license with public-domain DejaVu changes;
Noto Sans CJK uses SIL OFL 1.1. Fonts are not redistributed. `fixtures.json`
records their exact file hashes, Pillow version, fixed noise seed and all
expected answers. Generated images are synthetic and may be regenerated.

```sh
cargo build --locked -p saccade --features text-quality
python3 scripts/text-quality/fixtures.py --out generated-text-fixtures \
  --binary target/debug/saccade
```

The qualifier invokes the same `tofu` and `text-legibility` commands on all
capture constructions, asserts known states, exit codes, reasons and candidate
coordinates, and records every result in `qualification.json`. `report-*.json`
contains each full versioned report. The Rust integration test additionally
validates those outputs against the shipped generated schemas.

The negative glyph set includes Latin, Arabic and CJK (including real box-like
characters). Unobserved glyph completeness and ambiguous squares must abstain;
no clean tofu verdict is emitted. Pixel failures and abstentions remain
separate. The cached OCR check prints an explicit `SKIP` reason when optional
OCR is unavailable; it never converts missing OCR into agreement or downloads
models. Imported OCR tests cover exact regional agreement/disagreement and
stale image identities independently of model availability.

This finite generated proof does not qualify arbitrary scripts/fonts, natural
photographs, OCR model accuracy, shaping, human readability or compliance.
