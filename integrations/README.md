# Agent integrations
Install/build `saccade` first and ensure the executable is on PATH.

Claude Code MCP:
```sh
claude mcp add saccade -- saccade mcp --root .
```
Copy `claude-code/skills/saccade/` into `.claude/skills/saccade/`, and optionally copy `claude-code/commands/saccade.md` to `.claude/commands/saccade.md` for `/saccade`.

Codex MCP, in `~/.codex/config.toml`:
```toml
[mcp_servers.saccade]
command = "saccade"
args = ["mcp", "--root", "/absolute/project/path"]
# Optional: append "--watch", "baseline:captures" to args.
```
Append `codex/AGENTS.saccade.md` to your project's existing `AGENTS.md`, preserving its other instructions. MCP paths are confined to `--root`. Serve's default discovery cache is available to ask tools; an explicit `cache_dir` must be within the MCP root.

For the human inbox run `saccade serve ARCHIVE --port 7878` and open `http://127.0.0.1:7878/inbox`. A custom serve cache requires `ask --cache-dir` or the MCP `cache_dir` argument. Keep `serve.json` private; never copy its token to remote services. See the skill for the inspect/propose/ask loop. No integration auto-approves a baseline.

## MCP Registry (not yet published)

[`server.json`](../server.json) is draft metadata for `io.github.tavrin/saccade`,
using the [official MCP Registry format](https://github.com/modelcontextprotocol/registry/blob/main/docs/reference/server-json/generic-server-json.md).
It describes a future Cargo package and the installed `saccade mcp --root` binary
invocation; the crates.io package and registry entry are **not yet published**.
For now install the binary from [Releases](https://github.com/Tavrin/saccade/releases)
or `cargo install --git https://github.com/Tavrin/saccade saccade --locked`, then use
the configurations above. Registry publication requires verifying package
ownership and availability; this metadata does not publish anything.
