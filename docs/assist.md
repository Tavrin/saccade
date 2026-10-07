# Experimental AI advice

Build with Cargo feature `assist`. Every assist command requires
`--experimental`; all components are currently **unqualified**. Advice cannot
approve a baseline, create an exclusion, infer deployment causes or override a
comparison failure. Existing `saccade explain` keeps its evidence-export meaning.

```sh
saccade review explain --report report/saccade-report.v1.json --entry page.png \
  --experimental --offline --gemini-revision RECORDED_REVISION --out advice --json
saccade review audit-mask --report report/saccade-report.v1.json \
  --mask-manifest original-mask-memberships.json --experimental --route rules \
  --out mask-advice --json
saccade review check-ui "Continuer" --image capture.png --box 0,0,640,480 \
  --experimental --offline --gemini-revision RECORDED_REVISION --out ui-advice --json
```

`check-ui` accepts literal labels with `--kind label-visible|banner-absent`.
It also accepts `not-clipped|non-overlap` with stable `--target` and
`--second-target` node IDs from `--source-evidence` Wave 3 packets. Missing
source identity or bounds fail closed. Source text alone does not prove visible
text. Only exact hash-bound producer non-overlap geometry can route directly to
source facts. Persistence, navigation, server state, causal diagnosis and complete
accessibility cannot be requested as condition kinds.

`--box` is an original-pixel `[x,y,width,height]` region. `--incomplete-capture`
records unavailable requested scope. `--pre-masked` records missing original
pixels; hidden content is unavailable, never a safe mask. PNG/JPEG SDR images
are bounded to 16 MiB and 4,194,304 pixels. HDR/buffer display transforms are
not qualified. A report with several entries requires `--entry`; one command
selects one entry. Original hashes, geometry and capture validity are checked.

`--route rules` stops at deterministic evidence need; unresolved visual questions
remain unverifiable. `cascade` uses rules then vision and dependent Jev support.
`all-vision` also evaluates cases answerable by source facts, while missing scope
still stops. Optional `--jev-routing` adds a separately measured Jev evidence-need stage after deterministic rules. It is off by default and cannot suppress validity/missingness rules or produce a successful condition. Its fixed choices are vision or insufficient; insufficient withholds advice. Qualification compares the additional stage against the rules-only cascade.
The two anonymous orders use separate requests and caches. Different returned
revisions, unsupported statements and contradictions cannot commit advice.
Reconciliation is deliberately conservative: descriptions, roles, geometry,
visibility and citations must agree after remapping.

Each output directory must be empty. It receives `saccade-assist.v1.json`, exact
`requests.json`, `observations.json` with per-call receipts, and an escaped
**Experimental AI advice** HTML section. `mask-audit.json` contains individual
native changed-pixel accounting and a union that counts overlaps once. This is
separate from perceptual measurements and regression policy. Without expected
content, visible findings are potentially concealed changes, not proven defects.
Existing measured artifacts are not rewritten. JSON stdout is a bounded
`saccade-result.v2` summary with an artifact reference. Exit 0 means completed
execution (including explicit semantic unverifiable); exit 4 means incomplete
provider execution. Invalid inputs use the existing typed error/exit convention.

Historical reports retain a mask union, not its original component identities.
For individual attribution, supply a closed `saccade-assist-masks.v1` manifest:
`schema`, exact `report_hash` (`sha256:...`), and `masks`. Each mask has `id`,
`origin`, optional `rationale`, `dimensions`, sorted row-major `[start,length]`
`runs`, `membership_hash` (SHA-256 of one 0/1 byte per pixel), and
`original_pixels`. IDs/rationales belong to the capture producer. The manifest's
union must match the report's recorded resolved union. Matching the union proves
membership consistency, not the historical author or safety of the exclusions.
Missing component declarations remain unavailable; they are never reconstructed
from the union.

Offline operation reads exact `--replay observations.json` records or the fixed
`~/.config/saccade/assist-cache`. Provider envelopes, payload hashes, roles,
settings, transforms, catalog/condition identities and returned revisions are
checked. Replay records are attributed cached evidence, never independent samples.
`--bypass-cache` disables reuse; an explicit offline fixture is still a replay.

