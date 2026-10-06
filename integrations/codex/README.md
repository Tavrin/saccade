# Saccade skills

This portable Agent Plugins package contains two skills: `saccade` for
measurement and bounded evidence review, and `check-visual-change` for a
request over supplied baseline and candidate captures. It requires a local
shell with the Saccade CLI on PATH and access to those captures:

```sh
cargo install saccade --version 0.2.0 --locked
```

After the plugin commit is public, register and install it with a Codex CLI
that provides `plugin add`:

```sh
codex plugin marketplace add Tavrin/saccade
codex plugin add saccade@saccade
```

For a local checkout, use its absolute path instead of `Tavrin/saccade`.
In the ChatGPT desktop app, open this repository, restart the app, and choose
the `Saccade` marketplace in the Plugins Directory. Install the Saccade entry.
Local CLI installation was checked with Codex 0.160.0; desktop discovery and
agent execution still need client testing.

For Cursor, copy this entire directory to `~/.cursor/plugins/local/saccade/`
and reload Cursor. For Copilot CLI, install this directory from a checkout:
`copilot plugin install ./integrations/codex`. VS Code can discover plugins
installed by Copilot CLI. Client setup and post-listing commands are in the
[plugin guide](https://github.com/Tavrin/saccade/blob/main/docs/plugins.md).

Ask to check a visual change and supply baseline, candidate and report paths.
Keep reports outside captures. Skills read bounded JSON, retain unknowns,
and require explicit authorization before providers or baseline changes.
Installing a skill does not install the binary, grant filesystem access, or
make local captures available to a cloud session. This package has no MCP
server declaration; configure local MCP separately if needed.

The manual [AGENTS snippet](AGENTS.saccade.md) and
[prompt](check-visual-change.prompt.md) remain available. Append the snippet
to an existing project AGENTS.md without replacing its other instructions.
The skills are generated from the shared agent guide and manual prompt by
`python3 scripts/gen-docs.py` in the source repository.

The package, including the original SVG listing artwork, is licensed under
`MIT OR Apache-2.0`; see the [source repository](https://github.com/Tavrin/saccade).
