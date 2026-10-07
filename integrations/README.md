# Agent integrations

Install Saccade and put it on PATH. For packaged installation in Claude Code,
Codex, Cursor, Copilot CLI and VS Code, see [plugins](../docs/plugins.md).
The agent guide packs are generated from
[agent-guide.md](agent-guide.md) by `python3 scripts/gen-docs.py`.
Copy the Claude skill directory to `.claude/skills/saccade/`, or append the Codex
pack to the project's existing `AGENTS.md`, preserving its other instructions.
The optional [Claude command](claude-code/commands/check-visual-change.md) and
[Codex prompt](codex/check-visual-change.prompt.md) turn "check my visual change"
into one agent request over the installed CLI. Provide baseline and candidate
paths; the agent reads bounded JSON and acts on current next actions.

For Claude Code, register a local server:

```sh
claude mcp add saccade -- saccade mcp --root examples --out-root agent-reports
```

For Codex, add this server to user configuration:

```toml
[mcp_servers.saccade]
command = "saccade"
args = ["mcp", "--root", "examples", "--out-root", "agent-reports"]
```

Relative roots resolve from the server working directory. Set that directory to
the project. Inputs are read-only; generated outputs need the separate out-root.
Provider execution requires human startup authorization, finite budgets and
allowed source-root egress. No baseline-write tool exists.
See [the agent workflow](../docs/agents.md).

`server.json` describes the 0.2.8 Cargo package over stdio. Registry publication
requires publishing that crate version with the visible ownership marker in
the packaged root README, then verifying the rendered marker and submitting
the manifest as the owner.