The Gemini/Jev interactive live network path remains refused. Its authorization
controls require `--run`, source-root export permission and a separate
output root in the existing `~/.config/saccade/user.toml` policy. Keys come only
from `~/.config/saccade/{gemini,jev}.env`, with `SACCADE_GEMINI_API_KEY` and
`JEV_API_KEY`. Ambient keys and alternative credential directories are ignored.
Requested models are fixed to `gemini-3.8-flash` and `jev-1.13.0`.
`--gemini-revision` is required; `--jev-revision` defaults to the pinned Jev ID.
The integrator must establish actual revision selectors with provider receipts.
Drift is refused rather than substituted. Provider errors are retained as bounded
local classifications, never echoed as instructions or raw secret-bearing text.

The envelope caps one entry at $0.15, eight actual requests maximum (four by
CLI default), and a shared 300-second deadline. It reserves money before HTTP
under the existing locked ledger. Prices expire on 2027-01-01. Gemini input
must fit the local 16,000-token ceiling; candidate output is explicitly bounded
to 3,072 tokens plus a separately set 1,024-token thinking budget. The billed
output reservation is their sum, not the provider total added again.

`assist-prices/2026-10-05-local-v2` uses one token per serialized UTF-8 non-image
byte plus 1,024 framing tokens. Base64 image bytes are removed from that text
calculation. The pinned `assist-image-ceilings/1` table uses declared media
resolution and actual encoded PNG dimensions: low up to 512 pixels per edge
reserves 1,024 tokens; medium up to 1,024 reserves 4,096, medium up to 2,048
reserves 8,192; high up to 2,048 reserves 8,192. Unlisted combinations reserve
the maximum 16,384 per image, which refuses admission under the current overall
ceiling. These are conservative local policy ceilings, awaiting live conformance,
not externally verified tokenizer facts. Ordinary prepared requests declare
medium resolution. Oversized requests are refused before provider dispatch.

Settlement uses prompt, candidate, thinking and total `usageMetadata` counters.
Missing, inconsistent or out-of-bound Gemini usage keeps the entire reservation
consumed and cost unknown. Cached counts cannot exceed prompt counts. Daily
interactive assist is capped at $5. `countTokens` is off by default. The core's
optional counting path requires a distinct versioned policy explicitly marking
counting free or priced; it reserves attempts and money before counting HTTP and
includes auxiliary cost in provenance. Unknown counting billing no longer blocks
generation. No live calls run during development or ordinary tests.

MCP extends `saccade_review` with `explain`, `audit-mask`, `check-ui`. Use
`artifact` for the report/image, `out`, `experimental:true`, and the corresponding
CLI controls in snake_case; `box` is an integer array. Tool arguments cannot
increase human startup authorization. All nested source, replay and mask files
stay inside registered roots. The tool is annotated as potentially networked;
local/offline operations remain explicit. CLI and MCP share the implementation
and bounded result schema. Neither surface returns approval authority.

Public Batch commands are separate from interactive advice:

```sh
# Prepare source-bound requests locally; missing offline answers may exit 4.
saccade review check-ui "Continuer" --image capture.png --box 0,0,640,480 \
  --experimental --offline --bypass-cache --gemini-revision RECORDED_REVISION \
  --out prepared --json
saccade review assist batch submit --plan prepared/batch-plan.json \
  --job jobs/check.json --experimental --run --json
saccade review assist batch status --plan prepared/batch-plan.json \
  --job jobs/check.json --experimental --run --json
saccade review assist batch collect --plan prepared/batch-plan.json \
  --job jobs/check.json --experimental --run --json
```

Each prepared visual workflow also emits `batch-plan.json`, using
`saccade-assist-batch-plan.v1`. Its provider plan freezes model/revision, price ID,
requests and spend cap; task descriptors freeze input arguments and exact file
hashes for the original report, screenshots, source packets and mask manifest.
Submission/status/collection re-read those files through registered roots,
verify the exhaustive transitive closure, and reproduce every anonymous-order
request. A payload without source references, changed source, omitted screenshot
or non-reproducing request fails closed. The durable receipt separately binds
the complete source descriptor. Batch covers frozen Gemini observation requests;
collection is advisory raw evidence, not interactive reconciliation or Jev support.

Without `--run`, submit only validates/plans and status only reads local state.
Live status and collect perform one poll and return immediately. No interactive
workflow waits for Batch. `--budget-calls` is bounded to 128 for Batch and never
increases MCP startup authority; `--deadline-secs` is at most 300. The frozen
plan carries the dollar allowance. Caps above $25 require the separately named
`--allow-spend-above-25-usd` CLI flag; MCP refuses them. The existing $30
campaign parent still applies; there is no automatic top-up.
MCP mirrors these as `saccade_review` operations `batch-submit`, `batch-status`,
`batch-collect`, using `artifact` for the plan and `out` for the durable job file.

