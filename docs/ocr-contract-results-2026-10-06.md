# OCR contract results — 2026-10-06

Coordinator-reviewed generated contracts; CPU Rust PP-OCRv5 mobile/Latin, ONNX Runtime 1.22.
All original 27 images, expectations and thresholds are unchanged. The four required
groups add 36 cases; nine additional numeric cases cover U+202F narrow no-break spaces
without replacing the review’s quoted ASCII-space phrase. CER ≤ 0.02, WER ≤ 0.10
and exact required Unicode strings were frozen before inference. No retuning followed.

**Full gate: FAIL (exit 1). OCR: 46/72 PASS, 26/72 FAIL.**
Accent controls: 54/72 rejected; 18 numeric no-ops remain FAIL. Additional numeric
format controls: 18/18 rejected. Controls derive from truth; OCR errors cannot
manufacture a negative-control rejection.

| Contract group | OCR PASS/total | Accent controls rejected/total | Gate |
| --- | ---: | ---: | --- |
| fr | 8/9 | 9/9 | FAIL |
| de | 9/9 | 9/9 | PASS |
| es | 9/9 | 9/9 | PASS |
| fr-uppercase | 9/9 | 9/9 | PASS |
| fr-rare-lowercase | 2/9 | 9/9 | FAIL |
| fr-ligatures | 9/9 | 9/9 | PASS |
| fr-numbers | 0/18 | 0/18 | FAIL |

## Known limitations retained in the gate

- Original French DejaVu Serif/48: `cœur` → `cæur`; CER 1/39, WER 1/8, exact `cœur` absent. The linear resize and numeric tolerance fixes do not resolve this context. The separate ligature phrase passes 9/9; that does not clear the original failure.
- Rarer lowercase: DejaVu Serif/40, DejaVu Sans/32/40/48 and Liberation Sans/40 read `aujourd’hui` with an ASCII apostrophe (CER 1/71, WER 1/9). DejaVu Serif/48 and Liberation Sans/48 also change `l’été` to `l'été` (CER 2/71, WER 2/9). Only DejaVu Serif/32 and Liberation Sans/32 pass. The accented letters themselves were retained; literal punctuation and WER still fail.
- French numbers: every quoted and thin-space case fails. DejaVu fonts replace both en dashes with hyphens. Liberation Sans omits dashes, and the quoted 32 px case also returns an em dash. All thin-space cases replace or omit U+202F; some omit the space before `€`. Numeric digits, comma decimal and `3e` remain readable in these cases, which does not clear the exact-format contract.
- Numeric text contains no accented letters. Pure accent stripping leaves truth unchanged, so its negative control cannot be rejected. These 18 control failures remain explicit, alongside independently rejected format-stripping controls. No accented word was added to alter the quoted expectation.

## Other validation

Fmt, minimal check, strict OCR/provider/MCP clippy, targeted core tests (29 pass; four unrelated heavy ignores),
CLI fixtures (three pass), generation, frozen-contract byte match, real CLI/media/inspection inference (one pass),
docs/schema and genericity all PASS. The centre-sampled/no-antialias resize and numeric-roundoff/CTC regressions PASS.
No GPU or live provider calls. Generated-case results establish neither general OCR accuracy nor OpenCV/export parity.
One build at a time, `-j 4`, owned target; paused under disk pressure below 25 GB, then resumed.

## Every case

Expected text, PNG/font/licence hashes and frozen controls are in
[the contract file](../scripts/models/ocr-accent-contracts.json). Fractions below retain raw edit counts.
`NO-OP FAIL` means unchanged accent control; numeric format controls all PASS. Literal observed
spaces/punctuation are preserved inside code cells; `\u202f` denotes U+202F if observed.

