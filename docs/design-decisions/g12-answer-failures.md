# G12 answer failures and campaign safety

A closed-protocol refusal is a model measurement only after the Executor has
settled a successful response and checked spend, identity and secret reflection.
The collector checks the successful outer envelope and known, consistent billed
usage before classifying answer errors. Executor errors never enter the answer
classification path. Local request decoding stays campaign-stopping even when
its generic schema diagnostic matches a model-answer diagnostic. Unrecognized
validator errors stop the campaign.

The recorded pilot's P2 observation citing both P1:R0 and P2:R0 remains refused.
It is now `invalid_answer`, class `answer_failure`, reason `citation_identity`.
The next scheduled request dispatches. The response and original provenance
receipt remain on disk; no accepted answer artifact is written. Export and
reconciliation retain the reason and cannot make that answer qualification
eligible, even after its monetary receipt is reconciled.

Answer reasons are closed: `citation_identity`, `observation_slot`,
`normalized_geometry`, `geometry_bounds`, `uncertainty_range`,
`unsupported_statement`, `closed_schema`, `request_bound_answer`, and
`answer_content`. The full schema still runs first, so violations of its slot
or uncertainty enums/ranges are `closed_schema`. The stage-2 validator also
checks the instructed atomic statement vocabulary and kind correspondence.
No previously refused answer is accepted.

Transport/auth/egress, HTTP errors (including settled zero-cost refusals), money
reservation/ceiling/breach, revision drift, deadlines, unknown cost and storage
errors retain campaign-stopping behavior. Budget-bounded allowance exhaustion
retains the existing clean `not_run_budget` tail. `truncated_output` remains a
campaign failure: the pinned reasoning/output budget may be misconfigured, and
collecting more such responses would spend without useful complete answers.
Treating length failures as ordinary measurements was rejected for that reason.
No output or monetary bound changes, retries or automatic pin adoption are added.

The stage-2 safety defaults are maximum five consecutive invalid answers and
maximum 50% invalid answers after at least 20 settled answers. Stop only when a
limit is exceeded: the sixth consecutive invalid answer, or a rate strictly
above 50% once the sample minimum is reached. A valid answer resets the streak;
the rate includes valid abstentions and counts every settled answer, including
orders and descendants. The global interleaved campaign shares these limits.
They protect spend and are not statistical qualification thresholds.

Configure `--max-consecutive-invalid-answers N` (1..1000),
`--max-invalid-answer-percent X` (integer 1..100), and
`--invalid-answer-min-sample M` (1..1000). These options require `--stage2` and
are parsed offline before policy or credentials. `smoke.json` records the exact
limits. The triggering result retains its valid/invalid status and a
`safety_valve` reason; every undispatched tail result is
`not_run_answer_safety_valve`. A valve stop exits 4. Ordinary invalid answers
alone do not fail campaign collection.

The stage-2 mechanical scorer reports invalid counts and reason breakdown per
arm/workload, with both settled-answer and scheduled-request rates. A root/arm
with any invalid order or descendant is a model failure, `outcome=invalid_answer`,
not an abstention. Root/arm rates deduplicate orders and descendants while
retaining the entire scheduled denominator, including stops. Missing outputs
and disagreements remain separately unavailable. Stale answer files cannot
override either an invalid result or an explicit unrun result. `--corpus` binds
workloads to the verified frozen manifest; no routing workload is guessed from
the provider's task name. This reports protocol failures, not semantic truth or
model qualification.

Request bytes and provider policy are unchanged. New plans record the answer
safety defaults and truncation decision. The frozen corpus is refreshed because
its implementation-source hash changed; offline admission compares all Python
reservations with the Rust runner before printing the pilot command.

Focused regressions:

- `g12_recorded_cross_citation_is_invalid_and_next_root_dispatches`
- `g12_answer_safety_valves_trip_strictly_above_limits_and_reset_streaks`
- `g12_post_settlement_protocol_reasons_stay_refused`
- `g12_smoke_records_static_root_codes_and_stops_after_first_failure`
- `g12_budget_bounded_schedule_reserves_until_next_shortfall_and_retains_tail`
- `g12_recorded_length_response_is_truncated_in_root_outcomes`
- `test_invalid_answers_count_in_arm_workload_and_root_denominators`

All verification is offline. The response fixture retains the recorded content
and usage, with transient envelope metadata removed. No provider calls or real
credential reads are part of implementation or validation.
