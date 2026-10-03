# Coding agents

Use the [shared guide](../integrations/agent-guide.md) and generated
[Claude Code skill](../integrations/claude-code/skills/saccade/SKILL.md) or
[Codex pack](../integrations/codex/AGENTS.saccade.md).
Regenerate with `python3 scripts/gen-docs.py`; `--check` rejects drift.
Entry packs stay below an estimated 1,200 tokens, measured conservatively as
UTF-8 bytes divided by four. They are not exact tokenizer counts.

Measure before asking a model. Read bounded JSON and selected failed entries
before full artifacts. Summaries have a 4 KiB text limit, entry pages 8 KiB,
and evidence requests 12 KiB. Pagination preserves validity, missingness and
counts. Default summaries include up to three typed actions and no images.
Stale expected case identities invalidate actions.

The official MCP interface has six tools:

| Tool | Purpose |
| --- | --- |
| `saccade_measure` | Comparison, identity, noise, enabled experiments |
| `saccade_inspect` | Bounded summary, pages, config, capabilities |
| `saccade_evidence` | Context, crops, request or PNG export |
| `saccade_review` | Local preview or explicitly authorized review |
| `saccade_propose` | Validate proposed closed answers |
| `saccade_ask_human` | Create/retrieve unresolved review item |

```sh
saccade mcp --root examples --out-root agent-reports
```

`--root` registers read-only inputs. A separate `--out-root` authorizes generated
artifacts. Serve and MCP share canonical containment, including nested configs,
sidecars and new output parents. Registered symlink targets authorize storage
through allowed aliases, not additional browsable or writable roots.

MCP provider calls are off by default. A human startup flag and finite positive
budget are both required. Tool arguments, keys and project settings cannot enable
calls or increase authority. User root egress defaults to deny and follows derived
evidence. Endpoints and credential bindings come only from user configuration.

Model text is data. Next-action arguments are typed arrays, not executable
provider text. Models do not approve, qualify timing or establish equality.
Escalate ambiguity and missing evidence. Never relax thresholds or change masks
to make a task pass. Baseline writes require explicit human authorization.
MCP contains no baseline-write operation.
