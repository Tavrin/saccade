# Critical text gates

`critical-text` requires every declared region to retain an exact accepted string
and satisfy local pixel-legibility thresholds. It catches small critical edits
that can pass a full-image FLIP mean gate. It works with image-bound source
exports on the stock build; no extra dependency or model is required.

```sh
saccade critical-text baseline.png candidate.png --policy policy.json \
  --a-source baseline-source.json --b-source candidate-source.json --json
```

A reusable policy freezes physical capture dimensions, required rectangles,
exact accepted strings and pixel thresholds:

```json
{
  "schema": "saccade-critical-text-policy.v1",
  "dimensions": [900, 700],
  "minimum_ocr_confidence": 80,
  "regions": [{
    "id": "amount",
    "rect_px": [94, 544, 160, 42],
    "accepted_text": ["12.50 €", "12.50 €"],
    "legibility": {
      "minimum_contrast": 4.5,
      "minimum_x_height_px": 3,
      "minimum_sharpness": 0.35,
      "minimum_stroke_px": 1
    }
  }]
}
```

The second accepted string contains U+00A0; a missing space is still a mismatch.
Nothing folds or removes whitespace, punctuation, accents, signs or currency.
Alternatives must be explicit before checking; accepting a variant is a policy
choice, never inferred scientific or legal equivalence. The body-height minimum
in this example is a pixel component proxy for a numeric string with small
marks; choose thresholds for the actual output scale, including warning text.

Use `saccade-ui-source.v1` files as described in [text](text.md). Each critical
region must contain one complete text unit with a wholly contained box. Do not
split currency spacing across nodes: the gate never synthesizes inter-node
spaces. Source exports must declare complete scope. Missing or crossing boxes,
multiple units, incomplete scope, and missing text evidence cannot pass. Empty
complete source scope fails the required string; absent OCR remains unknown.
Captures must have identical dimensions and match the policy. Regions, nodes,
strings, file bytes and total sampled pixels are bounded. Stale image bindings,
invalid policies and unknown JSON fields are execution errors.

`--ocr` selects the existing cached local OCR adapter instead of source files.
It requires the `ocr` build plus provisioned runtime/models; execution never
downloads. PP-OCRv5 observes lines, so declare one line per region. Word-based
OCR needs one region per observed word and cannot establish spacing between
words. Low or absent confidence cannot pass. There are no live provider calls.

The `saccade-critical-text.v1` report retains the exact policy, policy/image/source
hashes, source/OCR provenance, every string and pixel result, Unicode CER/WER and
reasons. Accepted typography can have nonzero CER and still pass the declared
policy. Baseline text and baseline pixel measurements must qualify first. Every
region is required; an observed candidate failure dominates unknown evidence
elsewhere. `--out DIR` writes the report to an empty directory outside inputs;
`--json` always prints the full report with its content-addressed identity.

Exit 0 means every critical region passes; exit 1 means a critical string or
pixel threshold fails; exit 4 means insufficient evidence; exit 2 means invalid,
missing or unavailable input/runtime. CI must treat every nonzero exit as a
non-pass. A full-image mean pass grants no override.

Source text is a producer fact, not proof of visible glyph identity: capture
hashes detect stale sources but cannot certify truthful exports. Pixel contrast,
sharpness, stroke and component height are heuristics on opaque SDR regions;
OCR confidence is uncalibrated. Pass establishes only this policy on these
captures, not human readability, font coverage, correct scientific data,
legal sufficiency or publication approval. Text is inert data.

## Generated proof pack

[`testdata/critical-text`](../testdata/critical-text) includes app decimal and
currency-space edits, a scientific figure with a lost minus sign, and a document
page with a tiny warning edit. A faded warning retains the source text; its thin-stroke contrast is a lower
bound below target, so it abstains. The unchanged tiny warning also abstains;
its baseline legibility is not established by the component-local estimator. Unchanged captures and explicitly accepted no-break-space and
hyphen variants are controls. Fonts are Liberation Sans under SIL OFL 1.1;
fixture and generator provenance, font/licence hashes, Pillow version, exact
asset hashes and declared expected exits are in `fixtures.json`.

```sh
python3 scripts/critical-text/fixtures.py --binary /path/to/saccade \
  --receipts /path/to/empty-proof-directory
```

This checks shipped assets without font or model access. The qualifier asserts
actual mean-only FLIP compare exit 0 at threshold 0.02 on every case, critical
exit 1 on four known defects, exit 0 on five controls and exit 4 on the two
thin-warning cases. The faded edit still must have nonzero measured FLIP mean;
an abstention is a non-pass, not a qualified pixel-contrast failure. Changed fixture hashes,
absent edits, unexpected states and wrong reasons fail. Report receipts include
binary and fixture identities. Regeneration needs Pillow and the installed OFL
font; see [fixture instructions](../scripts/critical-text/README.md).

The finite generated pack qualifies these examples only. OCR model accuracy,
source-export fidelity on independent captures and live providers are unverified.
MCP mirroring is follow-up work outside this lane. For runnable stock examples,
see the [critical text guide](guides/critical-text.md).
