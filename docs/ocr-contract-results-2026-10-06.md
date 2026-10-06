# OCR contract results — 2026-10-06

Coordinator-reviewed generated contracts; CPU Rust PP-OCRv5 mobile/Latin, ONNX Runtime 1.22.
All 72 fixture records, expected texts, thresholds, PNG/font/licence hashes and
truth-derived control texts are unchanged from dbfea24. CER ≤ 0.02, WER ≤ 0.10
and exact required strings still apply.

The coordinator's post-run disposition is a **post-hoc contract-design correction**.
Strict scoring is unchanged. The additional declared **typographic-equivalence**
view folds ’/‘ → ASCII apostrophe, U+202F/U+00A0/U+2009 → normal space, and
en/em dash → hyphen, on observations, expected text and required strings.
It uses the same thresholds. No characters are deleted or whitespace collapsed:
omitted spaces or dashes still count as errors. Expected texts were not edited
to ASCII punctuation.

**Full gate: FAIL (exit 1), solely for accents. Strict: 46/72 PASS.
Typographic-equivalence: 61/72 PASS.** Strict failures remain recorded and
keep the gate failing. All strict rates, required-string checks, observed nodes
and case verdicts exactly match the prior run.

Accent controls: 54/54 applicable controls rejected in both views;
18 numeric controls without accents are **N/A**, not failed. Numeric format
controls: 18/18 rejected in both views. Controls derive from truth.

| Contract group | Strict PASS/total | Folded PASS/total | Accent controls rejected/applicable | N/A |
| --- | ---: | ---: | ---: | ---: |
| fr | 8/9 | 8/9 | 9/9 | 0 |
| de | 9/9 | 9/9 | 9/9 | 0 |
| es | 9/9 | 9/9 | 9/9 | 0 |
| fr-uppercase | 9/9 | 9/9 | 9/9 | 0 |
| fr-rare-lowercase | 2/9 | 9/9 | 9/9 | 0 |
| fr-ligatures | 9/9 | 9/9 | 9/9 | 0 |
| fr-numbers | 0/18 | 8/18 | 0/0 | 18 |

## Known limitations retained in both views

- **œ misread in serif at large sizes:** original French DejaVu Serif/48
  returns `cœur` → `cæur`; CER 1/39, WER 1/8, exact `cœur` absent.
  The separate ligature phrase passes 9/9; it does not clear that failure.
- **Dash omission with some sans fonts:** Liberation Sans omits one or both
  dashes. Folding an observed em dash to hyphen does not repair an omitted dash.
- Four DejaVu thin-space cases (Serif/Sans at 40/48 px) omit the space before
  `€`. Folding U+202F to space does not restore that missing character.
- Curly-apostrophe substitutions and declared space/dash substitutions remain
  strict failures, while their folded scores show equivalence.

## Validation and evidence

Only the requested `SACCADE_OCR_PYTHON=/mnt/linux-extra/saccade-models/venv/bin/python scripts/gates-ocr.sh`
was run. Fmt, minimal check, strict OCR/provider/MCP clippy, targeted core tests,
CLI fixtures, generation, frozen-contract byte match, local inference,
docs/schema and genericity PASS. No GPU or live provider calls.
The run used the designated target with `-j 4`, monitored the 25 GB disk floor,
and initially paused before admission. The target was removed after completion.
Generated-case results establish neither general OCR accuracy nor OpenCV/export parity.

Evidence outside the repository:
- `/mnt/linux-extra/moss-scratch/saccade-ocr/gate-typographic-2026-10-06.log`
- `/mnt/linux-extra/moss-scratch/saccade-ocr/gate-typographic-2026-10-06.json`
- `/mnt/linux-extra/moss-scratch/saccade-ocr/accent-results.json`

Contract SHA-256: `dc1d61420c30bca4636d7a277f889ffa4765491b73b9f948c59b8906d1f2c49f`.
Result JSON SHA-256: `f1d5b8721bb8e9015cc0b0f23568e18c79c1f2b158defac541c71ea114e829c6`.

## Every case

Expected text, PNG/font/licence hashes and control texts are in
[the contract file](../scripts/models/ocr-accent-contracts.json).
Rates retain raw edits/reference denominators. PASS requires both rates and
all required strings to pass. Literal observed spaces/punctuation are preserved;
`\u202f` denotes U+202F if observed. All numeric format controls pass in both views.

