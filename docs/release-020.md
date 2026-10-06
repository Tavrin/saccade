# 0.2.0 release preparation

Prepared on `release/v0.2.0` from `53af26a7a5c883fd205946a24951e948d07488ae`.
The coordinator owns pushing, registry publication and release publication.
No push or publication is performed by this lane.

## Assist qualification decision

On 2026-10-06, `scripts/qualify-wave4.sh --max-spend-usd 15` passed the fixed
Gemini/Jev credential preflight, then refused with exit 4 because `--corpus`,
`--out` and an exact-source `--gate-receipt` are required. No provider call was
made; spend was $0 against the $15 cap. Receipt:
`/mnt/linux-extra/moss-scratch/saccade-release/receipts/qualification-preflight.log`.

No frozen constructed epoch with an observed immutable Gemini revision was
provided or found in the lane evidence. The user provider config contains only
pacing, with no registered egress roots/output root. Missing inputs do not
establish a failed accuracy measurement or allow a guessed revision. Qualification
remains unqualified under [the frozen policy](assist-qualification.md).

| Feature decision | Status | Reason |
| --- | --- | --- |
| `explain` | Unqualified / experimental | No admissible live constructed epoch or scored held-out observations. |
| `audit_mask` | Unqualified / experimental | Same prerequisites; no precision/recall/safety evidence. |
| `check_ui` | Unqualified / experimental | Same prerequisites; no visible-condition qualification. |
| `blind_orders` | Unqualified / experimental | No paired live order/availability gates. |
| `jev_support` | Unqualified / experimental | No incremental unsupported-assertion reduction/value gates. |
| `routing` | Unqualified / experimental | No matched-coverage recall, cost and necessary-evidence gates. |
| `jev_evidence_routing` | Unqualified / experimental; off by default | Separate optional routing arm was not measured. |

Batch submit/status/collect transports these workflows and inherits no accuracy
qualification. CLI `--experimental` remains required. Advice never approves
baselines, creates exclusions or overrides numerical failures.

Decision: retain conservative status and the refusal receipt. Rejected:
bypassing preflight, inventing an immutable model revision, silently shrinking
held-out counts, or fabricating a passing gate receipt. Reversal requires a new
source-bound corpus/receipt and a capped, explicitly authorized qualification run;
no API/schema migration is needed.

## Release gates

Results and source identity are retained in the lane receipts directory. Pending
checks are not acceptance. Native platform, live-provider, browser and external
publication checks remain separate from local release preparation.