States are planned, submitted, submission_unknown, pending, completed, partial
and failed. Unknown submissions cannot repeat silently. Per-item hashes and
revisions are checked; job success cannot hide a failed item. Monetary reservations
stay open until terminal collection. Settlement is idempotent and can recover
from a crash without another HTTP poll. Partial, failed or unknown-cost collections
retain the full charge. A recorded `collect --response FILE` is for offline
fixtures only and cannot settle a live monetary reservation. Qualification and
heavy gate commands are in [constructed qualification](assist-qualification.md).


## OpenRouter provider ceiling

The `assist_openrouter_smoke` example enables only OpenRouter chat-completions,
using the existing executor, source-root egress policy, attempt reservations and
campaign money ledger. It accepts a reviewed JSON request file, one request per
independent root (1–10 roots), and a new output directory. This is a transport
smoke, not constructed-corpus qualification. The legacy paid qualification runner
and Gemini-direct network dispatch remain refused.

Only `~/.config/saccade/openrouter.env` with `OPENROUTER_API_KEY` is accepted;
ambient keys, alternate key directories and redirects are refused. Provider
responses are checked for literal, escaped and nested credential reflections
with the actual dispatch key before artifacts are created. Accounting response
bodies are retained only as SHA-256 hashes and parsed monetary metadata.

Preflight reads OpenRouter's `/api/v1/key` and `/api/v1/credits`. The ceiling is the
minimum available key remaining limit and credit balance; a null key limit uses
credits alone. A non-null limit with invalid `limit`, `limit_remaining` or
`usage` is refused even when credits are valid; refusal is
`openrouter_ceiling_unavailable`. Neither parseable also refuses.
The allowance must fit the ceiling. Before every dispatch, after pacing, another
fresh read subtracts settled spend not yet reflected in provider usage and the
outstanding reservations (including this request). With two usage counters, the
least reflected settled spend is used conservatively. It also checks both
usage deltas against our settled spend plus a fixed $0.000001 tolerance. A missing
check, insufficient remaining balance, regressing usage or another consumer stops
the campaign, recording the reason. The ledger lock serializes these checks;
crashed and unknown-cost reservations remain nonzero. USD decimals are parsed
without floating-point rounding at the accounting boundaries.

Every request includes `usage: {"include": true}`. Returned USD cost settles the
original money receipt even when the answer fails validation. Every dispatched
receipt starts with `reconciliation.state: "pending"`; the smoke performs no
generation lookup and succeeds if dispatch and artifact writes succeed. A later
`--reconcile-only` invocation uses `/api/v1/generation?id=` to attach authoritative
cost, response hash, model and provider to pending receipts. Unpublished records
remain pending; terminal failures and cost differences above one nanodollar are
recorded as mismatch and stop spending. Failed calls and crashes retain their
charges and pending state for operator review.
The external remaining ceiling is distinct from the local campaign allowance;
local token-price estimates alone do not prove a provider invoice bound.

The request file is a JSON array of objects with exactly `root`, `model`,
`revision` (expected fingerprint, explicit `absent`, or dated model ID), and `payload` (the existing adapter's
closed chat-completions shape). Ten distinct roots are required for `--roots 10`.

The smoke records every root in `smoke.json` under `root_outcomes`, using the
assist error's static reason or a stable storage/provider code. It stops after
the first failure; remaining roots have `skipped_after_failure`. A quarantined
response is charged normally and its sanitized money receipt is written to
`receipt-N.json`; successful receipts retain their provenance fields. Both
receipt forms include reconciliation and revision identity metadata. Returned
identity is also available in the campaign ledger and the corresponding root's
`response_identity`. It contains `returned_model` (at most 128 ASCII bytes),
`system_fingerprint` (at most 256 ASCII bytes or null), `returned_revision`, and
a fingerprint presence/absence code. Identity strings allow only ASCII letters,
digits and `-_.:/`; malformed metadata is refused without retaining its text.
Dispatch-secret reflections are rejected by the existing transport first.

