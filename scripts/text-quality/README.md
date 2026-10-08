# Generated text-quality proof

Requires Python 3, Pillow, locally installed DejaVu Sans and Noto Sans CJK
Regular (`apt-get install fonts-dejavu-core fonts-noto-cjk` on Debian/Ubuntu).
DejaVu supplies both Latin and Arabic fixtures. No font, image, runtime or model
is downloaded by the generator. The runtime commands
add no dependencies; these fonts are fixture-generation inputs only.
DejaVu uses the Bitstream Vera font license with public-domain DejaVu changes;
Noto Sans CJK uses SIL OFL 1.1. Fonts are not redistributed. `fixtures.json`
records installed font package names and versions, exact file hashes, Pillow
version, fixed noise seed and all expected answers. Generated images are
synthetic and may be regenerated.

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
models. Imported OCR tests cover exact regional disagreement and
stale image identities independently of model availability.

This finite generated proof does not qualify arbitrary scripts/fonts, natural
photographs, OCR model accuracy, shaping, human readability or compliance.

## PR #24 CI fix decisions

The Linux all-features matrix and release-check jobs install both font packages.
The macOS/Windows jobs run default features, so no missing-font skip is added.
Missing fonts remain a hard fixture failure; reducing the multilingual cases or
silently passing was rejected. Package versions are queried from the installed
Debian/Ubuntu packages and recorded alongside font hashes in each manifest.

The CLI reference is regenerated from a release binary with every feature except
`imgtune-avif`, using `--allow-missing-imgtune-avif` because local system dav1d is
unavailable. Lines 5–8 are restored from `origin/main`, then the newly compiled
`text-quality` feature is added to line 8. Keeping the old feature list verbatim
would still fail CI's all-features documentation check. No command help is edited
by hand.

Local validation for this fix: the locked `text-quality,schema` integration gate
passes all three tests, including 25 generated truth cases and schema checks.
Font provenance records `fonts-dejavu-core` version `2.37-8` and `fonts-noto-cjk`
version `1:20230817+repack1-3` on this machine; CI records its own installed
versions. A focused missing-CJK-font check still raises the explicit error.

The regenerated release help body matches this branch's existing reference
exactly. Compared with `origin/main`, 96 command sections are identical, 27
reflect existing source/help differences, and two text-quality sections are new;
no commands are removed. Only header lines 5–8 differ from local AVIF-omitting
generation. Adding AVIF to the reported feature inventory reproduces the final
reference byte for byte; the AVIF codec and full CI release-check were not run.
`python3 scripts/gen-docs.py --check` and public hygiene pass. Actionlint is not
installed, so its conditional gate is skipped.

The perspective-interpolated `capture-2` control retains its original legible
expectation. Rendered contrast measures displayed stroke cores independently
of the source-colour lower-bound policy. Source pixels and thresholds are unchanged.
The nearest-neighbour doubled control remains legible.
