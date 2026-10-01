# Visual checks with flipdiff
Use `compare BASE CAP --out REPORT --json` for regressions and `identity PARENT CANDIDATE --json` for unchanged-output refactors. Read lean verdict/totals/failing/hotspots/paths. Inspect `explain` strips and `snapshot` before drawing a visual conclusion. Exit 0 passes, 1 regresses, 2 is an error (also ambiguous bisect or ask timeout).

MCP tools: `flipdiff_compare`, `flipdiff_identity`, `flipdiff_explain`, `flipdiff_snapshot`, `flipdiff_decision_request`, `flipdiff_decide`. Record proposals; never auto-approve. Explicit human authorization is required for baseline updates.

Use `bisect --runs ... --json` / `flipdiff_bisect` to localize divergence; it assumes monotonic history and reports observed reversals and skipped-probe uncertainty. Command capture runs only via the CLI and executes user shell code.

For repeated captures use `watch BASE CAP --out REPORT --json` (JSONL), or launch MCP with `--watch BASE:CAP` and read `flipdiff_watch_status`. For ambiguity use `flipdiff_ask_human` or `ask --serve http://127.0.0.1:7878 --question ... --answers accept,reject --wait --json`; humans answer at `/inbox`, agents read `flipdiff_inbox_get`. Share a local evidence deep link including its view hash. An inbox answer does not authorize an automatic baseline update.
