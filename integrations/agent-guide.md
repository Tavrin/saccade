# Saccade agent guide

Establish the selected captures, intended change and invariants. Measure first:
`compare BASE CANDIDATE --out REPORT --json` measures differences;
`identity BASE CANDIDATE --out REPORT --json` checks native sample equality.
Identity covers the supplied captures and named scope. Capture validity and
performance qualification remain separate findings.

Read the bounded result before the full report. Inspect failed or changed entries
with `summary`, `entries`, `explain` and `snapshot`. Exit 1 is a failed measurement
gate; exit 2 is a usage, configuration or execution failure. Missing evidence is
unknown, not zero noise or proof of comparability.

State hypotheses separately from measured facts. Attribute model observations
and keep their uncertainty. Use review proposals only within explicit network
and root egress authorization and a declared provider call budget. Never forward
private captures, credentials, blind keys or session tokens without authorization.
Unknown validity, ambiguous intent, contradictory presentation orders and ties
remain unresolved and need a human review.

All `decide` answers are proposals, including `--source human`. Confidence,
calibration and panel agreement grant no decision authority. Never relax
thresholds, change masks or approve a baseline to make a task pass. Apply baseline
changes only under explicit human authorization for the exact selected content.
Historical viewer finals and model-promoted records must be reviewed again.

An authorized CLI update has two steps:

```sh
saccade approve --report REPORT/saccade-report.v1.json --all-failing --dry-run --out PLAN
# Human reviews PLAN/manifest.json, PLAN/decision.json and the exact report.
saccade approve --report REPORT/saccade-report.v1.json --decisions PLAN/decision.json --out RECEIPT
```

Use repeatable `--entry NAME` instead of `--all-failing` for a narrower scope. Deletion needs
both a decision approving deletions and `--prune-missing`. Stale report, candidate,
baseline or sidecar contents fail closed; there is no force override. Dry-run
creates a draft and manifest without changing baselines or issuing a receipt.

Authority levels remain distinct: `human` means a workbench-attested receipt;
`cli` means an unattested CLI operation; `automated` is a reserved policy record
with a policy ID and exact evidence digest. No current writer, CLI or MCP tool
can issue automated authority. It never meets a human-required check.
Human-final is an application policy and audit boundary in the user's trust
environment; an agent with unrestricted shell access can invoke CLI approval.
CLI receipts always have `human_attestation: null`.

A blind key does not blind the agent that created the pair or knows its inputs.
Record reviewer exposure. Independent blind review needs a reviewer or session
without the mapping or implementation context. Inbox feedback and preferences do
not authorize baseline writes. MCP exposes no baseline-write tool.