For the next independently authorized smoke, inspect the failed root's sanitized
identity, verify that `returned_model` matches the reviewed request's `model`,
and copy its exact observed `system_fingerprint` into that request row's
`revision`. Keep the payload model and reviewed price/cap consistent and use a
new output directory. Discovery uses the already charged response; it adds no
completion request and never automatically accepts drift. An omitted or null
fingerprint has code `openrouter_fingerprint_absent`; an unpinned absence fails
with `openrouter_fingerprint_absent_requires_explicit_pin`. Only an explicit
`"revision": "absent"` pin accepts identity as the matching returned model plus
the absence marker. A dated pin such as
`"revision": "google/gemini-3.8-flash-20260902"` also requires that same matching
alias and absent fingerprint at dispatch; its dated identity check is deferred
to reconciliation. A present fingerprint still fails this absence check. Empty, malformed or literal `"absent"` fingerprints are
invalid rather than absence. Present fingerprints still require an exact pin.

Generation accounting allows up to four read-only GETs per receipt, waiting
2, 4 and 8 seconds (14 seconds total backoff). Each GET has at most five seconds;
all pending receipts, GETs and waits share the later invocation's 30-second deadline.
A retry is skipped if its wait would exhaust that deadline. Only unpublished
generations (404 or null data) and transient transport/429/5xx failures retry.
Identity, cost, malformed-body and secret-reflection failures are terminal.
Each money receipt retains `reconciliation.state`, `reason`, `attempts`,
`waited_ms`, `attempted_ms` (Unix milliseconds), generation cost/hash and match
status, plus a history of lookup invocations. A never-published generation stays
pending with reason `openrouter_generation_not_ready`; transient unavailability
or an exhausted lookup deadline also stays pending. Pending accounting does not
stop spending or clear the existing charge. Matched and mismatched records are
terminal and skipped on repeat invocations, including credential loading when
all records are terminal. Reconciliation never repeats a paid completion,
changes an allowance, or alters settled money counters.

`revision_identity` records the generation's `dated_model`, `provider_name`,
requested revision, `revision_drifted` and `quarantined` flags. A dated model that
differs from the reviewed dated pin records `provider revision drift quarantined`
and mismatch, even when billing agrees. It is ineligible for qualification.
Pending receipts are also ineligible. `qualification_eligible` means only that
accounting and identity checks passed for a completed dispatch; it is no model
qualification claim. The smoke always retains `qualified: false`.
A campaign's aggregate reconciliation is matched only when every dispatched
receipt matched; any mismatch is a recorded failure and exits 4. Pending remains
observable and exits 0. Later attempts refresh the original receipts and root
outcomes using execution IDs; dispatch failures retain their original codes.

Live admission accepts only `google/gemini-3.8-flash`, pinned by
`openrouter-price-allowlist/2026-10-07-v1` to $0.75/M text input tokens,
$3.75/M output tokens and $0.75/M image input tokens. Source: the supplied
OpenRouter models API record (`https://openrouter.ai/api/v1/models`, 2026-10-07);
no price discovery or second model is enabled. Historical recorded response
fixtures do not authorize their model for dispatch.

