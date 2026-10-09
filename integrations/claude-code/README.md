# Saccade for Claude Code

This plugin measures supplied captures with an installed `saccade` CLI. It
includes the Saccade skill, the `saccade` and `check-visual-change` commands,
and a local stdio MCP server. Install the binary first:

```sh
cargo install saccade --version 0.3.0 --locked
```

In Claude Code, after the plugin commit is available on the public repository:

```text
/plugin marketplace add Tavrin/saccade
/plugin install saccade@saccade
```

`saccade` must be on the PATH Claude Code uses.

Invoke `/saccade:check-visual-change BASELINE CANDIDATE` or
`/saccade:saccade BASELINE CANDIDATE`. Reports distinguish measurement,
capture validity and performance qualification. A passing threshold grants
no baseline approval.

For a local checkout, run `claude plugin marketplace add /absolute/path/to/saccade`
and install `saccade@saccade`.

For MCP tools as well, configure the server yourself; it is not bundled in the
plugin, because it launches the separately installed `saccade` binary:

```sh
claude mcp add saccade -- saccade mcp --root /absolute/captures --out-root /absolute/reports
```

Neither root may contain the other. The server runs locally over stdio and makes
no provider calls unless explicitly authorized with a finite budget.

The package is licensed under `MIT OR Apache-2.0`. See the repository's
[licenses and source](https://github.com/Tavrin/saccade) and
[client instructions and submission checklist](https://github.com/Tavrin/saccade/blob/main/docs/plugins.md).
