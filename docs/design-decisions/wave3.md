# Wave 3 decisions

The lane follows SPEC-wave3.md and ALGORITHM-RESEARCH-2026-10-05.md §§12–15,
with workflows A/B/C from the roadmap and the localized/geometry/motion inputs
from IDEAS-FLESHED. Schemas and public report fields are additive; raw evidence
retains its authority. Synthetic fixtures qualify bounded behaviours only.

## Item 1: one brand review

The front door is `saccade review brand`, with one source-bound packet. The
review needs producer colour/DOM/layout facts; screenshot clustering cannot
supply font applicability or glyph ink bounds. External profile bytes are pinned,
and builtin profiles record generated-profile hashes. The supported ICC scope
is RGB matrix/shaper, relative colorimetric with Bradford/ICC D50 PCS adaptation
to D65. Reject unsupported intents and profile classes rather than substitute
an interpretation. Absolute BT.2020 PQ is a separate explicit ΔE-ITP seam.

moxcms 0.7.11 is a direct, exact-pinned dependency (BSD-3-Clause OR Apache-2.0,
Rust minimum 1.85), already resolved transitively by image. Its published
manifest and licence files were inspected. This pure Rust sample transform is
small and available without heavy optional runtimes. No baseline MSRV change.
Sharma numerical pairs are reference data, not copied research-only code.
Machado numerical tables were transcribed from MIT DaltonLens-Python; its full
notice is retained in THIRD_PARTY.md. Our interpolation uses published severity
steps, not a linear blend of the severity-one endpoint.

APCA is explicitly unavailable, labelled WCAG 3 draft, because the research's
reviewed apca-w3 terms are restricted. Implementing its arithmetic would not
resolve that licence boundary. This residual is not a WCAG 2.x blocker.

## Item 2: one UI/agent change packet

`review ui` joins source text/layout/order findings with the existing frozen
localized inside/boundary/complement measurement. Playwright reporter metadata
is additive and capture-bound for both sides; ingest preserves it in named
sidecars and mapping entries. Source IDs and exact strings are authoritative
producer assertions. Common-node semantic order is compared independently of
pixels and geometric reflow; incomplete scope cannot prove a missing disclosure.

The one OCR fallback is external Tesseract behind the optional CLI `ocr` feature.
The engine and official traineddata are Apache-2.0, with separate local native
runtime dependencies. No new Cargo dependencies. Pin executable and each model,
stage checked models in isolation, use fixed PSM, bound process time/TSV size,
and retain confidence without promotion to proof. A stale supplied DOM/AX record
is rejected rather than replaced. Exact version output and executable digest
are runtime pins; linked-library versions are recorded, but individual library
binary hashes are not established. No runtime/model is downloaded. Tesseract is absent locally;
constructed TSV/price/identifier tests qualify parsing and uncertainty, not
recognizer accuracy. Live small/rotated/multilingual text qualification remains
an explicit deployment residual. Geometric word-slot IDs are deliberately
labelled uncertain rather than inventing semantic OCR node identity.

## Item 3: paired robust performance statistics

A candidate-side optional saccade-perf-pairs.json binds the complete fixed
acquisition plan and run pairs to both exact aggregate performance files. This
avoids changing historical measurement inputs or pretending a frame spread is
an independent-run sample. Validate every planned ID/order/count, both AB/BA
orders, run-level independence declaration, plan hash and prespecified analysis.
No acquisition or opportunistic stopping. Freezing/independence are producer
assertions; Saccade cannot establish their chronology itself.

Use an independent native paired HL implementation: median of Walsh averages
of within-pair differences, including i=j. Paired log-ratios give a separate
relative effect. Seeded whole-pair percentile bootstrap uses SplitMix64 and
linear empirical quantiles. Percentile was chosen for transparent reproducible
behaviour; BCa and dependent/block sampling remain unqualified. Degenerate
bootstrap distributions do not establish a new speedup. Independent NumPy
reference values and constructed symmetric null/alternative coverage are tested.
No new dependencies or copied reference code; no timing measurements.

PerfDiff's robust_effect is additive. Existing capture/noise checks still gate
verdicts, and the complete interval must resolve the noise threshold. Invalid
supplied paired evidence rejects performance qualification. Raw aggregate
measurements and term/counter diagnostics remain present; robust inference is
for frame timing only. Historical absence does not manufacture uncertainty or
rewrite old verdict rules. It remains explicit in the user-facing limits.
