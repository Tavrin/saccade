# G12 explicit OpenRouter reasoning budget

The 2026-10-07 stage-2 pilot ended with `finish_reason: length` and
`native_finish_reason: MAX_TOKENS`. Its recorded usage was 2518 prompt tokens,
4089 completion tokens, 3928 reasoning tokens inside completion, and
$0.01722225 billed; visible content was only 327 characters. The old normalized
usage parser incorrectly substituted zero thinking tokens for chat completions.
The sanitized `truncated-pilot.json` fixture retains those counters and finish
metadata, replacing the execution ID and answer text with fixture values.

## Decision

Pin request policy `assist-openrouter-reasoning/1` to the minimal unified
`reasoning: {"max_tokens": N}` shape authorized by the brief. No existing repo
record documents another unified request shape. Do not introduce `effort`,
`exclude`, or native Gemini thinking controls into this OpenRouter dialect.
Keep `require_parameters: true`, disabled fallbacks, and the existing model and
price pins. Missing, changed, or extra reasoning fields fail offline admission.

For the standard aggregate `max_tokens: 4096`, `check_ui` gets 512 reasoning
tokens; `explain` and `audit_mask` get 1024. Routing requests carry `check_ui` and
therefore get 512. Unknown/missing task metadata uses the bounded 1024 default.
For smaller aggregate ceilings, cap reasoning further at one quarter of the
aggregate, preserving at least three quarters for visible output. Payloads too
small to reserve a positive reasoning budget are refused.

The payload hash pins the exact control. Plans and money receipts also record
the request policy and requested reasoning hint. The serialized control participates in
the conservative input bound. Reserve the full aggregate completion ceiling at
the pinned completion rate once: reasoning is its subset, never an additional
4096-plus-1024 reservation. The aggregate stays within `OUTPUT_LIMIT`.

Parse `usage.completion_tokens_details.reasoning_tokens` into `thinking_tokens`.
Absent, null, or malformed counters remain unknown; only an explicit numeric
zero means zero. `candidate_tokens` retains OpenRouter's aggregate completion
counter, including reasoning; Gemini-direct counters retain their existing
semantics. Current schema descriptions document this distinction. Billed cost,
aggregate output bounds, ceiling, and reconciliation rules remain authoritative.
Exceeding the requested reasoning budget is metadata, not a monetary breach;
see [the superseding classification decision](g12-reasoning-hint.md). Impossible
reasoning-greater-than-completion counters remain an integrity failure and retain
the billed amount. Unknown thinking does not erase a known bill or
release an unknown-cost reservation.

A single choice with `finish_reason: length` is `truncated_output`, separate from
refusal or misrouting, and never supplies qualifying observations. The executor
also rejects truncation in the short smoke path. Monetary breach and identity
quarantine checks retain priority in execution; their receipt evidence is not
reclassified away by a simultaneous length finish. Collection retains truncated
requests in the full denominator as unavailable, even beside stale answer files.

## Offline preparation

Evidence and regenerated artifacts live in the external evidence directory
linked in the implementation handoff. Fresh corpora are
required because the frozen gate-source hash changes with this implementation;
the prior pilot receipt and response remain read only. The pilot contains 216
scheduled requests and the larger plan 864. The larger plan retains the
budget-bounded $5 envelope; its full reservation does not fit, and that is
explicitly recorded. Neither plan authorizes dispatch or establishes model
qualification. No provider calls or real credential reads were performed.

Focused regressions:

- `g12_task_reasoning_policy_is_closed_and_reserved_inside_output_limit`
- `g12_recorded_reasoning_usage_is_a_subset_and_unknown_stays_unknown`
- `g12_recorded_length_response_is_truncated_in_root_outcomes`
- `test_reasoning_policy_is_pinned_in_both_regenerated_plans`
- `g12_recorded_call7_reasoning_hint_is_not_a_campaign_stop`

Exact preparation, admission, and future operator commands are recorded in
`commands.sh` beside `gates.json` in the evidence directory. Future operator
commands are printed for review and were not executed.

## Validation

Required gates exited 0: formatting; strict workspace clippy with default features
and with `assist`; workspace tests with `assist,schema,evaluation`; smoke example
(10 tests); Python assist unittest discovery (18 tests); generated documentation;
and public hygiene, including its regression tests. Both regenerated plans passed
Rust offline admission before any user policy or key access. The pilot reserves
$4.6217235 for 216 requests. Full completion of the larger schedule remains
outside the $5 envelope and requires budget-bounded admission.
