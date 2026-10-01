---
description: Inspect a visual change with flipdiff and ask for a human decision when needed.
argument-hint: BASELINE_DIR CAPTURE_DIR
---
Compare $ARGUMENTS with `flipdiff compare ... --out flipdiff-report --json` or `flipdiff_compare` MCP. Read verdict, totals and failing hotspots. Inspect `flipdiff explain` strips and a `flipdiff snapshot` for the worst entry. Summarize the visible change with evidence paths. If intent is unclear, post a closed-answer question via `flipdiff_ask_human` or `flipdiff ask` and link the exact view. Record proposals with `decide`; never auto-approve or update a baseline without explicit human authorization. Exit 1 is a regression, not a tool failure; exit 2 needs investigation.
