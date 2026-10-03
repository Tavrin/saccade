# Agent integrations

Install Saccade and put it on PATH. These packs are generated from
[agent-guide.md](agent-guide.md) by `python3 scripts/gen-docs.py`.
Copy the Claude skill directory to `.claude/skills/saccade/`, or append the Codex
pack to the project's existing `AGENTS.md`, preserving its other instructions.

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

`server.json` remains draft registry metadata; package and registry publication
are not established by this integration.
