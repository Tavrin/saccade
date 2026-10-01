---
description: Inspect a visual change with saccade and ask for a human decision when needed.
argument-hint: BASELINE_DIR CAPTURE_DIR
---
Compare $ARGUMENTS with `saccade compare ... --out saccade-report --json` or `saccade_compare` MCP. Read verdict, totals and failing hotspots. Inspect `saccade explain` strips and a `saccade snapshot` for the worst entry. Summarize the visible change with evidence paths. If intent is unclear, post a closed-answer question via `saccade_ask_human` or `saccade ask` and link the exact view. Record proposals with `decide`; never auto-approve or update a baseline without explicit human authorization. Exit 1 is a regression, not a tool failure; exit 2 needs investigation.
