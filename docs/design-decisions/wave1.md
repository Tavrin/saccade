# Wave 1 decisions

## Item 0: shared evidence contracts

Read FEATURE-RESEARCH-2026-10-05 sections 2, 5, 6 and build order, and
ASTRA-VISION-2026-10-05 candidate 22 before implementation. This lane follows
the lane's explicit order: contracts, exclusion audit, onset, geometry, motion.

Add `evidence::analysis` alongside existing evidence contracts. Availability has
explicit available, unknown, unsupported, excluded and rejected states. Historical
absence defaults to unknown, never available. Measurements carry implementation
revision, package version, code licence, resource identities and execution settings.
Missing resource hashes and licences remain null. No model or runtime download is
introduced. Existing schema identifiers and exit authorities remain unchanged;
new report fields will be optional. Unknown fields remain reader errors, preserving
the existing version-skew handling required by DESIGN-1.0 section 17.

No new dependencies. These records provide provenance, not reproducibility proof
across arbitrary hardware, nor capture validity or approval authority.

## Item 1: exclusion audit

Keep the pre-mask FLIP map authoritative for excluded-error accounting. Record
resolved mask runs and their byte hash; overlap counts once. Evaluate a copy of
the region policy without masks for the counterfactual, leaving the original
entry and verdict intact. Exact identity ignores thresholds in both paths.
Store capture-selection exclusions before filtering names. Absent expected-capture
manifests mean coverage beyond supplied names is unknown. Buffer/HDR/SDR scope is
stated explicitly, and historical reports do not manufacture audit evidence.
Expose JSON, portable HTML/Markdown and `inspect exclusions`. No new dependencies.
