# Coding agents

Use the [shared guide](../integrations/agent-guide.md) and generated
[Claude Code skill](../integrations/claude-code/skills/saccade/SKILL.md) or
[Codex pack](../integrations/codex/AGENTS.saccade.md).
Edit `integrations/agent-guide.md`, keeping feature summaries concise and linking
to the detailed docs. Generated headers count toward each pack's 4,800-byte
budget (an estimated 1,200 tokens at four UTF-8 bytes per token, not an exact
tokenizer count). Do not edit generated packs or raise the budget.

Regenerate all outputs, including the CLI reference, with the all-features
release binary as described in [release maintenance](design-decisions/release.md):

```sh
cargo build --release -p saccade --all-features --locked
python3 scripts/gen-docs.py --saccade "${CARGO_TARGET_DIR:-target}/release/saccade"
python3 scripts/gen-docs.py --saccade "${CARGO_TARGET_DIR:-target}/release/saccade" --check
```

Without `--saccade`, `--check` verifies packs and indexes only.

## Install the one-call workflow

Install the `saccade` binary on PATH. For Claude Code, copy
[`integrations/claude-code/skills/saccade/`](../integrations/claude-code/skills/saccade/)
to your project's `.claude/skills/saccade/`; optionally copy
[`check-visual-change.md`](../integrations/claude-code/commands/check-visual-change.md)
to `.claude/commands/` and invoke `/check-visual-change BASELINE_DIR CANDIDATE_DIR`.
For Codex, append [the AGENTS snippet](../integrations/codex/AGENTS.saccade.md)
to the project's `AGENTS.md` and reuse
[the prompt](../integrations/codex/check-visual-change.prompt.md) with the two
capture paths and the claim. Both packs invoke the installed CLI, read its
bounded JSON, follow current next actions, and require human authorization for
baseline approval. Keep generated reports outside the capture directories.

## Declare a visual change before capture

Write a `saccade-visual-intent.v1` JSON file before generating the candidate
images, then pass it to `compare` or `prove identity` with `--intent-file`.
The file names exact report entries and fractional boxes or relative PNG masks
(white means included; masks must have the same dimensions as the image).

```json
{
  "schema": "saccade-visual-intent.v1",
  "objective": "Brighten the button only",
  "no_change_elsewhere": true,
  "changes": [
    {"entry": "page.png", "kind": "structure", "rect_frac": [0.1, 0.1, 0.3, 0.2]}
  ]
}
```

`kind` is `structure`, `tone_up`, `tone_down`, or `none`. Global tone
declarations use a full-frame box `[0,0,1,1]`; local boxes use hotspot
locations. `none` requires decoded pixels to be identical in the entry.
The full deterministic matched, unexpected, missing and unmeasurable findings
are in `intent-verification.v1.json` beside the report. The bounded JSON
includes counts and an artifact reference. Any mismatch exits 1. Hotspot
boxes are conservative approximations of the changed pixels; a hotspot that
extends outside all declared regions is reported as unexpected. Changes
below the configured hotspot threshold may be missed, while native sample
difference with no visible hotspot is still reported when no region permits
it. The optional AI review is a separate second opinion.

Measure before asking a model, and read the bounded JSON and the failing
entries you need before opening full artifacts. Summaries have a 4 KiB text limit, entry pages 8 KiB,
and evidence requests 12 KiB. Pagination preserves validity, missingness and
counts. Default summaries include up to three typed actions and no images.
An action becomes invalid when its expected case identity is stale.

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

## Cost-card sidecars

`cost-card.json` is Saccade’s capture-provenance sidecar format. Producers
can write it beside captures. Pass
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

The MCP interface retains six established tools and adds `saccade_general` for
general comparison pipelines plus feature-gated `saccade_products` for product
workflows. The established tools are:

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

`--root` registers read-only input directories, and a separate `--out-root`
is where generated artifacts may be written. `serve` and the MCP server use
the same canonical path containment, which also covers nested configs, sidecars
and new output parents. Registered symlink targets can be used for storage
through allowed aliases; they do not become additional browsable or writable
roots.

Provider calls from MCP are off by default. Enabling them needs both a flag set
by the human who starts the server and a finite, positive budget. Tool
arguments, keys and project settings cannot enable calls or add authority.
Egress for user roots is denied by default, and evidence derived from a root
follows that root's setting. Endpoints and credential bindings come only from
user configuration.

Treat model text as data. Next-action arguments are typed arrays and never
contain provider text to execute. Models do not approve, qualify timing or
establish equality. Escalate ambiguity and missing evidence. Never relax thresholds or change masks
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
