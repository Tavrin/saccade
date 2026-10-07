# Print raster comparison

Build `cargo build -p saccade --features print`. `doctor --json` and
`capabilities --json` list `print`. The first-party `saccade-print` library depends
on core; core has no dependency on print. Default CLI builds do not include the CMM.
Little CMS is vendored, so CI needs its existing C compiler, not a new system package.

```sh
saccade print baseline.tif candidate.tif --out evidence \
  --tac-limit 300 --dpi 300 --output-profile target.icc --json
```

Each side uses its embedded CMYK ICC profile. `--input-profile input.icc` explicitly
overrides both sides; the report records both embedded and selected profile hashes.
No default press profile or RGB-to-CMYK conversion is guessed. Missing/malformed
profiles and RGB-only images exit 2 with stable `print_profile` or `not_print_input`
codes. Measurement completion exits 0 and means `measured`, not approved. TAC and
colour differences are evidence; this command does not assign a universal tolerance.

Supported inputs:

- CMYK TIFF: unsigned 8/16-bit samples; CMYK+alpha: 8-bit, or decoded 16-bit when
  supported by the TIFF container reader. Associated and straight alpha must be
  declared. One page, top-left orientation, process CMYK inks only.
- 8-bit Adobe CMYK/YCCK JPEG, including chunked embedded ICC profiles. JPEG has no alpha.
- PDF raster: one page, one full-page unrotated 8-bit CMYK image, inline resources,
  a direct content stream containing `q cm Do Q`, optional matching DeviceGray
  soft mask, and ICCBased or DeviceCMYK with explicit input ICC. Streams may be
  uncompressed or FlateDecode without DecodeParms; arbitrary vector pages, annotations, cropping, Decode arrays,
  encrypted documents and unsupported transforms are refused. For general PDF
  artwork, export a CMYK TIFF with the intended rasterizer and embedded profile.
  This adapter preserves the embedded raster; it does not render PDF vectors.

Encoded input limit: 128 MiB per file; decoded input limit: 16 million pixels;
ICC limit: 4 MiB; PDF content stream: 64 KiB; decoded PDF object/xref stream: 4 MiB. Dimensions must match. No automatic alignment or resampling.
Alpha is unassociated when necessary, then ink amounts are multiplied by alpha
onto unprinted paper. TAC, colour conversion and separation maps all use that
same effective coverage. This is an explicit raster policy, not overprint simulation.

Comparison uses relative colorimetric CIELAB D50, no black-point compensation,
and CIEDE2000 with kL=kC=kH=1. Percentiles use nearest rank. `delta-e2000.png`
is a grayscale heatmap, 0 = no change, 255 = ΔE 10 or greater.
`separation-{c,m,y,k}.png` maps absolute ink changes: 255 = 100 percentage points.
`{reference,candidate}-tac-over.png` marks TAC strictly above the declared percent
limit. All maps share the original raster pixel coordinates, with top-left origin.

`--output-profile` adds `{reference,candidate}-out-of-gamut.png` and the Little CMS relative-colorimetric proofing gamut-check share. Two isolated
CMM contexts use different alarm colours, so legitimate output colours cannot be
confused with an alarm. This is an ICC-model gamut test, not a physical press
measurement or certification. Without a target,
its state is `not_requested`.

Four-connected components with C/M/Y each ≥5% and K≥50%, at least two pixels,
and height ≤`--small-text-points` (default 12) are reported as registration-sensitive
small four-colour marks. The caller must declare DPI. At most 10,000 candidates
are returned, with a truncation flag. These are rich-black/text-like candidates;
OCR, semantic text recognition and proofing UI are outside this feature.

`saccade-print.v1.json` has `report_id` and `source_refs` from core report_links;
encoded inputs, selected profiles and policy are bound into the identity.
Retrieve its contract with `saccade schema get saccade-print.v1`.
The schema lives in `crates/saccade-core/schemas/` and is shared by the extension and CLI catalogue. MCP exposes the same
operation as `saccade_general`, `operation: print_compare`, using `reference`,
`capture`, `out`, required `tac_limit` and `dpi`, and optional `input_profile`,
`output_profile`, `small_text_points`. Existing MCP root/output policies apply.

## Reproduce both domains

```sh
cargo run -p saccade-print --example fixtures -- /tmp/print-inputs
for case in packaging-label magazine-page; do
  saccade print /tmp/print-inputs/$case.tif /tmp/print-inputs/$case-shift.tif \
    --out /tmp/print-evidence/$case --tac-limit 300 --dpi 300 \
    --output-profile /tmp/print-inputs/synthetic.icc --json
done
```

The original generated ICC is an analytic test device, MIT OR Apache-2.0;
no commercial or third-party profile is bundled. Both domains must produce
ΔE max in 2.4..2.5, exactly 50 TAC-over-limit pixels at x=8..17, y=35..39,
nonempty small-mark candidates, and ΔE 0 for identity comparisons.
The tests also cover alpha, 16-bit TIFF, JPEG polarity/embedded ICC, raster PDF
identity, vector-content refusal, missing profiles and RGB refusal. The JPEG
fixture was generated by Pillow as a constant 8×8 CMYK image (64,32,16,128),
quality 100; its ICC APP2 is generated by the test.
