# G12 reasoning budget is a provider hint

The supplied epoch-2 pilot stopped on call 7 solely because 1638 reasoning tokens
exceeded its requested 1024-token hint. Input 5173 was below 14261, aggregate
completion 1862 below 4096, and known cost 10,862,250 nano-USD below the 26,055,750
reservation. These recorded counters establish no money-bound breach.

## Decision

Keep provider request policy `assist-openrouter-provider-schema/1`, prompt epoch
`g12-pilot/2`, prompt policy, encoder, request payloads, model/revision pins,
input/output limits, prices, allowance and answer safety valves unchanged.
Reasoning is already included in aggregate completion and is never added again.
Record nullable `provider_reasoning_over_hint` and `reasoning_hint` with
`requested_tokens` and `observed_tokens` in monetary receipts and root outcomes.
The legacy `reasoning_bound` field remains the requested count for compatibility;
it is not a monetary bound. Null thinking usage yields a null flag.

Input and aggregate output overruns still set `bound_breach` and permanently stop
spending. Actual cost above the reservation still permanently stops spending.
Unknown cost stops the runner, holds the full reservation and requires billing
reconciliation before resume; it cannot produce a completed answer. Impossible
reasoning subset counters remain an integrity error, with the known bill retained.
Normal identity, truncation, protocol and content validators remain authoritative.
An over-hint answer may be completed or invalid on its content.

Stage-2 scoring exposes the rate per arm/workload over known hint observations,
and a separate scheduled-request rate. Unknown counts are excluded only from the
known-usage denominator; all requests remain in the scheduled denominator.
Known counters in legacy receipts count even if the old outcome is a campaign
failure. This does not turn unavailable answer evidence into valid observations.

## Existing pilot and fresh-run fallback

Inspected only recorded counters/outcomes and the file inventory under the
read-only epoch-2 pilot directory. There are six response and answer files;
`response-6.json` and `answer-6.json` are absent. `receipt-6.json` contains monetary
usage and identity, not answer text. The old executor returned before
`record_response`, so the seventh answer cannot be evaluated offline. The
campaign ledger was durably stopped by the old monetary classifier.

Do not clear the old stop bit, rewrite known charges, fabricate a seventh answer,
or re-dispatch a settled call. Instead advance the local runner identity to
`saccade-g12-campaign/3`. This deliberately refuses old `/2` ledgers and takes the
spec's fresh-run alternative. It does not alter provider request identity.
The regenerated source-bound corpus records the new implementation; request
payloads/schedule are checked byte-for-byte against the epoch-2 plan. Existing
pilot evidence remains untouched. A fresh campaign incurs its own allowance;
this is not continuation of the old spend ledger.

Under `/3`, settled over-hint calls with evaluated answers resume at the next
root, preserving their bill, content outcome and safety history. Focused fixtures
cover both valid and invalid answers. Fixture call-7 content is synthetic; only
its usage counters reproduce the recorded evidence.

## Operator command forms

Evidence, gates, freshly frozen pilot plan, executable and fully expanded exact
commands are in the external directory supplied in the implementation handoff.
Set `G12_EVIDENCE` to that directory and `SACCADE_USER_POLICY` to the original
reviewed policy file. These portable forms were printed only; no provider calls
or key reads ran.
Fresh run (the required fallback for the old stopped pilot):

```sh
"$G12_EVIDENCE/assist_openrouter_smoke" --stage2 \
  --requests "$G12_EVIDENCE/pilot-plan/requests.json" \
  --roots 216 --max-spend-usd 5 \
  --max-consecutive-invalid-answers 5 --max-invalid-answer-percent 50 --invalid-answer-min-sample 20 \
  --user-policy "$SACCADE_USER_POLICY" \
  --out "$G12_EVIDENCE/pilot-live"
```

Exact resume command for that fresh `/3` campaign:

```sh
"$G12_EVIDENCE/assist_openrouter_smoke" --stage2 \
  --requests "$G12_EVIDENCE/pilot-plan/requests.json" \
  --roots 216 --max-spend-usd 5 \
  --max-consecutive-invalid-answers 5 --max-invalid-answer-percent 50 --invalid-answer-min-sample 20 \
  --user-policy "$SACCADE_USER_POLICY" \
  --resume "$G12_EVIDENCE/pilot-live"
```

Any incomplete dispatch still needs authoritative reconciliation before retry:

```sh
"$G12_EVIDENCE/assist_openrouter_smoke" --reconcile-only \
  "$G12_EVIDENCE/pilot-live"
```

## Offline validation

Focused regressions:

- `g12_recorded_call7_reasoning_hint_is_not_a_campaign_stop`
- `g12_call7_only_input_and_total_output_are_token_reservation_bounds`
- `g12_total_output_over_reservation_is_charged_and_stops_campaign`
- `g12_returned_cost_over_reservation_is_charged_and_stops_campaign`
- `g12_openrouter_reserves_settles_unknown_zero_and_quarantines_usage_breach`
- `g12_resume_over_hint_settled_answer_continues_at_next_root`
- `test_reasoning_over_hint_rates_keep_invalid_and_unknown_usage_separate`

Gate exit receipts and source/executable identities are recorded externally in
`gates.json` and `admission-verification.json`. These are offline tests and
admission evidence, not live provider compatibility or qualification.
