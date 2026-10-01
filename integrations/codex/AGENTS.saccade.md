# Visual checks with saccade
Use `compare BASE CAP --out REPORT --json` for regressions and `identity PARENT CANDIDATE --json` for unchanged-output refactors. Read lean verdict/totals/failing/hotspots/paths. Inspect `explain` strips and `snapshot` before drawing a visual conclusion. Exit 0 passes, 1 regresses, 2 is an error (also ambiguous bisect or ask timeout).

MCP tools: `saccade_compare`, `saccade_identity`, `saccade_explain`, `saccade_snapshot`, `saccade_decision_request`, `saccade_decide`. Record proposals; never auto-approve. Explicit human authorization is required for baseline updates.

Use `bisect --runs ... --json` / `saccade_bisect` to localize divergence; it assumes monotonic history and reports observed reversals and skipped-probe uncertainty. Command capture runs only via the CLI and executes user shell code.

For repeated captures use `watch BASE CAP --out REPORT --json` (JSONL), or launch MCP with `--watch BASE:CAP` and read `saccade_watch_status`. For ambiguity use `saccade_ask_human` or `ask --serve http://127.0.0.1:7878 --question ... --answers accept,reject --wait --json`; humans answer at `/inbox`, agents read `saccade_inbox_get`. Share a local evidence deep link including its view hash. An inbox answer does not authorize an automatic baseline update.
