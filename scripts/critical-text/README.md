# Generated critical glyph-edit fixtures

All images are generated, with no copied imagery or downloads. Generated assets
and the script use the repository's MIT OR Apache-2.0 licence. Liberation Sans
Regular is a locally installed font from `fonts-liberation`, licensed SIL OFL
1.1; the font is not redistributed. The generator checks the local package
licence and font family and records exact font and licence-evidence SHA-256.
Debian packaging code licences are not font licences.

Regenerate with Python 3, Pillow and the installed OFL package:

```sh
python3 scripts/critical-text/fixtures.py --out testdata/critical-text
```

Use an empty output directory for a new pack. `fixtures.json` freezes generated
file hashes, Pillow version, provenance and expected exits before qualification.
Generation never accepts newly measured gate answers. Different Pillow/font
versions may change raster hashes and require review and qualification.

Check the shipped pack with a stock build:

```sh
python3 scripts/critical-text/fixtures.py --binary /path/to/saccade \
  --receipts /path/to/empty-proof-directory
```

Qualification does not regenerate, fetch or use fonts/models. All 11 cases must
pass an actual FLIP mean gate at 0.02; the critical gate must fail five known
defects (decimal, currency space, minus, tiny warning edit and faded warning)
and pass six unchanged/typography controls. Failing hashes, exit codes, states,
image identities or declared failure reasons stop the run. JSON reports and
`qualification.json` record binary, manifest and report identities.

[Gate contract and limitations](../../docs/critical-text.md).
