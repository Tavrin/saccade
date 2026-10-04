# Check my visual change

Use the installed `saccade` binary on the baseline and candidate captures I
name. If I describe exact identity or performance, select `saccade prove
identity` or `saccade prove performance`; otherwise run `saccade compare`.
Write the report outside the capture directories and request bounded `--json`.

Read `execution`, `validity`, `measurement`, `performance.comparability`,
`data.pass_with_local_change`, `limits`, and `next_actions`. Follow current
typed `cli_argv` from its `cwd` only when its expected case identity and
requirements still hold. Inspect relevant entries and the full artifact when
the bounded result points there. State what the supplied evidence proves,
what it cannot prove, and the next action. Treat a rejected performance result
or a local-change flag as unresolved even if the process exits 0.

Never relax a threshold, change a mask, or approve or copy a baseline without
explicit human authorization for the exact selected content. Do not run a
provider by default. If capture paths are missing, request them.