| Case | CER (edits/ref) | WER (edits/ref) | OCR | Accent control | Observed text |
| --- | --- | --- | --- | --- | --- |
| `fr-DejaVuSerif-32.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSerif-32.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSerif-32.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSerif-32.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSerif-32.png` | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd’hui` |
| `fr-ligatures-DejaVuSerif-32.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSerif-32.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSerif-32.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 € - 12 h 30 - 3e` |
| `fr-DejaVuSerif-40.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSerif-40.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSerif-40.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSerif-40.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSerif-40.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-DejaVuSerif-40.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSerif-40.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSerif-40.png` | 0.24000 (6/25) | 0.44444 (4/9) | FAIL | NO-OP FAIL | `1 234,56€ - 12 h 30 - 3e` |
| `fr-DejaVuSerif-48.png` | 0.02564 (1/39) | 0.12500 (1/8) | FAIL | PASS | `café très fête à garçon hôpital où cæur` |
| `de-DejaVuSerif-48.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSerif-48.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSerif-48.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSerif-48.png` | 0.02817 (2/71) | 0.22222 (2/9) | FAIL | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l'été, aujourd'hui` |
| `fr-ligatures-DejaVuSerif-48.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSerif-48.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSerif-48.png` | 0.24000 (6/25) | 0.44444 (4/9) | FAIL | NO-OP FAIL | `1 234,56€ - 12 h 30 - 3e` |
| `fr-DejaVuSans-32.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSans-32.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSans-32.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSans-32.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSans-32.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-DejaVuSans-32.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSans-32.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSans-32.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 € - 12 h 30 - 3e` |
| `fr-DejaVuSans-40.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSans-40.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSans-40.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSans-40.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSans-40.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-DejaVuSans-40.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSans-40.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSans-40.png` | 0.24000 (6/25) | 0.44444 (4/9) | FAIL | NO-OP FAIL | `1 234,56€ - 12 h 30 - 3e` |
| `fr-DejaVuSans-48.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-DejaVuSans-48.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-DejaVuSans-48.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-DejaVuSans-48.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-DejaVuSans-48.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-DejaVuSans-48.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-DejaVuSans-48.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 € - 12 h 30 - 3e` |
| `fr-numbers-thin-space-DejaVuSans-48.png` | 0.24000 (6/25) | 0.44444 (4/9) | FAIL | NO-OP FAIL | `1 234,56€ - 12 h 30 - 3e` |
| `fr-LiberationSans-Regular-32.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-LiberationSans-Regular-32.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-LiberationSans-Regular-32.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-LiberationSans-Regular-32.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-LiberationSans-Regular-32.png` | 0.00000 (0/71) | 0.00000 (0/9) | PASS | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd’hui` |
| `fr-ligatures-LiberationSans-Regular-32.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-LiberationSans-Regular-32.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 €  12 h 30 — 3e` |
| `fr-numbers-thin-space-LiberationSans-Regular-32.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 €  12 h 30  3e` |
| `fr-LiberationSans-Regular-40.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-LiberationSans-Regular-40.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-LiberationSans-Regular-40.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-LiberationSans-Regular-40.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-LiberationSans-Regular-40.png` | 0.01408 (1/71) | 0.11111 (1/9) | FAIL | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l’été, aujourd'hui` |
| `fr-ligatures-LiberationSans-Regular-40.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-LiberationSans-Regular-40.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 €  12 h 30  3e` |
| `fr-numbers-thin-space-LiberationSans-Regular-40.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 €  12 h 30  3e` |
| `fr-LiberationSans-Regular-48.png` | 0.00000 (0/39) | 0.00000 (0/8) | PASS | PASS | `café très fête à garçon hôpital où cœur` |
| `de-LiberationSans-Regular-48.png` | 0.00000 (0/23) | 0.00000 (0/4) | PASS | PASS | `Grüße für schöne Straße` |
| `es-LiberationSans-Regular-48.png` | 0.00000 (0/30) | 0.00000 (0/7) | PASS | PASS | `El señor tomó café y está aquí` |
| `fr-uppercase-LiberationSans-Regular-48.png` | 0.00000 (0/40) | 0.00000 (0/10) | PASS | PASS | `ÉTÉ À PARIS : ÇA COÛTE CHER, ÎLE DE NOËL` |
| `fr-rare-lowercase-LiberationSans-Regular-48.png` | 0.02817 (2/71) | 0.22222 (2/9) | FAIL | PASS | `maïs, naïve, âme, dîner, piqûre, Noël, «guillemets», l'été, aujourd'hui` |
| `fr-ligatures-LiberationSans-Regular-48.png` | 0.00000 (0/26) | 0.00000 (0/5) | PASS | PASS | `cœur, œuvre, sœur, ex æquo` |
| `fr-numbers-LiberationSans-Regular-48.png` | 0.08000 (2/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 €  12 h 30  3e` |
| `fr-numbers-thin-space-LiberationSans-Regular-48.png` | 0.24000 (6/25) | 0.22222 (2/9) | FAIL | NO-OP FAIL | `1 234,56 €  12 h 30  3e` |

## Evidence identity

- Pre-commit source HEAD: `00880c48f1cc38294dda9f6e6bf34f7a3a7baf41`; compiled-source fingerprint: `ebfd45a8a1ed40b9d0ebf6f4209ced55c9619add7e6c23fea13c39618f593808`.
- Frozen contract SHA-256: `99ca5ecfe32e9f884aed74aade131fbcf5ae0a8c206a98b592fe1cc6548b75e9`.
- Full result receipt SHA-256: `048d1553c6dceaf48b367f99107e94b86108c56652c96199e1ad45b9cb70d8b3`.
- Inference executable SHA-256: `92e31420da7e69d56b37cf50cd3be91b5cbf0f3b17bd57518f3acfdccf47240e`.
- Full JSON result, gate/resource logs and source/runtime/binary receipts are retained in
  `/mnt/linux-extra/moss-scratch/saccade-ocr/` as `accent-results-resume-2026-10-06.json`,
  `gates-resume-2026-10-06.log`, `resources-resume-2026-10-06.log`,
  `source-resume-2026-10-06.json` and `validation-resume-2026-10-06.json`.
  The owned Cargo target is removed after validation; evidence and pinned models remain.
