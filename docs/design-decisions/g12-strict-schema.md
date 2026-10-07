# G12 strict structured answers

The 2026-10-07 pilot completed with `finish_reason: stop`, 445 completion
tokens, zero reasoning tokens and $0.00355725 billed. Local closed decoding
refused its geometry objects because they used `value` instead of `pixels`.
The sanitized `value-geometry-pilot.json` fixture preserves the answer and usage,
replacing execution and provider routing identity with fixture values. The
original response remains read only.

## Decision

Use one JSON Schema in `crates/saccade-core/src/assist/answer.schema.json` for
both provider dialects and Python stage-2 preparation. The assist Gemini builder
at the starting revision only requested a JSON MIME type; the `responseSchema`
in the generic judge adapter describes a different answer protocol. Do not
reuse that judge schema for assist observations.

The Gemini assist builder now embeds the shared schema in
`generationConfig.responseJsonSchema`. This JSON Schema surface preserves
`additionalProperties: false` on every object and the disjoint box/point
`anyOf` variants without a lossy conversion to Gemini's OpenAPI schema dialect.
The [Gemini generation reference](https://ai.google.dev/api/generate-content#v1beta.GenerationConfig)
documents this alternative to `responseSchema`, including these keywords.
Gemini request settings and payload hashes contain the schema; admission refuses
drift when it is present. Anonymous extraction packets without generation
settings still receive the exact shared schema when converted to OpenRouter.

OpenRouter carries the exact format:

```json
{
  "type": "json_schema",
  "json_schema": {
    "name": "saccade_assist_answer",
    "strict": true,
    "schema": "<shared schema object>"
  }
}
```

Offline admission compares the whole format, including its name, strict flag,
required fields, enums, geometry variants and every closed object. Omitted,
weakened, extended or legacy `json_object` formats are refused before
authorization or accounting. `require_parameters: true`, disabled fallbacks,
the model and revision pins and price ceilings remain mandatory. There is no
downgrade or schema repair path.

Pin request policy `assist-openrouter-strict-schema/1` in plans and money
receipts. Exact serialized schema bytes participate in payload hashes and the
conservative input reservation; plans also record `response_format_hash`.
Include the shared schema in the frozen gate-source hash. Keep all reasoning
semantics from the [reasoning decision](g12-reasoning-budget.md): 512 tokens for
`check_ui`, 1024 for `explain`/`audit_mask`, bounded further by one quarter of
smaller aggregate limits. The aggregate output ceiling stays 4096. Reasoning
remains its subset, with no additional completion reservation.

Retain all strict local decoding and semantic checks. The schema expresses the
closed wire shape, enum choices, coordinate array sizes, normalized coordinate
ranges, uncertainty range and observation ceiling. Exact digest identity,
request-local slots and citations, nonempty observed answers, positive box area,
sum-of-coordinate bounds and atomic statement semantics remain local checks.
Provider structured output alone does not establish those facts or qualification.

## Offline evidence

Focused regressions:

- `g12_strict_openrouter_schema_is_identical_to_prepared_gemini_schema`
- `g12_strict_response_format_is_exact_and_hash_bound`
- `g12_recorded_value_geometry_is_still_refused_locally`
- `g12_shared_schema_rejects_recorded_value_and_closed_protocol_drift`
- `test_strict_schema_and_policy_are_pinned_in_both_regenerated_plans`

Fresh pilot and larger corpora and plans are required because the gate-source
hash changes. The pilot retains 216 requests and the larger plan 864. Preserve
the $5 envelope and budget-bounded mode for the larger plan; offline admission
does not authorize dispatch. External evidence records exact commands, gate
exits, artifact hashes and matching Rust/Python per-request reservations.
No provider calls or real credential reads are needed for this change.

Both regenerated plans passed Rust offline admission, with every per-request
reservation matching Python. The pilot reserves $4.8435015 for 216 requests.
The larger plan reserves $19.383933 for all 864 requests and passes only in
budget-bounded mode under the unchanged $5 allowance. Its ordinary all-fits
admission control returns the expected refusal, exit 4. Both plans pin
`response_format_hash` to
`sha256:5bae705eb4dbb529f6ed69e90ed5257a5b081d8e0087f0054f768a1b92929414`.
Provider compatibility and model qualification remain unverified.
