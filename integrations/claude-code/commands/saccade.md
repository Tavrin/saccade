---
description: Measure a visual change and request human review when needed.
argument-hint: BASELINE_DIR CAPTURE_DIR
---
Compare $ARGUMENTS with `saccade compare ... --out saccade-report --json` or
`saccade_measure` with operation `compare`. Read validity, limits, totals and
next actions. Inspect the worst entry with `saccade inspect evidence` and
`saccade inspect export --format png`. Preview `saccade review` locally, prepare
a closed request with `saccade review request`, and use `saccade review ask`
when unresolved. `saccade review propose` records advice. Never approve a
baseline without explicit human authorization. Respect egress and call budgets.
Exit 1 is a failed measurement gate; exit 2 needs investigation.
