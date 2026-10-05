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
