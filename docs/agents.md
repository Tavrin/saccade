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

For ablation with repeats, read `base_repeats`, `excluded_base_repeats`,
`repeat_qualification`, and each arm's `repeats` and `excluded_repeats` in
`saccade-ablate.v1.json`. A rejected repeat qualification cannot support a
performance claim even if the image comparison passes.
If an arm reports `arm output is not deterministic across repeats`, inspect
its per-image repeat hashes and max FLIP in the full artifact, then recapture
under fixed conditions. The ablation HTML and Markdown exports carry the same
finding. Do not approve the arm based on a single chosen repeat.

## Reading a compare or identity result

`compare --json` and `identity --json` print one bounded `saccade-result.v2`
object. Read these fields first:

| Field | Meaning |
| --- | --- |
| `verdict` | `regression`, `pass` or `performance_rejected` |
| `performance` | Present when a performance comparison ran: `verdict`, `comparability`, `repeat_qualification`, `summary` |
| `overall` | `performance_rejected` when images pass but the timing comparison is rejected; absent otherwise |
| `worst` | The failing entry with the highest value/threshold ratio: `entry`, `metric`, `value`, `threshold`; `null` when no failing entry has a value |
| `failing` | Up to five failing entries, worst first |
| `next_actions[]` | Typed follow-up commands, with `cli_argv` and `cwd` |

`performance_rejected` means the image thresholds passed but the two runs'
timings cannot be compared (for example, a producer qualification check
failed or the frame statistics differ). `performance.comparability` and its
summary give the reasons. The exit code still follows the image result, so a script that
gates on exit 0 keeps working. Do not report such a run as a plain pass.

`failing` and `worst` sort by `value / threshold`, highest first. Entries
without a measured value (`missing`, `new`, `error`) come after every measured
failure, then by name. Read `worst` to pick the entry to inspect first.

Each next action carries `cli_argv` and `cwd`. Relative paths in `cli_argv`
resolve against `cwd`, the absolute directory where saccade ran. Run the
command from `cwd`; other paths in the result stay relative and portable.

## Inspecting one entry or the validity reasons

```sh
saccade inspect REPORT/saccade-report.v1.json --entry NAME --json
saccade inspect REPORT/saccade-report.v1.json --validity-reasons --limit 10 --json
```

`--entry NAME` returns that entry's deciding fields: `status`, `metric`,
`value`, `threshold`, up to three `hotspots` (`rect_px`, `max_flip`,
`position`), a short `explanation` and `error`. The top-level `measurement`
is that entry's result. A next action points at `inspect evidence` for crops.
Full reports are never copied into this output.

`--validity-reasons` pages through every capture-validity reason. Each item
has an `index` and a `reason` that names the image and the missing or
mismatched evidence. Follow `page.next_cursor` with `--cursor`. It
cannot be combined with `--entry` or `--status`.

## Moss cost cards

Captures produced by the Moss engine carry a `cost-card.json` sidecar. Pass
`--meta-name cost-card.json` to `compare` and `identity`. Saccade reads
build provenance from these keys (case and `.`, `-`, space are ignored):

| Provenance field | Accepted keys |
| --- | --- |
| `binary_sha256` | `binary.sha`, `binary_sha256`, `binary_hash`, `build_binary_sha256` |
| `source_head` | `build.commit`, `source_head`, `git_head`, `source_git_head` |

These are the producer's declarations, not hashes computed from pixels. When
a field is missing, the result's `validity_missing` names the missing keys and
the `--meta-name` sidecar that can supply them.

## Review preview cost

`review REPORT --out DIR --json` writes `DIR/requests.json` (the exact
payloads) and `DIR/preview.json` (this result). Each planned question reports
`request_bytes`, `estimated_input_tokens` (bytes divided by four), a 512-token
`estimated_output_tokens` allowance and `estimated_cost_usd`. The cost is
`null`, with a `cost_reason`, unless the user configured model rates. No
provider is contacted. See [review](review.md).

## MCP tools

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
Check `data.pass_with_local_change` on a passing compare result. It marks a
severe local hotspot hidden by the deciding average; follow the inspect action
and read the full report entry before describing the capture as unchanged.
The default rule is max FLIP ≥ 0.5 and area ≥ 16 pixels.

For performance evidence, read `qualification_reasons` and any
`gpu_clock_before` / `gpu_clock_after` summaries. An unqualified or mismatched
clock or power state makes performance comparability `rejected`; image results
remain separate.
