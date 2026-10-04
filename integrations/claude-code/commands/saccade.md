---
description: Measure a visual change and request human review when needed.
argument-hint: BASELINE_DIR CAPTURE_DIR
---
Compare $ARGUMENTS with the installed binary using
`saccade compare BASE CANDIDATE --out saccade-report --json`; choose
`saccade prove identity` for exact equality or `saccade prove performance`
for repeated performance captures. Read validity, performance comparability,
local-change count, limits, totals and next actions. Inspect the worst entry
with `saccade inspect evidence`. Preview `saccade review` locally, prepare
a closed request with `saccade review request`, and use `saccade review ask`
when unresolved. `saccade review propose` records advice. Never approve a
baseline without explicit human authorization. Respect egress and call budgets.
Exit 1 is a failed measurement gate; exit 2 needs investigation.
