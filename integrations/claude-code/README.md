# Saccade for Claude Code

This plugin measures supplied captures with an installed `saccade` CLI. It
includes the Saccade skill, the `saccade` and `check-visual-change` commands,
and a local stdio MCP server. Install the binary first:

```sh
cargo install saccade --version 0.1.0 --locked
```

In Claude Code, after the plugin commit is available on the public repository:

```text
/plugin marketplace add Tavrin/saccade
/plugin install saccade@saccade
```

Set `source_root` to an absolute existing directory containing your captures.
Set `out_root` to a separate absolute report directory. Neither directory may
contain the other. The plugin prompts for both; it supplies no default roots.
The MCP process inherits PATH, so `saccade` must be available to Claude Code.
Use a current Claude Code with plugin `userConfig` support; strict validation
and a local install were checked with 2.1.289.

Invoke `/saccade:check-visual-change BASELINE CANDIDATE` or
`/saccade:saccade BASELINE CANDIDATE`. Reports distinguish measurement,
capture validity and performance qualification. A passing threshold grants
no baseline approval. The MCP declaration enables no provider calls or egress;
separate explicit authorization and a finite budget are required for those.

For a local checkout, run `claude plugin marketplace add /absolute/path/to/saccade`
and install `saccade@saccade`. Shell installation can supply settings with
`--config source_root=/absolute/captures --config out_root=/absolute/reports`.

The bundled MCP process runs locally over stdio. It is not a hosted server for
claude.ai. Anthropic may require changes during directory review because it
launches an installed executable outside the plugin folder. Local validation
and installation do not establish directory acceptance.

The package is licensed under `MIT OR Apache-2.0`. See the repository's
[licenses and source](https://github.com/Tavrin/saccade) and
[client instructions and submission checklist](https://github.com/Tavrin/saccade/blob/main/docs/plugins.md).
