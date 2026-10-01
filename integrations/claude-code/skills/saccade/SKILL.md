---
name: saccade
description: Compare rendered images, inspect visual evidence, and ask a human before baseline updates.
---
Use for screenshot regressions, renderer refactors, ablations and capture archives.

- Compare: `saccade compare BASE CAP --out REPORT --json`, MCP `saccade_compare`.
- Refactor identity: `identity PARENT CANDIDATE --json`, MCP `saccade_identity`.
- Localize history: `bisect --runs R0 R1 R2 --out BISECT --json`, MCP `saccade_bisect` (existing runs only).
- Iteration: `watch BASE CAP --out REPORT --json` (JSONL); MCP start `mcp --root . --watch BASE:CAP`, then `saccade_watch_status`.
- Rank alternatives: `rank REF CANDIDATE... --json`, MCP `saccade_rank`.
- Inspect: `summary REPORT_JSON --format json`; `explain REPORT_JSON --out PACK`; `snapshot REPORT_JSON --state 'entry=scene.png&hotspot=1' --out SNAPSHOT.png`. Corresponding MCP tools use the command name prefixed `saccade_`.

Loop: read lean JSON verdict/totals/failing/hotspots/paths → inspect snapshot and explain strips → `decision-request` → record a proposal with `decide` → for ambiguity use `ask --serve http://127.0.0.1:7878 --question 'Intended?' --answers accept,reject --link '/compare?runs=a,b#entry=scene.png' --wait --json` or `saccade_ask_human`; read with `saccade_inbox_get`.

Never auto-approve, change thresholds to hide failures, or move baselines from a model proposal or inbox answer. Ask for explicit human authorization before `approve`.

Exit 0 = pass; 1 = regression; 2 = usage/IO/config or inconclusive bisect/ask timeout. Ask open/answered returns 0; timeout returns 2 with `timed_out:true`. Watch Ctrl-C returns the last result's code. Lean `saccade-result.v1` uses absolute paths and rounded floats; use full reports only when needed. MCP regressions are successful tool results; `isError:true` indicates a failed call. Bisect command capture is CLI-only: user shell code may have arbitrary side effects, and saccade never checks out revisions itself.
