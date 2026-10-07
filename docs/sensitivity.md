# Gate sensitivity

`saccade sensitivity BASELINE --catalogue catalogue.json --config policy.toml --out sensitivity --json`
measures miss rates for a declared, frozen negative-control suite. It runs the existing
compare gate, including per-path thresholds, deciding metrics, regions, masks and
`hotspot_fail` (the local cluster guard). Input images are never written. An optional
`--before-config previous.toml` evaluates the exact same injected pairs under both
policies, showing when a tolerance change hides controls previously detected.

Sensitivity on injected defects is not field recall. A reduced alert count is not
success when known defects become invisible. No tolerance is recommended or changed.

The catalogue uses `saccade-sensitivity-catalogue.v1`. Declare each operation's ID,
location/colour and strictly increasing magnitudes before running the experiment:

```json
{
  "schema": "saccade-sensitivity-catalogue.v1",
  "provenance": "Procedural controls, MIT OR Apache-2.0",
  "injections": [
    {"id": "local", "defect": {"class": "local_edit", "rect": [0.2, 0.3, 0.02, 0.02], "colour": [255, 0, 0]}, "magnitudes": [0.25, 0.5, 1]},
    {"id": "colour", "defect": {"class": "colour_shift", "delta": [24, -16, 8]}, "magnitudes": [0.25, 0.5, 1]},
    {"id": "missing", "defect": {"class": "missing_element", "rect": [0.3, 0.3, 0.1, 0.1], "background": [240, 240, 240]}, "magnitudes": [0.25, 0.5, 1]},
    {"id": "blur", "defect": {"class": "blur"}, "magnitudes": [0.5, 1, 2]},
    {"id": "shift", "defect": {"class": "misalignment"}, "magnitudes": [1, 2, 4]}
  ]
}
```

| Class | Magnitude and construction |
| --- | --- |
| `local_edit` | Opacity (0,1] towards the declared sRGB colour in a fractional rectangle. |
| `colour_shift` | Fraction (0,1] of signed RGB offsets, each -255..255, clamped per channel. |
| `missing_element` | Opacity (0,1] of erasure towards a declared background in a known element rectangle. No automatic element inference. |
| `glyph_edit` | Opacity (0,1] between SHA-256-pinned intact/edited PNG patches at a fractional origin, preserving native patch resolution. |
| `blur` | Gaussian sigma in physical pixels, (0,32]. |
| `misalignment` | Integer horizontal shift in physical pixels, 1..64, smaller than the image width; left edge pixels repeated. |

Glyph operations require `before`, `after`, `before_sha256`, `after_sha256` and
`origin: [x,y]`. Patch paths are relative to the catalogue without traversal.
Both patches must have identical dimensions and fit every baseline. The intact patch
is added to the **control copy**, and the blended edited patch to the candidate copy.
This isolates glyph edits without assuming the user's imagery already contains text;
it does not measure recognition or correctness of existing text. The generated proof
uses minus-sign, decimal-point, currency-space and warning edits from the
[critical-text pack](critical-text.md), whose provenance records the OFL font.

The output retains exact source bytes under `sources/`, normalized lossless PNG pairs
under `trials/`, control/candidate compare reports, catalogue and configuration files,
and `saccade-sensitivity.v2.json` (successor of the unlinked v1 contract). Original
relative names determine overrides and selection even when sources are JPEG.
No JPEG re-encoding noise is added to trials. Supported inputs are bounded 8-bit SDR
rasters and enabled single-page documents; unsupported native-depth/HDR images fail.
Sidecar-dependent identity, arm, buffer and effect gates are refused explicitly.

Summaries pool operations by **class, declared magnitude and policy**. Miss rate is
`missed / (detected + missed)` and is null if there are no eligible observations.
A detection requires a changed pair, a passing unmodified control, and a failing
candidate image verdict. No-op injections, excluded entries and failed evaluation
remain visible as `ineffective`, `excluded` and `unavailable`, outside the denominator.
The experiment exits 4 if any of those occur; exits 1 for a complete experiment with
configured-policy misses; exits 0 only when every configured trial is detected.
Input/configuration failures exit 2 with the existing typed error transport.

`smallest_detected_magnitude` is the smallest **tested** value detecting at least one
eligible injection in that class under that policy. It is null if none was detected.
It is not an all-images guarantee, an interpolated threshold, or proof of monotonicity;
inspect the full magnitude sweep and individual compare artifacts.

Bounds: 64 images, 32 operations, 16 magnitudes per operation, 2,048 trials and 256
million processed source pixels per experiment. Reduce the set or catalogue on refusal.
All output must be empty and outside the baseline, policies and patch inputs.
If supplied, global `--report-index` must name `OUT/reports/index.jsonl`; it cannot
write into baselines or collide with trial images.

Reproduce the two generated sets with the same command and no presets:

```sh
python3 scripts/sensitivity/fixtures.py --out samples/sensitivity --binary saccade --receipts sensitivity-proof
```

The generator records provenance/licences and asserts rates, discrete minima,
misses under a permissive previous policy, detections in all six classes under the
configured policy, retained-copy hashes and unchanged source hashes. These generated
proofs do not qualify arbitrary field data. MCP transport is a recorded follow-up.
