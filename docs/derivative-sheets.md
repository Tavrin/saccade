# Derivative review sheets

`derivative-sheet` checks declared sizes, crops and delivered renditions together.
It writes `index.html`, a labelled `contact-sheet.png`, one display-size PNG per
available derivative, original-resolution text crops and
`saccade-derivative-sheet.v1.json`. The existing output manifest hashes all assets
and registers the report and its JSON rows through the report index.

```sh
saccade derivative-sheet source.png derivatives.json --out review --json
saccade manifest verify review --json
```

Outputs must be new empty directories outside the inputs. Input paths in a
declaration resolve relative to that declaration. The bounded input contract is
`saccade-derivatives.v1`:

```json
{
  "schema": "saccade-derivatives.v1",
  "subjects": [{"label": "protected subject", "rect_px": [20, 30, 80, 100]}],
  "text_regions": [{"label": "small label", "rect_px": [120, 200, 160, 40]}],
  "derivatives": [
    {"id": "square", "display_size": [256, 256], "crop_px": [0, 0, 320, 320]},
    {"id": "small", "display_size": [80, 60]},
    {"id": "delivered", "display_size": [160, 120], "path": "delivered.jpg"}
  ]
}
```

All boxes are original source pixels `[x,y,width,height]`; omitted `crop_px`
means the full source. Omitted `path` generates the exact crop resized with
Lanczos3. A delivered file is resized to its declared display pixels for text
measurement; its declared crop describes source geometry. Nonmatching aspect
ratios are resized independently on each axis, with no additional hidden crop.
The report records source/declaration hashes, delivered hashes and dimensions,
preview hashes, effective crops, contact-sheet placement and every input row.
Missing/unreadable renditions remain failed rows with a reason and a red PNG slot.

The shared model configuration supplies the cached `--detector` (default YuNet).
This command never provisions models or calls a provider. Missing features,
models or runtime produce an explicit `faces.state = unavailable`, and manually
protected subjects still get geometry checks. `--faces-report FILE` accepts an
image-bound face receipt for explicit replay, validates its digest/dimensions
and labels its provenance `replay` with source parity false. An empty detection
is `not_established`; it never certifies safety. Known manual and detected boxes
are combined and projected as a separate annotated HTML view, with each box marked `included`, `cut` or `excluded`.

`crop_safety` describes only those known boxes. `subject_preservation` establishes
retention for generated crops of those boxes. For delivered files it remains
`not_established` even when asserted source geometry retains all boxes: source
coordinates alone cannot establish that the subject survived delivery.

Text uses the existing deterministic text-legibility pixel measures (contrast,
component body-height proxy, sharpness and stroke width), with the same fixed
policy at source and final display sizes. Regions map relative to the declared
crop, rounding outwards. Cut/excluded text fails; no separable glyphs or an
unestablished baseline abstains. Full-resolution source text sits beside each
final-size preview in HTML. Browser zoom changes physical viewing size. The PNG
retains every preview at its declared sampled dimensions, with numbered status
labels corresponding to JSON rows.

OCR is an explicit unavailable section because this operation measures pixel
legibility; use `text` or `critical-text` for observed Unicode correctness. Pixel
thresholds and even OCR success cannot establish human readability. Reviewers
must inspect final-size previews; no subject completeness or publication approval
is inferred.

Exits: **0** all scoped generated geometry/text checks pass; **1** unsafe crops,
illegible text or failed rendition acquisition; **4** completed review with
unestablished evidence; **2** invalid input or an operation that cannot run.
Missing face models alone need not cause exit 4 when declared boxes establish
scoped geometry and declared text passes. No subject/text declarations abstain.

Bounds: 1–64 derivatives; at most 64 subject and 64 text regions; display axes
1–4096; at most 16 million preview and contact-sheet pixels; declarations/face
receipts 1 MiB, encoded raster files 64 MiB and decoded rasters 16 million pixels.
Legacy face/text and manifest schemas remain unchanged; the new report includes
linkage from v1. Discover both contracts with `schema get`.

[Generated product and map proofs](../scripts/derivatives/README.md) record licence
and provenance and assert the injected failures. Model recall on natural photos,
human readability and an MCP mirror remain separate follow-up work.