| Case | Strict CER | Strict WER | Strict | Folded CER | Folded WER | Folded | Accent control | Observed text |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `fr-DejaVuSerif-32.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSerif-32.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSerif-32.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSerif-32.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSerif-32.png` | 0.00000 (0/71) | 0.00000 (0/9) | PASS | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd’hui` |
| `fr-ligatures-DejaVuSerif-32.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSerif-32.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | 0.00000 (0/25) | 0.00000 (0/9) | PASS | N/A | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSerif-32.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | 0.00000 (0/25) | 0.00000 (0/9) | PASS | N/A | `1 234,56 € - 12 h 30 - 3e` |
| `fr-DejaVuSerif-40.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSerif-40.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSerif-40.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSerif-40.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSerif-40.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-DejaVuSerif-40.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSerif-40.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | 0.00000 (0/25) | 0.00000 (0/9) | PASS | N/A | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSerif-40.png` | 0.24000 (6/25) | 0.44444 (4/9) | FAIL | 0.04000 (1/25) | 0.22222 (2/9) | FAIL | N/A | `1 234,56€ - 12 h 30 - 3e` |
| `fr-DejaVuSerif-48.png` | 0.02564 (1/39) | 0.12500 (1/8) | FAIL | 0.02564 (1/39) | 0.12500 (1/8) | FAIL | PASS | `café très fête à garçon hôpital où cæur` |
| `de-DejaVuSerif-48.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSerif-48.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSerif-48.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSerif-48.png` | 0.02817 (2/71) | 0.22222 (2/9) | FAIL | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l'été, aujourd'hui` |
| `fr-ligatures-DejaVuSerif-48.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSerif-48.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | 0.00000 (0/25) | 0.00000 (0/9) | PASS | N/A | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSerif-48.png` | 0.24000 (6/25) | 0.44444 (4/9) | FAIL | 0.04000 (1/25) | 0.22222 (2/9) | FAIL | N/A | `1 234,56€ - 12 h 30 - 3e` |
| `fr-DejaVuSans-32.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSans-32.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSans-32.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSans-32.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSans-32.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-DejaVuSans-32.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSans-32.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | 0.00000 (0/25) | 0.00000 (0/9) | PASS | N/A | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSans-32.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | 0.00000 (0/25) | 0.00000 (0/9) | PASS | N/A | `1 234,56 € - 12 h 30 - 3e` |
| `fr-DejaVuSans-40.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSans-40.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSans-40.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSans-40.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSans-40.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-DejaVuSans-40.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSans-40.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | 0.00000 (0/25) | 0.00000 (0/9) | PASS | N/A | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSans-40.png` | 0.24000 (6/25) | 0.44444 (4/9) | FAIL | 0.04000 (1/25) | 0.22222 (2/9) | FAIL | N/A | `1 234,56€ - 12 h 30 - 3e` |
| `fr-DejaVuSans-48.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSans-48.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSans-48.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSans-48.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSans-48.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-DejaVuSans-48.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSans-48.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | 0.00000 (0/25) | 0.00000 (0/9) | PASS | N/A | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSans-48.png` | 0.24000 (6/25) | 0.44444 (4/9) | FAIL | 0.04000 (1/25) | 0.22222 (2/9) | FAIL | N/A | `1 234,56€ - 12 h 30 - 3e` |
| `fr-LiberationSans-Regular-32.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-LiberationSans-Regular-32.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-LiberationSans-Regular-32.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-LiberationSans-Regular-32.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-LiberationSans-Regular-32.png` | 0.00000 (0/71) | 0.00000 (0/9) | PASS | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd’hui` |
| `fr-ligatures-LiberationSans-Regular-32.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-LiberationSans-Regular-32.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | 0.04000 (1/25) | 0.11111 (1/9) | FAIL | N/A | `1 234,56 €  12 h 30 — 3e` |
| `fr-numbers-thin-space-LiberationSans-Regular-32.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | N/A | `1 234,56 €  12 h 30  3e` |
| `fr-LiberationSans-Regular-40.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-LiberationSans-Regular-40.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-LiberationSans-Regular-40.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-LiberationSans-Regular-40.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-LiberationSans-Regular-40.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-LiberationSans-Regular-40.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-LiberationSans-Regular-40.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | N/A | `1 234,56 €  12 h 30  3e` |
| `fr-numbers-thin-space-LiberationSans-Regular-40.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | N/A | `1 234,56 €  12 h 30  3e` |
| `fr-LiberationSans-Regular-48.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-LiberationSans-Regular-48.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-LiberationSans-Regular-48.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-LiberationSans-Regular-48.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-LiberationSans-Regular-48.png` | 0.02817 (2/71) | 0.22222 (2/9) | FAIL | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l'été, aujourd'hui` |
| `fr-ligatures-LiberationSans-Regular-48.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-LiberationSans-Regular-48.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | N/A | `1 234,56 €  12 h 30  3e` |
| `fr-numbers-thin-space-LiberationSans-Regular-48.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | N/A | `1 234,56 €  12 h 30  3e` |
