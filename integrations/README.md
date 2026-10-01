# Agent integrations
Install/build `flipdiff` first and ensure the executable is on PATH.

Claude Code MCP:
```sh
claude mcp add flipdiff -- flipdiff mcp --root .
```
Copy `claude-code/skills/flipdiff/` into `.claude/skills/flipdiff/`, and optionally copy `claude-code/commands/flipdiff.md` to `.claude/commands/flipdiff.md` for `/flipdiff`.

Codex MCP, in `~/.codex/config.toml`:
```toml
[mcp_servers.flipdiff]
command = "flipdiff"
args = ["mcp", "--root", "/absolute/project/path"]
# Optional: append "--watch", "baseline:captures" to args.
```
Append `codex/AGENTS.flipdiff.md` to your project's existing `AGENTS.md`, preserving its other instructions. MCP paths are confined to `--root`. Serve's default discovery cache is available to ask tools; an explicit `cache_dir` must be within the MCP root.

For the human inbox run `flipdiff serve ARCHIVE --port 7878` and open `http://127.0.0.1:7878/inbox`. A custom serve cache requires `ask --cache-dir` or the MCP `cache_dir` argument. Keep `serve.json` private; never copy its token to remote services. See the skill for the inspect/propose/ask loop. No integration auto-approves a baseline.
