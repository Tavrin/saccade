# Review

Measure first, then preview locally:

```sh
saccade compare examples/baseline examples/capture --entry sphere_shadow.png --out review-report --json
saccade review review-report/saccade-report.v1.json --out review-plan --json
saccade review request review-report/saccade-report.v1.json --question triage.route.v1 --out request.json
saccade review ask request.json --out human-review
```

The comparison intentionally exits 1; the other operations exit 0.
An offline `review --out DIR` preview writes `DIR/requests.json` and
`DIR/preview.json`; JSON names both paths. Each planned question reports
request bytes, estimated input tokens (bytes divided by four, rounded up),
and a 512-token output allowance. The preview prices the `jev/jev-latest`
model (Jev, from typesafe.ai, is the default triage provider; Gemini is the
other built-in provider). To estimate USD, set
`[pricing."jev/jev-latest"]` in the user TOML (`~/.config/saccade/user.toml`,
or the file named by `--user-config`):

```toml
[pricing."jev/jev-latest"]
input_per_million_usd = 1.0
output_per_million_usd = 2.0
```

The values above are placeholders; saccade ships no default price because
prices change. Without those rates, `estimated_cost_usd` is null and
`cost_reason` explains why. Estimates make no provider calls and are not
billed usage. Source roots are shown relative to the current directory unless
`--record-absolute-paths` is set.
The five questions are `triage.route.v1`, `vision.route.v1`, `perf.interpret.v1`,
`capture.disposition.v1` and `intent.match.v1`.
Questions use closed answer sets and exact case and request hashes. Answers
have bounded reasons and fact citations, and can abstain or mark facts as
missing.
`review propose REQUEST --answers FILE` validates and records proposals only.

Declare objective, expected changes, invariants and computable criteria through
`--intent-file`. Free-text `--intent` is lower-assurance input.
Measured facts, declarations, model observations and inferences remain distinct.
Observation-dependent conclusions name the actual provider/model, rubric,
presentation transform and configured/actual fallback identities.

A human may authorize `review REPORT --run --budget-calls N` after inspecting
payloads, source roots and policy. User configuration owns endpoints and
credential bindings. Built-in credentials cannot be sent to a custom endpoint;
custom providers need separate IDs and dedicated credentials. Project files
cannot define endpoints, key bindings or security authority.

Unknown or denied source roots block provider dispatch, including transitive
crops, measurements, logs and observations. Retries and fallback each reserve
from the shared ledger. Budget exhaustion preserves unresolved work.
Missing responses mean unavailable, not abstention.

For independent blind review, create anonymous views with a private key outside
the bundle. The reviewer must lack the mapping and implementation context.
Record reviewer exposure. Both-order contradictions, mixed model/order answers,
ties, unknown validity and ambiguous intent remain unresolved.

A human reviews numerical criteria and exact content before deciding.
Workbench attestation checks a scoped token, origin, expiry, content and hashes.
Offline/CLI records have no human attestation. Human-final is an application
policy and audit boundary; it cannot authenticate a person against an
unrestricted shell agent. Models never approve baselines.

For explicitly authorized selected updates:

```sh
saccade approve --report review-report/saccade-report.v1.json --entry sphere_shadow.png --dry-run --out update-plan
# The human reviews the report, update-plan/manifest.json and update-plan/decision.json.
saccade approve --report review-report/saccade-report.v1.json --decisions update-plan/decision.json --out update-receipt
```

Run these only when a human has authorized the update; they are not an
automatic fix.
Dry-run does not write baselines. Current hashes must still match at application.
Deletion requires explicit approval plus `--prune-missing`. Historical promoted
records require fresh review; confidence and calibration cannot grant authority.

For multiple independent raters, use the offline [review disagreement board](review-board.md).
