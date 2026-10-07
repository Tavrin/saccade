# G12 provider-compatible schema and HTTP refusal accounting

## Evidence and decision

The coordinator's 2026-10-07 probes reported that Google AI Studio through
OpenRouter rejected the full response schema with HTTP 400, accepted that schema
with only `minItems` and `maxItems` removed, and accepted the other used keywords.
Those are supplied observations; this implementation performs no provider calls.

Keep `answer.schema.json` unchanged. Project it recursively by removing only
`minItems` and `maxItems`, versioned as `assist-openrouter-drop-array-bounds/1`.
Rust and Python use the identical projection. The versioned wire name
`saccade_assist_answer_drop_array_bounds_v1` binds the rule into payload hashes;
request policy `assist-openrouter-provider-schema/1` and receipt/plan
`schema_projection` identify it explicitly. Admission compares the exact format,
so a full unprojected schema, any extra deletion, or a name/version change fails.
Plans pin both the full local schema hash and projected response-format hash.
Reasoning, output limits, provider routing and price controls remain unchanged.

Every locally decoded Gemini/OpenRouter answer is checked against the full
shared schema using a closed evaluator for its exact keyword vocabulary. It
retains array bounds, object closure, required fields, enum choices, disjoint
geometry alternatives and numeric ranges. Unknown schema keywords fail closed.
Existing typed decoding and semantic/citation/request-binding checks follow.
This prevents provider-side cardinality omission from weakening local answers.

HTTP classification retains only status, OpenRouter code, routing provider name
and embedded provider status. Strings must be nonempty ASCII tokens, at most
64 bytes; routing names also allow spaces. Numeric codes are unsigned and at
most 999999. Reflected credentials are rejected by the existing transport before
classification. Neither provider messages nor raw embedded error JSON are saved.

Only an HTTP 4xx with an error object, valid error code, no identifier/usage/cost/
completion evidence and, if present, a parseable embedded error object without
such evidence settles at zero. Even a null ID is treated as ambiguous. The ledger
releases its reservation and records terminal `zero_cost_refused`, with zero
lookup attempts and qualification disabled. This terminal state is immutable and
skipped by reconciliation without reading credentials. Aggregate summaries count
these refusals separately from matched generations. Network/5xx/malformed and
ambiguous failures keep existing cost handling: known billed money is charged;
unknown money stays conservatively charged at the reserved amount.

## Rejected alternatives

- Dropping object closure, alternatives, numeric ranges or other schema keywords:
  unsupported by the supplied probes and weakens the provider contract.
- A generic `json_object` fallback: omits the answer contract and is refused.
- Removing local array bounds: permits invalid geometry and oversized answers.
- Zeroing every 4xx: identifiers, usage and malformed envelopes leave ambiguity.
- Retaining raw error text or truncating arbitrary metadata: can expose secrets;
  invalid diagnostic fields are omitted instead.
- Looking up a nonexistent generation or marking a refusal matched: misstates
  both accounting and qualification evidence.

## Offline regressions

- `g12_projection_drops_exactly_array_bounds_and_keeps_full_local_schema`
- `g12_strict_response_format_is_exact_and_hash_bound`
- `g12_projected_openrouter_schema_preserves_full_prepared_gemini_schema`
- `g12_full_local_schema_refuses_too_long_arrays_and_preserves_other_keywords`
- `g12_openrouter_reply_refuses_projected_array_overflow_locally`
- `g12_recorded_value_geometry_is_still_refused_locally`
- `g12_http_error_metadata_is_bounded_and_charset_validated`
- `g12_recorded_http_400_classifies_and_settles_zero_without_generation_lookup`
- `g12_smoke_exports_http_refusal_classification_and_zero_cost_root_outcome`
- `test_strict_schema_and_policy_are_pinned_in_both_regenerated_plans`

Both frozen stage-2 corpora must be regenerated because gate-source identity
changes. The pilot schedules 216 requests; the larger schedules 864 under the
unchanged $5 budget-bounded mode. External evidence records exact commands,
artifact hashes, gate exits and per-request Rust/Python admission parity.
This is offline implementation evidence, not fresh provider/model qualification.