Every payload requires messages, temperature zero, explicit `max_tokens` (1–4096),
JSON-object output, disabled routing fallbacks, required parameters and included
usage accounting. `provider.max_price` must be exactly
`{"prompt":0.75,"completion":3.75}` in OpenRouter's USD-per-million-token units;
the adapter sets these caps, and reviewed request files must include them.
Only text and inline PNG content are admitted. Prompt bounds count serialized
non-image UTF-8 bytes plus 1024 framing tokens. OpenRouter uses the calibrated
`assist-image-ceilings/2` table: each inline PNG reserves
`3086 * ceil(width * height / 524288)` image tokens for nonzero edges through
2048. Declared `detail:low` receives the same conservative bound as high;
auto/omitted detail uses high. Unknown dimensions/resolution use 16384 tokens
and refuse admission. Every constructed corpus image fits one area block,
so two images reserve 6172 image tokens. This is calibrated with a safety factor
of two against supplied receipts, not a documented tokenizer guarantee.
See [the calibration record](assist-qualification.md#g12-image-token-bound)
for evidence, prices and limits. The `/1` table remains readable for historical
receipts and Gemini-direct. Invalid headers, remote/unsupported media and prompt
bounds above 16000 tokens refuse before credential access or dispatch.
Reservation uses those input bounds and explicit output tokens at the pinned
integer nanodollar prices. Returned cost above reservation is charged, recorded
as a failure and stops the campaign. Actual prompt or output tokens exceeding
the receipt's reserved bound also stop the campaign, even below the global input
limit and when billed cost fits. New receipts retain their image-table revision;
old receipts retain their original stored bounds. The smoke sums all root reservations and
refuses `smoke_reservations_exceed_cap` before policy loading or dispatch if they
do not fit its allowance. This runner does not consume oracle answers,
generate corpora, score outputs or confer immutable model identity.

```sh
cargo run --locked -p saccade-core --features assist --example assist_openrouter_smoke -- \
  --requests /path/to/reviewed-openrouter-requests.json --roots 10 \
  --max-spend-usd 1 --user-policy ~/.config/saccade/user.toml \
  --out /path/to/new-openrouter-smoke
```

Later, after the provider publishes the generation records, run against the same
output directory (no request file, user policy or spend allowance is required):

```sh
cargo run --locked -p saccade-core --features assist --example assist_openrouter_smoke -- \
  --reconcile-only /path/to/new-openrouter-smoke
```

The library entry point is `assist::openrouter::reconcile_pending(ledger, http,
keys, timeout)`; the existing transport-based `reconcile` delegates to it.
Only fixed-endpoint generation GETs occur in this mode.

Every CLI rejects allowances above $25 by default. The smoke's separately named
`--allow-spend-above-25-usd` flag allows up to the existing $30 campaign parent;
it never bypasses provider checks. No verified `:batch` model/compatible shape was
supplied, so batch arms are omitted and `:batch` requests are refused. No separate
batch API is invented. All lane evidence is synthetic/in-memory; endpoint
compatibility, current model availability and actual provider enforcement require
the coordinator's reviewed live smoke.

OpenRouter requests pin an explicit reasoning subset under
`assist-openrouter-provider-schema/1`: 512 tokens for `check_ui`, 1024 for
`explain`/`audit_mask`, within the 4096-token aggregate completion ceiling.
Reported reasoning tokens are retained; missing counts stay unknown. Exceeding
the requested reasoning hint records `provider_reasoning_over_hint` and
`reasoning_hint.requested_tokens` / `observed_tokens` in monetary receipts and root
outcomes. Answers still undergo the normal content validators. Input, aggregate
completion and actual cost above the reservation still stop spending; unknown
cost stops the runner and holds its reservation for reconciliation.
See [the classification and resume decision](design-decisions/g12-reasoning-hint.md).
`finish_reason: length` yields `truncated_output` and cannot qualify an answer.
The [G12 reasoning decision](design-decisions/g12-reasoning-budget.md) records
reservation semantics, conservative failure precedence, and offline fixtures.

OpenRouter sends `response_format.type: json_schema`, with the versioned
`saccade_assist_answer_drop_array_bounds_v1` name and `strict: true`.
Projection `assist-openrouter-drop-array-bounds/1` removes only `minItems` and
`maxItems` from the [shared answer schema](../crates/saccade-core/src/assist/answer.schema.json).
The projected format is pinned in offline admission and payload identity.
Gemini requests and validation of every local answer retain the full schema.
`require_parameters: true` remains mandatory. Local citation, statement and
geometry checks still apply. Oversized arrays and `geometry.value` are refused.
See the [provider-schema decision](design-decisions/g12-provider-schema.md).

HTTP errors retain bounded, charset-validated status/code/provider classification
in receipts and root outcomes, never raw bodies or messages. Only a proved 4xx
error object with no generation, usage or completion evidence is settled as
`zero_cost_refused`; it remains non-qualifying and needs no generation lookup.
Network, 5xx and ambiguous failures retain existing unknown-cost handling.

### Conservative settlement of unknown transport cost

For an existing frozen campaign, `--resume EXISTING_DIR
--settle-unknown-at-reservation` retains each unknown receipt's full reservation,
records `settled_conservatively` (never reconciled), and permanently skips that
request as `not_run_transport_failure`. Repeat the original runner options.
Without the flag, unknown charges still block resume. Known costs and permanent
stops are never rewritten. The failed request stays unavailable in pilot scoring.
See [the settlement and scoring decision](design-decisions/g12-unknown-settlement.md).

Score an existing partial live pilot offline with `python3
scripts/assist/pilot_score.py --corpus CORPUS --requests REQUESTS --results RUN_DIR
--local-results LOCAL_RESULTS --source-revision ORIGINAL_COMMIT --out SCORE-partial.md`.
The report and adjacent JSON separate development/calibration/heldout samples,
retain unknown reservations, and remain explicitly partial and unqualified.

## Timestamped video judging

`assist video-judge --experimental` prepares frame-map/rubric requests offline;
[video judge](video-judge.md) documents both-order comparison, bounded live collection,
abstention and calibration JSONL. Native MP4 and unpriced models remain refused.
