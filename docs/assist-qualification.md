# Constructed assist qualification

This is a separate `saccade-constructed-truth.v1` epoch. Historical evaluation,
human labels and qualification status remain unchanged. No manual labels are
accepted. All candidate components are **unqualified** until an exact-source
heavy gate receipt and the preregistered provider/value gates pass.

The generator freezes family splits, independent root seeds, random geometry,
font sizes, colours, widths, themes, French labels, device-pixel ratios, renderer
and font hashes, source/prompt/schema hashes, pinned model bindings, admissibility,
metrics and gates. Descendants, reverse orders and retries keep the same root and
split. Twenty families are held out from development and calibration. The default
held-out target is 1,000 independent roots per workload: 600 important challenges,
200 controls and 200 unavailable cases. All exclusions and their preregistered
render-witness reasons are published before provider results exist.

```sh
python3 scripts/assist/corpus.py --out /path/to/new-corpus \
  --gemini-revision OBSERVED_IMMUTABLE_REVISION --jev-revision jev-1.13.0
python3 scripts/assist/corpus.py --out /path/to/new-corpus --verify
```

Generation is CPU-only and uses installed Pillow and DejaVu fonts. No models,
browsers or network are involved. The manifest records the exact local versions,
font hashes and licence. Generated pixels are MIT OR Apache-2.0; the font notice is
copied into the frozen artifact, not the repository. Retain those renderer/font
versions to reproduce the epoch. Frozen directories are never overwritten.

The oracle checks actual PNG samples, glyph witnesses, geometry, changes and mask
membership; running a mutation command is not enough. It re-renders and verifies
the private oracle against frozen seeds during scoring. Manual expected-answer
changes, edited pixels, split leakage and hash drift are refused. Routing includes
opposite visible truths with the same empty structured packet; the full-path
counterfactual stays attached to its original root and cannot increase independent
sample counts. Private categories, mutations, seeds and oracle answers never enter
vision/routing inputs. Only the explicitly diagnostic Jev ceiling arm receives
oracle-derived textual observations; it is segregated from qualification arms.

Assist statements use a finite atomic protocol: literal `text:...`,
`presence:present|absent`, `clipping:clipped|contained`, `overlap:overlap|separate`,
`appearance:changed|unchanged`, `appearance:lines=N`, or `appearance:rgb=R,G,B`.
This prevents descriptions from asserting causal or behavioral success. Literal
transcriptions remain untrusted data. The initial oracle scores presence, text
and changed/unchanged appearance. Other atomic facts are counted as unsupported
until an independent oracle for them is added in a new epoch. This conservative
qualification scope cannot establish general natural-language explanation accuracy.

Run `scripts/gates-wave4.sh` with your optional admission wrapper. It checks
formatting, default/relevant-feature compilation, strict clippy (including examples
and tests), full touched-crate tests, ignored Wave 4 CLI/MCP end-to-end tests,
CPU corpus/scoring tests, docs/schema references and shell syntax. It emits one
`GATE <name> PASS|FAIL` per gate and exits nonzero on failure. A receipt is written
only after all gates pass, under `CARGO_TARGET_DIR/gates-wave4-receipt.json` or
`SACCADE_WAVE4_GATE_RECEIPT`. Disk preflight refuses every build below 25 GB free.
No provider calls occur in this gate script.

Provider qualification is a separate explicit opt-in:

```sh
scripts/qualify-wave4.sh --max-spend-usd 250 --corpus /path/to/frozen-corpus \
  --gate-receipt /path/to/gates-wave4-receipt.json --out /path/to/new-results
```

The runner uses the existing user-root egress policy, fixed key files, isolated
**evaluation** attempt ledger and monetary reservations. It has no cache replay,
model fallback, automatic retry or budget top-up. It durably records each root/arm
as it finishes; provider errors and budget exhaustion remain in availability
denominators. Reopening an epoch cannot raise its monetary allowance. The script
checks the fixed ordinary credential files before corpus reads, output creation
or any build; missing keys refuse with exit 4 without a provider request. Ambient
keys cannot pass preflight. The corpus, new result directory and exact-source
heavy receipt remain explicit required arguments alongside the cap.

Local conservative reservations replace mandatory Gemini counting. Output and
thinking budgets are both explicit. Settlement needs consistent prompt,
candidate, thinking and total counters; unknown usage is never zero cost.
`countTokens` is off by default and only a separately versioned free/priced policy
can enable it. Existing frozen epochs are invalidated by this policy/source change;
freeze a new `wave4-constructed/2` epoch with `constructed-assist/2` policy.

Seven paired arms are recorded: rules, diagnostic oracle-text Jev, single Gemini,
two Gemini plus mechanics, two Gemini plus mechanics and Jev, rules-first cascade,
and `cascade_jev_route` with the optional Jev evidence-need component. The last arm
is measured separately against `cascade`, including its additional request,
usage, cost, latency, withheld advice and necessary-evidence skips. Jev routing
remains off by default; `jev_evidence_routing` gets its own value/recall/safety
decision and cannot inherit the deterministic router's qualification.
The full path evaluates cases the cascade routes to source facts. Necessary-pixel
counterfactuals also run the full path; they remain descendants of the same root.
Every observation retains actual model revision, exact request/response/prompt
hashes, sampling, usage, cost status and timings. No oracle answers are exposed to
vision or to the routed candidate path.

Scoring reports every preregistered metric per workload and arm: availability,
case and assertion precision, committed coverage, important-change recall, false
reassurance, abstentions, order disagreement, unsupported assertions, geometry and
citation failures, routing mistakes, tokens, unknown cost, cold-call p50/p95,
queue delay, retries and end-to-end latency. Missing results remain failures;
abstention is neither detection nor false reassurance. Exact one-sided binomial
bounds assume independent roots. Per-family results and a fixed-seed family block
bootstrap are reported separately, with explicit family gates. Routing uses a
fixed hash-rank matched-coverage comparison. Jev requires measurable unsupported
assertion reduction; a zero baseline does not establish incremental value.

```sh
python3 scripts/assist/score.py --corpus /path/to/frozen-corpus \
  --results /path/to/results.jsonl --gate-receipt /path/to/gates-wave4-receipt.json \
  --out /path/to/saccade-constructed-truth.v1.json
```

Each workflow, blind-order handling, Jev support and routing receives an explicit
qualified/unqualified decision and failed-gate list. Missing mechanical receipts,
insufficient corpus size, failed family assumptions or unknown cost preserve
unqualified status. These are synthetic-domain results. They do not establish
accuracy on arbitrary pages or grant baseline/exclusion authority.

## G12 pre-spend correction (2026-10-06)

The earlier paid-run instructions above are superseded. **Gemini-direct live network
adapters and the old paid runner refuse dispatch.** The bounded OpenRouter-only
smoke described below has a separate provider-verified admission boundary. No verified provider-side
invoice limit or authoritative token/billing ceiling was supplied. Local prices
prove reservation arithmetic only; they cannot establish an unconditional invoice
ceiling. There is no environment switch to bypass this refusal. Re-enabling live
billing requires a separately reviewed limit capability and a complete campaign plan.
The batch example now supports offline collection only; raw submission is crate-private.

```sh
scripts/qualify-wave4.sh --dry-run
scripts/qualify-wave4.sh --dry-run --out /path/to/new-offline-run
python3 scripts/assist/test_g12.py
```

The dry run freezes 20 roots per workload, executes fake providers, persists separate
execution transcripts and monetary receipts, reconstructs requests while scoring,
and checks `scripts/assist/fixtures/dry-run-expected.json`. It never reads provider
credentials or opens sockets. Its small support and absent heavy-gate receipt keep
all qualification decisions **unqualified**. Positive 1,000-root metric fixtures
and failing mutations test the real qualification thresholds without lowering them.
The normalized synthetic request dialect is `synthetic-offline/1`; its results
cannot be relabelled as live model qualification or immutable-revision evidence.

The new epoch is `wave4-constructed/3`, policy `constructed-assist/3`. Old freezes
must be regenerated. Labels no longer encode seed/class suffixes. The independent
oracle distinguishes absent, partial and complete text, checks glyph coverage in
the actual observation box, and requires task-specific evidence for every mask.
Semantic outcome correctness, control errors, unavailable commitments, complete
cost accounting and at least 600 usable paired roots have separate gates. Single
image tasks have no paired-order applicability. Assertion confidence uses one
all-facts-correct event per root, retaining descendants within that event. Reports
include class, mutation, visibility, label-strength and condition support strata.
Colour, line count and pixel-only overlap still lack independently qualified truth
and remain unsupported. Source-derived non-overlap remains a deterministic fact.

Each recorded provider stage must have an execution ID, returned identity, usage,
request/response hashes and a matching separately persisted campaign receipt.
Receiptless provider arms, invented source-only claims, unmatched dispatches,
unknown usage and altered outputs are refused. This is evidence within the local
trusted artifact boundary, not cryptographic proof against an unrestricted shell
that can replace every artifact. Unknown usage retains its reservation. Known
bound breaches are charged even if the answer is invalid and permanently stop
all campaign dispatch; monetary accounting shares one ledger across production,
evaluation, batches, retries and epochs with a $30 parent ceiling. Older
namespace-local monetary records require reviewed migration rather than a silent
allowance reset. Legacy floating-point cap inputs now round down; exact decimal
CLI parsing remains a recorded low-severity follow-up.

OpenRouter has an explicit chat-completions adapter (`assist::openrouter`) and a
`two_openrouter` recorded arm. The historical `openai_compatible` adapter now uses
the fixed OpenRouter endpoint without a project `base_url`. It accepts namespaced
JSON model IDs, bounded output, explicit routing, usage and execution identity.
`OPENROUTER_API_KEY` comes only from
`~/.config/saccade/openrouter.env`; ambient keys are refused and keys are never printed. Responses are checked
for literal, Unicode-escaped and nested reflections with the actual dispatch key.
OpenRouter is the billing source and passes provider prices through without markup.
Price fixtures are versioned and explicitly **not live-verified**; returned
fingerprints are treated as alias-bound and time-specific. API-shape recordings
are sanitized synthetic fixtures, not claimed captures of live API traffic.

Gate receipts now require positive matching test counts, source closure before and
after the gates, toolchain/build configuration and the tested binary hash. The
empty ignored core selection was removed; receipt-write failure fails the gate.
No heavy or live-provider qualification is implied by focused lane tests.

`qualify-wave4.sh --plan --corpus DIR` emits a read-only full arm/descendant
request count, qualifying class support, conservative local reservation estimate
and $30 campaign-fit decision. It never grants authorization; missing billing,
API payload or reviewed execution inputs are explicit plan blockers. Full support
and budgets are required before a future paid campaign can be considered.


## OpenRouter ceiling stage

### Smoke observability and reconciliation correction

The smoke preserves the first failure's static assist code and records skipped
roots. Already charged responses supply bounded returned identity for operator
pinning; quarantined responses never become accepted advice. Missing fingerprints
require an explicit `absent` revision pin together with an exact returned model.
Later generation reads retry within one 30-second deadline and retain fixed
reasons; unpublished records remain pending. Dispatch itself performs no
reconciliation GETs. The operator procedure and exact retry limits are documented in
[assist.md](assist.md#openrouter-provider-ceiling).

Focused synthetic/recorded-shape regression mapping:

| Goal | Test |
| --- | --- |
| Per-root static codes and existing stop policy | `g12_smoke_records_static_root_codes_and_stops_after_first_failure` |
| Discovery from one charged response, with bounded sanitized metadata | `g12_quarantined_drift_retains_bounded_identity_without_another_dispatch` |
| Delayed generation lookup and retained failure reasons | `g12_delayed_generation_retries_and_preserves_terminal_failure_reasons` |
| Explicit absent-fingerprint identity rule | `g12_absent_fingerprint_requires_explicit_pin_and_matching_model` |

`g12_generation_deadline_bounds_gets_waits_and_following_receipts` additionally
proves that GET time and waits share the deadline across receipts. These tests
use fake HTTP and synthetic credentials only. Decisions: retain quarantine and
stop-on-first-failure; reject automatic pin adoption or paid completion retries.
Reserve `absent` for explicit missing-fingerprint identity; reject malformed and
empty fingerprints. Retry only unpublished or transient generation accounting,
retaining terminal identity, billing and reflection failures. No scoring, token
ceiling, Gemini-direct or batch behavior is changed by this correction.

Correction validation: fmt, workspace/all-target strict clippy with `assist`,
separate core and CLI tests with `assist,schema,evaluation`, the four smoke
example tests, all 14 Python assist unittests, generated-docs check and public
hygiene checks exited 0. Test binaries used an isolated user directory; the
initial ambient-policy workspace run exited 101 on a CLI configuration error.
An additional isolated workspace-wide run exited 101 on unchanged compression
schema drift in `saccade-quality-report.v1.schema.json`. That broader failure
remains outside this correction; schemas and assertions were not altered to
accept it. Fixture validation does not establish live API compatibility,
fingerprint availability for this model or model/corpus qualification.

Use the bounded request-file invocation in [assist.md](assist.md#openrouter-provider-ceiling)
for the coordinator's 10-root, $1 smoke. It uses existing accounting and egress,
provider-key and credit preflight, a fresh check after pacing on every dispatch,
returned-cost settlement and generation reconciliation. Campaign receipts record
allowance, ceiling snapshots, raw-response hashes, generation costs and stable
refusal classifications. No live request was run or API compatibility verified
in the fixture lane. Passing fixture/gate tests establishes implementation
behavior only; all model/corpus qualification decisions remain unchanged.

Decisions: reuse the existing executor and ledger, including the $30 parent;
reject a second accounting path or an unverified bypass. Reversal would require
an accounting migration and a new provider-limit design. Use a separate bounded
smoke over reviewed requests; reject relabelling it as the old full qualification
campaign. Reversal requires a complete reviewed execution schedule and unchanged
qualification gates. Omit batch because no supported model variant/shape was
supplied; adding it requires provider evidence and fixtures, not a guessed API.
Keep Gemini-direct refused; changing that requires a provider-side hard cap.
Use exact monetary decimal parsing and fixed tolerances; changing tolerances or
limits requires reviewed policy and renewed fixtures.

Ceiling review fixes bind live dispatch to the single supplied model price pin
(`openrouter-price-allowlist/2026-10-07-v1`, OpenRouter models API record dated
2026-10-07), payload-derived text/PNG token ceilings, explicit completion bounds
and provider route price caps. By default the smoke admits the entire schedule's
worst-case reservations against its cap before the first call. Explicit
`--budget-bounded` mode admits each call through the same reservation and ceiling
checks and retains unrun requests as unavailable. Malformed non-null key limits
cannot fall back to valid credits. Settled spend remains deducted until both
tracked provider usage counters reflect it. Fixtures cover stale usage plus an
equal spend by another consumer, partial reflection and full reflection.

Decisions: accept only the model with supplied price evidence; reject guessing a
second price or fetching prices in the fixture lane. Adding a model requires a
new recorded price pin and renewed admission fixtures. Reuse the PNG ceiling
table with high-resolution bounds for every image detail; unsupported content
refuses. Broadening content requires a conservative token bound and fixtures.
Require the reviewed request file to carry the exact route caps, preserving
payload hashes; reject mutating request bytes after hashing. Existing files must
be regenerated with the pinned adapter. Deduct the least reflected spend across
key/account usage; reject releasing settled reservations on a stale counter.
The previously recorded scope, permit lookup, parallel consumer classification,
reconciliation placement and artifact-test limitations remain outside this round.
No live call, provider route enforcement or current model availability is verified.

Focused regression evidence was replayed against `b09e99f` with implementation
code unchanged (the smoke adds only an argument-injection seam). The following
tests fail there and pass with the ceiling fixes:

| Finding | Focused regression |
| --- | --- |
| Model/input/output-dependent reservation | `g12_reservation_tracks_payload_and_explicit_output_before_dispatch` |
| Model pin and provider route price caps | `g12_model_price_caps_and_payload_bounds_fail_closed` |
| Entire smoke schedule must fit its cap | `g12_smoke_total_reservations_must_fit_before_policy_or_dispatch` |
| Invalid non-null limit with valid $20 credits | `g12_non_null_malformed_key_limit_never_falls_back_to_credits` |
| Settled spend with stale provider usage | `g12_settled_unreported_spend_is_subtracted_before_dispatch` |

Additional fixtures cover PNG headers, unsupported content, explicit output
bounds, exact-cap admission, malformed key accounting, partial/full usage
reflection, and returned cost overruns that remain charged and stop the campaign.
This evidence is limited to offline fixtures; no network, credential or live
model/provider validation was performed.

## Deferred reconciliation and dated revision identity (fix 3)

The coordinator's supplied evidence showed an unpublished generation at about
three minutes and a published record at about one hour. The recorded 200 shape
contains `google/gemini-3.8-flash-20260902`, `Google AI Studio`, a total cost of
$0.0023235 (2,323,500 nanodollars), 1,513 native prompt tokens and 212 completion
tokens. The fixture retains these fields with a synthetic generation ID; it is
not a live API capture produced by this implementation run.

Dispatch now leaves receipts pending. A later library call or smoke
`--reconcile-only` invocation updates the existing ledger without dispatch or
allowance. Unpublished/transient/deadline results stay pending and retain attempt
time and history. Billing/identity/revision mismatches remain failures. Only all
matched receipts make a campaign reconciled; repeated reconciliation skips
terminal receipts without additional reads. Original charges remain unchanged.

A reviewed revision may pin the dated model ID. That pin still requires an
absent fingerprint and matching alias at dispatch. Reconciliation records the
actual dated model and provider name; a changed dated model quarantines the
result from qualification even when cost matches. No pin is automatically
adopted, and no date or provider text is guessed from the request.

| Goal | Focused test |
| --- | --- |
| Deferred 404-to-200 lookup, attempt history, unchanged charges and idempotence | `g12_deferred_404_then_200_reconciliation_is_pending_and_idempotent` |
| Dated revision match versus drift quarantine, with absent-fingerprint dispatch checks | `g12_dated_revision_pin_matches_or_quarantines_after_reconciliation` |
| Reconcile-only arguments and refreshed exported receipts | `g12_reconcile_only_needs_no_allowance_and_preserves_exported_provenance` |

Decisions: reuse the locked campaign ledger and the existing bounded GET policy;
reject in-run publication waits, allowance resets, completion retries and silent
acceptance of dated drift. Keep dispatch fingerprint and reconciled dated identity
separate. This correction does not change scoring, batch, two-image token ceilings
or Gemini-direct. Offline fixture evidence does not qualify a live model or prove
live API availability. See [assist.md](assist.md#openrouter-provider-ceiling) for
the exact dispatch and later reconciliation commands.

Fix 3 validation (offline and locked Cargo resolution):

| Gate | Command or scope | Exit |
| --- | --- | --- |
| Formatting | `cargo fmt --all --check`; explicit `rustfmt --edition 2024 --check` on the ledger module | 0 |
| Strict clippy | `cargo clippy --workspace --all-targets --features assist -- -D warnings` | 0 |
| Core unit tests | `cargo test -p saccade-core --features assist,schema,evaluation --lib` (254 passed, 6 pre-existing ignored) | 0 |
| CLI unit tests | `cargo test -p saccade --features assist,schema,evaluation --bin saccade` (26 passed, 1 pre-existing ignored) | 0 |
| Smoke example tests | `cargo test -p saccade-core --features assist --example assist_openrouter_smoke` (5 passed) | 0 |
| Python assist tests | `python3 -m unittest discover -s scripts/assist -p 'test_*.py'` (14 passed) | 0 |
| Generated documentation | `python3 scripts/gen-docs.py --check` | 0 |
| Public hygiene | `bash scripts/check-public-hygiene.sh` (worktree and index) | 0 |

Cargo commands used `--offline --locked`, `CARGO_INCREMENTAL=0` and
`CARGO_PROFILE_DEV_DEBUG=0`. Final Rust test binaries ran with `HOME` and
`USERPROFILE` removed by the Cargo target runner, preventing ambient credential
or user-policy reads. Credential fixtures used synthetic keys in temporary
directories. No network or real credential access occurred. The initial focused
run exited 101 because the tiny synthetic request's conservative input bound was
below the recorded 1,513 prompt tokens; enlarging that fixture request corrected
the mismatch without changing production ceilings. The final focused run passed
all 21 G12 tests. These are unit and fixture gates; broader integration, ignored
heavy tests and live qualification were not run for fix 3.


## G12 image-token bound

The repository records no documented provider image-token rule for
`google/gemini-3.8-flash-20260902` or OpenRouter's declared image detail.
The existing `/1` documentation expressly describes local policy awaiting live
conformance. No provider documentation was fetched. Therefore `/2` is a
**calibrated local bound, not a documented tokenizer guarantee**; the durable
post-call breach stop is essential to its use.

Evidence: the coordinator's ten-call single-image ledger supplied on 2026-10-07
contains completed prompt counts of **1503–1543**, total reservations of
$0.238797 and billed cost of $0.0348585. The supplied brief summarized the range
as 1513–1539; implementation uses the larger observed maximum, 1543.
The ledger SHA-256 is `4c01b3edad8228b69500420d400af235f2477c9feec116223784b14d403b2dc4`.
These receipts are calibration evidence supplied by the coordinator, not new
live calls or model qualification performed in this lane.

Decision: `assist-image-ceilings/2` reserves
`2 * 1543 * ceil(width * height / 524288)` tokens per image, using decoded PNG
header dimensions. The safety factor is **2** against the largest *whole prompt*
count, including system and user text; serialized non-image UTF-8 bytes and
1024 framing tokens are additionally reserved. The area block is a local policy
choice, not a claimed provider tile size. It covers every constructed corpus
size: width `([240,320,480][family % 3] + jitter) * (1 + family % 2)`, jitter 0–39,
height `160 * (1 + family % 2)`, for families 0–31; maximum 1038 by 320.
Larger images scale in rounded-up area blocks. Edges above 2048, zero dimensions
and unknown resolution retain the 16384-token refusal fallback.

OpenRouter `detail:low` maps to low resolution, and high/auto/omitted detail maps
to high. Both get the full calibrated bound: no undocumented low-detail discount.
Gemini-direct and its declared medium resolution retain `/1`. The versioned
reader preserves `/1` exactly; live OpenRouter always selects `/2`. New receipts
retain `image_table` through settlement and deferred reconciliation. Historical
receipts lacking that field continue to use their recorded bounds and charges.
The price pin is unchanged: $0.75/M input and image tokens, $3.75/M output tokens.

For a corpus image the image component is **3086 tokens**, and two images use
**6172 tokens**, leaving respectively **12914** and **9828** tokens for the
serialized non-image payload plus framing under `INPUT_LIMIT=16000`.
With `max_tokens=4096`, the exact reservation formula is
`750 * input_bound + 3750 * 4096` nanodollars. The worst permitted reservation
for either a single-image or two-image request with arbitrary admitted text is
**$0.02736** (the INPUT_LIMIT-based cap). Reapplying `/2` to the supplied
single-image receipts gives input bounds 6240–6270 and maximum reservation
**$0.0200625**, down from $0.023892. No single observed prompt count is treated
as a guaranteed provider maximum.

At the maximum corpus dimensions, the normal prepared request regression reserves
**$0.020454** for single-image `check_ui` with the maximum 512-byte label condition
(input bound 6792), and **$0.02259225** for two-image `explain` in either blind order
(input bound 9643). These cases leave 9208 and 6357 tokens of margin respectively.
They are concrete prepared-payload reservations; larger admissible text/catalogs
remain subject to the $0.02736 absolute reservation cap.

Acceptance coverage:

| Requirement | Offline regression |
| --- | --- |
| Every corpus size, area transitions, resolution and fallback behavior | `g12_calibrated_table_covers_corpus_sizes_and_preserves_version_one` |
| Real PNGs at maximum corpus dimensions; single-image and both two-image explain orders admitted with at least 6000 input tokens of margin; reservation at or below the INPUT_LIMIT price cap | `g12_corpus_single_and_two_image_explain_reservations_fit_with_margin` |
| Actual prompt usage one token above the calibrated reservation, still below INPUT_LIMIT and below reserved cost, records `usage_limit_exceeded` and prevents a second dispatch | `g12_openrouter_reserves_settles_unknown_zero_and_quarantines_usage_breach` |
| Historical `/1` bound and receipt deserialize, verify and round-trip without revision or monetary changes | `g12_historical_version_one_reservation_and_receipt_still_verify` |
| Review denial is independent of real user configuration | `local_tools_and_preview_never_authorize_network_and_images_are_explicit` uses an empty user policy in an isolated temporary config directory |

Rejected alternatives: asserting an unavailable documented provider rule;
using only the brief's smaller count; discounting low-detail input without
provider evidence; globally reducing `/1` or changing Gemini-direct; repricing
historical receipts; accepting a breach because total cost or global input limits
still fit. Corpus construction, scoring and batch are outside this change.

Final offline validation (2026-10-07):

| Gate | Command | Result |
| --- | --- | --- |
| Formatting | `cargo fmt --all --check` | Exit 0 |
| Strict clippy | `cargo clippy --locked --workspace --all-targets --features assist -- -D warnings` | Exit 0 |
| Core feature suite | `cargo test --locked -p saccade-core --features assist,schema,evaluation` | Exit 0; 396 passed, 6 pre-existing ignored; schema conformance passed |
| CLI feature suite | `cargo test --locked -p saccade --features assist,schema,evaluation` | Exit 0; 198 passed, 24 pre-existing ignored |
| Smoke example | `cargo test --locked -p saccade-core --features assist,schema,evaluation --example assist_openrouter_smoke` | Exit 0; 5 passed |
| Full default suite | `cargo test --locked -p saccade-core -p saccade` | Exit 0; 617 passed, 25 pre-existing ignored; real user.toml present |
| Focused acceptance | `cargo test --locked -p saccade-core --features assist,schema,evaluation --lib g12_ -- --nocapture` | Exit 0; 24 passed |
| Python assist | `python3 -m unittest discover -s scripts/assist -p 'test_*.py'` | Exit 0; 14 passed against stable final source |
| Generated docs | `python3 scripts/gen-docs.py --check` | Exit 0 |
| Public hygiene | `bash scripts/check-public-hygiene.sh` | Exit 0; worktree and staged index |

Cargo used offline resolution, the required dedicated target, no incremental
compilation and no dev debug information. The largest observed target size was
6.10 GB, below the 8 GB limit; completed default-test executables were reclaimed
before subsequent builds. The target was removed after validation. Real user
configuration remained present and unmodified; the review denial regression
selects its own isolated empty policy instead of masking ambient configuration
for the whole suite. No external network calls or real key reads were performed.

An additional, broader **combined-package** run
`cargo test --locked -p saccade-core -p saccade --features assist,schema,evaluation`
remains **FAIL** on `saccade-quality-report.v1.schema.json` conformance.
Combining packages enables compression in the core schema suite; the unchanged
`report_links::extend_schema` adds optional `report_id`/`source_refs`, which the
committed quality-report schema lacks. The separate core feature suite's schema
checks passed. This broader configuration is not converted into PASS: the
quality/report linkage files remain untouched, and changing them is deferred
outside the image-bound envelope. No live model or provider qualification is
claimed by these offline tests.

## G12 stage 2 priced pilot (no-spend preparation)

`corpus.py --stage2` freezes the requested `wave4-constructed/2`,
`constructed-assist/2` profile, with campaign `g12-stage2/1`. This is a separate
profile retaining the current rendered oracle, safety checks and qualification
thresholds; it does not restore historical /2 behavior or relabel /3 fixtures.
It pins the supplied dated OpenRouter Gemini identity. No Jev identity or price
is inferred for dispatch. Development/calibration cases remain offline.

`scripts/qualify-wave4.sh --plan --stage2 --corpus DIR --out NEW_DIR` emits
requests and a priced plan without credentials/network. It includes reverse
orders and necessary-pixel counterfactuals, and excludes unpriced Jev arms.
Cascade uses deterministic routing and mechanics without dependent Jev support.
The single-image expectation uses the supplied ten-call aggregate; two-image
expectation adds a mean 1,523 prompt tokens at the pinned prompt price. Completion
usage is assumed unchanged; transfer to other workloads is not verified.
Reservations are derived from complete payloads and the existing image table.
The schedule shuffles cases and active arm/workload groups with seed 4406 and
interleaves one request per active group per round. Adding `--budget-bounded` to
the plan allows a larger schedule whose full reservation exceeds $5, while
retaining the same $5 maximum campaign allowance and 1,000-request runner limit.
The plan records whether the full reservation fits; it grants no dispatch authority.
The 95% bounds use independent challenge roots. The existing 99% qualification
threshold remains unchanged; this small pilot cannot qualify a model/workflow.

`scripts/qualify-wave4.sh --openrouter-stage2 --requests FILE --roots COUNT
--max-spend-usd 5 --user-policy FILE --out NEW_DIR` collects the bounded schedule
through the same smoke Executor, ceiling checks, monetary ledger
and reconciliation path. Infrastructure errors stop collection. It supports up to
1,000 requests with a six-hour campaign
deadline and a fresh executor deadline per root, bounded by both 300 seconds and
the remaining campaign time. The executor's 300-second guard remains enforced.
Campaign expiry stops new dispatch and records every remaining request as
`not_run_deadline`. Stage 2 cannot raise the $5 cap. Without `--budget-bounded`,
the full worst-case reservation must fit before policy/key access. With the flag,
each executor call must pass the existing monetary reservation and external
ceiling checks; a local allowance shortfall stops cleanly and records the current
and remaining requests as `not_run_budget`. Storage, ceiling, accounting, usage
breach and reconciliation failures retain fail-closed behavior. Partial collection
does not imply qualification. `--validate-only` validates every request and prints
integer per-request reservations before policy/key access, using the selected
all-fits or budget-bounded admission mode.
It validates the closed response protocol, geometry and citations and retains
answers beside responses/receipts. Independent-root scoring and semantic
order comparison remain separate; these artifacts cannot be passed off as the
synthetic offline scorer dialect or qualification evidence. Later use the smoke
`--reconcile-only DIR` command; there are no paid retries or automatic pin adoption.

The plan also records deterministic rules/unavailable/source-only outcomes in
`local-results.json`. After dispatch, use `python3 scripts/assist/stage2.py
--corpus CORPUS --requests FILE --results-dir DIR --out NEW_FILE` for conservative normalized
paired-order comparison. Missing answers and disagreement yield unverifiable;
the full scheduled root/arm denominator and unavailable request codes are reported,
including budget/deadline stops. Explicit unrun outcomes cannot be overridden by
stale answer files. This report does not score semantic truth or grant qualification.

Focused fixtures: `g12_stage2_per_root_deadlines_survive_pacing_and_stop_campaign`,
`g12_budget_bounded_schedule_reserves_until_next_shortfall_and_retains_tail`,
`test_stage2_replan_interleaves_pilot_and_budget_bounded_larger_schedule`, and
`test_budget_and_deadline_stops_remain_unavailable_in_full_denominator`. The fake
provider executor fixture also verifies that a six-hour executor deadline is
rejected before dispatch; the paced schedule uses a simulated clock, never sleeps.

## G12 explicit reasoning policy

Stage-2 plans now pin `assist-openrouter-provider-schema/1`: `reasoning.max_tokens` is
512 for `check_ui` (including routing) and 1024 for `explain`/`audit_mask`, inside
the existing 4096 aggregate output limit. Offline admission refuses omitted or
changed controls. OpenRouter's reported reasoning counter is a completion subset;
missing thinking usage stays unknown. `truncated_output` is an unavailable,
non-qualifying root outcome. Monetary breaches continue to stop the campaign and
retain billed spend. See [the decision and focused fixtures](design-decisions/g12-reasoning-budget.md)
for the exact request choice and offline evidence location.

## G12 provider-compatible structured output

The full closed answer schema feeds the Gemini payload and local validation of
every Gemini/OpenRouter answer. OpenRouter requests and Python stage-2 plans use
projection `assist-openrouter-drop-array-bounds/1`: recursively remove only
`minItems` and `maxItems`. Preserve `additionalProperties`, `anyOf`, enums,
required fields and numeric ranges. The versioned wire name is
`saccade_assist_answer_drop_array_bounds_v1`, with `strict: true` and
`require_parameters: true`; admission refuses any schema or format drift.
The projected format participates in payload hashes and input reservations.
Plans pin `response_format_hash`, `full_answer_schema_hash`, `schema_projection`
and request policy `assist-openrouter-provider-schema/1`.

Local validation uses the full shared schema, followed by the existing request,
citation, geometry and statement checks. Oversized observation/coordinate arrays
and the recorded `geometry.value` answer remain refused. The
[provider-schema decision](design-decisions/g12-provider-schema.md) records the
projection, refusal accounting, rejected alternatives and regression names.

HTTP error receipts and root outcomes expose only `http_status`,
`openrouter_error_code`, `provider_name` and `provider_status`, plus the
`zero_cost_refused` classification. Strings are ASCII charset-checked and capped
at 64 bytes; raw messages/bodies are never retained. A 4xx error object without
any generation, usage or completion evidence settles at zero and becomes a
terminal, non-qualifying `zero_cost_refused` receipt. Embedded raw error JSON
must also contain an error object without generation evidence. Generation
lookup is skipped for these receipts. Network failures, 5xx, malformed errors,
identifiers (including null IDs), and ambiguous embedded errors retain existing
cost handling, including charging the reservation when cost is unknown.

Regenerating and admitting both plans remains offline evidence only. The
coordinator supplied the compatibility probes; this change makes no provider
calls and establishes no fresh provider/model qualification.

## G12 answer-level protocol failures

Stage-2 collection now records settled successful responses with protocol errors
as `invalid_answer`, retaining the response, provenance and stable reason code
while dispatching the next request. These answers stay refused and cannot become
qualification eligible through reconciliation. Infrastructure, spend, identity
and storage errors still stop the campaign; truncation remains campaign-stopping.

The safety defaults stop above five consecutive invalid answers or above 50%
after 20 settled answers. Configure `--max-consecutive-invalid-answers`,
`--max-invalid-answer-percent`, and `--invalid-answer-min-sample`; the runner
records the effective policy. A valve stop retains every scheduled root and
marks the undispatched tail `not_run_answer_safety_valve`.

`stage2.py --corpus CORPUS --requests FILE --results-dir DIR --out NEW_FILE`
reports invalid rates and reason breakdown per arm/workload. Invalid orders and
descendants count as root/arm model failures in the complete denominator, never
as abstentions. Missing and disagreement counts remain separate. This is
protocol measurement only. See the [decision and focused regressions](design-decisions/g12-answer-failures.md).

## G12 pilot round 2: frozen geometry/citation prompt and resume

The new prompt epoch is `g12-pilot/2`, prompt policy
`assist-openrouter-geometry-citations/2`, corpus campaign `g12-stage2/2`,
constructed epoch `wave4-constructed/4`, and policy `constructed-assist/4`.
Encoder identity advances to `assist-encoder/4`. The provider schema projection,
reasoning hints/output bounds, statement vocabulary and all answer validators stay fixed.
Plans include the exact prompt hash and policy; request hashes include the prompt.

The system instruction explicitly defines normalized boxes as
`[x,y,width,height]`, with positive width/height, `x+width<=1` and `y+height<=1`;
points are `[x,y]`. It supplies one minimal valid observation for each of
`check_ui`, `explain` and `audit_mask`, and states that `evidence_refs` may only
cite regions of the same slot. Examples are protocol shapes, not truth labels.

The coordinator's epoch-1 pilot retains 60 dispatches: 44 completed, 15 invalid
answers (8 geometry bounds, 4 citation identity, 3 closed schema), then one
unknown-cost incomplete call; supplied known spend was $0.168273. A split-only
inspection found all eight geometry failures belong to the frozen **held-out**
split (indices 11, 13, 27, 28, 33, 51, 52, 58). Their response bodies and held-out
oracle labels were not opened. There are no supplied development-split geometry
responses with which to confirm corner-coordinate use. The spec's proposed cause
therefore remains unconfirmed; these clarifications follow the already enforced
protocol. No held-out tuning, validator relaxation or new live evidence is claimed.
The old pilot is retained as epoch-1 evidence and cannot resume under this prompt.

Executor calls now cap their remaining deadline at 120 seconds, while retaining
the 300-second caller guard and existing campaign deadline. HTTP transport errors
carry only a closed `transport_failure` class: `timeout`, `connect`, `reset`,
`tls`, or `other`. Typed transport errors and exact fixture codes select the
class; untrusted strings, endpoints, keys and error bodies never become diagnostic
text. Receipts and root outcomes retain the class. Unknown-cost failures continue
to stop dispatch and hold their full reservation.

`--resume EXISTING_DIR` replaces `--out NEW_DIR`; repeat the original requests,
roots, allowance, stage-2/budget mode and safety flags. The frozen campaign binding
includes request-file bytes, cache/prompt/policy identities, user-policy identity,
allowance and safety limits. Any change refuses resume. Legacy runs without this
binding are refused. One exclusive runner lock covers dispatch and reconcile-only.
The existing ledger and ceiling baseline remain authoritative; reopening never
resets money, request counters, safety history or a permanent spending stop.
Retries share the existing 1,000-attempt stage-2 limit (10 in small smoke mode).

Completed/invalid answers and proved zero-cost refusals are not dispatched again.
An incomplete dispatched root is retried only after authoritative generation
reconciliation matches its identity and settles its cost. An unknown incomplete
cost can now be settled from that proof, including a proved zero cost; only that
previously unknown charge is updated, with the original reservation and proof hash
retained. Known charges are never rewritten, discrepancies remain failures and
an overrun stops spending. Missing generation IDs are not proof of zero billing;
the old pilot's final receipt remains blocked by the existing missing-generation
rule. There is no manual zero-cost override or inferred settlement from account
balance changes.

Each reservation carries its durable root index before dispatch. Atomic root
checkpoints and separate `money-ID.json` exports retain all attempts, so a retry
cannot shift following receipts onto another root. Missing settled outcome records
or unknown/in-flight charges refuse resume conservatively; they are never replayed
as new paid calls. Safety-valve history includes the prior valid/invalid answers.
Reconciliation cannot make invalid answers qualification eligible.

Fresh seed-4406 plans retain the original topology: 216 pilot requests and 864
budget-bounded larger requests, each with a $5 allowance. The pilot reserves
$4.9695375; the larger schedule cannot reserve its full schedule within $5 and
continues to require `--budget-bounded`. Every Python reservation must match the
Rust offline admission. These are transport/admission fixtures, not qualification.

Operator command shapes (prepared only; no provider calls during implementation):

```sh
assist_openrouter_smoke --reconcile-only OLD_PILOT_DIR
assist_openrouter_smoke --stage2 --requests EPOCH2_PLAN/requests.json \
  --roots 216 --max-spend-usd 5 --user-policy USER_POLICY --out NEW_PILOT_DIR
assist_openrouter_smoke --reconcile-only NEW_PILOT_DIR
assist_openrouter_smoke --stage2 --requests EPOCH2_PLAN/requests.json \
  --roots 216 --max-spend-usd 5 --user-policy USER_POLICY --resume NEW_PILOT_DIR
```

Focused offline regressions:

- `test_round2_prompt_epoch_convention_examples_and_same_slot_citations`
- `g12_call_cap_120_and_transport_receipts_remain_unknown_cost`
- `g12_transport_classes_discard_untrusted_diagnostics`
- `g12_resume_identity_and_reconciliation_preserve_prior_spend`
- `g12_unknown_settlement_requires_identity_and_stops_on_overrun`
- `g12_resume_skips_settled_roots_blocks_unknown_and_preserves_attempt_mapping`
- `g12_resume_keeps_answer_safety_history_and_checkpoint_failures_stop`

Rejected alternatives: accepting corner boxes, permitting cross-slot citations,
inspecting held-out responses to tune examples, treating missing IDs as unbilled,
resetting allowances/counters on resume, and relaxing mismatches or permanent stops.


## G12 reasoning hints and runner identity

Requested reasoning budgets are provider hints. Only input above its reserved
bound, aggregate completion (including reasoning) above its reserved bound, and
actual cost above the reservation constitute known money-bound breaches.
Unknown cost stops the runner and retains the reservation for reconciliation.
The separate `provider_reasoning_over_hint` flag and requested/observed counts
never override content validation or identity quarantine. Missing thinking counts
remain unknown, not a measured below-hint call.

Stage-2 collection reports `provider_reasoning_over_hint_rates_per_arm_workload`:
known observations are the rate denominator; each row also retains the scheduled
request denominator and scheduled over-hint rate. Invalid answers and legacy
campaign failures contribute their known usage without becoming valid answers.

The epoch-2 pilot's seventh response/answer was never saved by the old executor.
Its bill is known, but token counters cannot reconstruct or validate its content.
The runner binding therefore advances from `saccade-g12-campaign/2` to `/3`;
request, prompt, encoder, provider, pricing, output ceiling and safety controls
stay fixed. The old durably stopped ledger is retained read only; no stop bit or
spend counter is cleared. Use a fresh campaign, then resume that campaign with
identical policy/plan/allowance. This is the spec's documented fresh-run fallback.
See [the exact commands and evidence](design-decisions/g12-reasoning-hint.md).

### G12 operator settlement exception and partial scoring

The explicit resume-only flag `--settle-unknown-at-reservation` accepts full
reservation charges for unknown receipts after frozen binding validation. It
never releases allowance or clears stops, never claims generation reconciliation,
and never redispatches the conservatively settled request. That request is
`not_run_transport_failure` and remains unavailable in quality denominators.
The default unknown-cost refusal and matched-incomplete retry behavior remain
unchanged. See [the decision](design-decisions/g12-unknown-settlement.md).

The offline `scripts/assist/pilot_score.py` adapter uses the original frozen
oracle/scoring functions and records input identities, independent root counts,
nominal one-sided 95% limits, invalid answers, over-hint rates, known costs,
unknown reservations and completed cold-call latency. Split results are separate;
no development/calibration provider performance is inferred from heldout calls.
The synthetic scorer and its qualification gates retain their original rules.

## G12 combined pilot scorer audit

`python3 scripts/assist/pilot_audit.py --corpus CORPUS --requests ORIGINAL_REQUESTS
--campaign ORIGINAL_REQUESTS FIRST_RESULTS --campaign SUBSET_REQUESTS SECOND_RESULTS
--local-results LOCAL_RESULTS --source-revision FROZEN_COMMIT --out SCORE_PREFIX`
combines existing campaigns offline. It verifies each campaign request-file hash,
canonical root binding and identical payload hash before opening answer files;
unknown roots, changed payloads, duplicate terminal dispatches, duplicate money
IDs and unbound receipts refuse combination. The original files remain read-only.
The frozen corpus adapter verifies the original source snapshot and requires
unchanged oracle, scorer, payload and receipt implementation files. The union
adapter has its own source hash in the report.

Selection is by canonical request identity, never response quality. A later paid
retry can supersede an incomplete call for answer scoring, but all physical
attempts remain in cost and request-rate denominators. Unknown charges retain
their full reservations. Scheduled request counts remain distinct from attempts.
Quality bounds use root events; correlated request bounds remain descriptive.
The report publishes aggregate metrics and combination reasons only, with no
individual held-out response, oracle answer or root result.

The supplied two campaigns execute **only held-out requests**. Development and
calibration each have 60 corpus roots, but no provider observations. Therefore
reviewed development/calibration disagreements are zero; empirical model-error
and scorer-artefact counts are **unavailable**, not zero. Their quality rates do
not measure model behavior. No held-out individual answer or oracle was inspected
to tune the scorer, prompt or policy. Automated oracle/response reads are used
only for the frozen aggregate score. No provider calls or key reads are needed.

No semantic scorer change is justified by these inputs. Source audit findings:

- `text:<literal>` compares exact code points, case, whitespace and line breaks.
  NFC conversion, whitespace folding or case folding would be a new protocol
  decision, not a repair authorized by the literal protocol. A multiline
  transcription cannot be adjudicated from a held-out example.
- Two-order arms require exact role-normalized observations, including geometry,
  evidence citations, visibility and uncertainty. Missing orders/descendants and
  disagreement withhold the root. Aggregate reasons distinguish missing samples,
  invalid answers and exact disagreement. `check_ui` uses one scheduled sample
  even in `two_gemini`; that arm name does not establish independent replication.
- `presence:present` can witness one visible glyph while `check_ui` task evidence
  requires complete literal text, absence with target coverage, or clipping.
  The prompt's presence example establishes a valid observation shape, not task
  success. `unverifiable` is an abstention only for a completed consistent root;
  invalid, missing and inconsistent answers are unavailable, not abstentions.
- Text requires every witnessed glyph pixel inside the exact half-open box.
  Absence/clipping/explanation require target coverage. Points use a one-pixel
  footprint for assertions; task coverage requires boxes. There is no IoU
  matching threshold, padding, snapping or rounding tolerance.
- `audit_mask` task evidence requires exclusion-ID citations while the live
  protocol permits only same-slot region citations. This incompatibility prevents
  task success; it cannot be counted as demonstrated model error. Repair requires
  a separately authorized development protocol epoch and independent evidence.

Focused synthetic tests use invented text and pixel witnesses, not copied pilot
answers. They lock exact Unicode/whitespace/case and geometry behavior, partial
presence versus task success, retry cost conservation, aggregate-only publication,
request-hash refusal and duplicate execution refusal. The combined pilot remains
unqualified. Do not proceed to run B: first obtain a development/calibration audit
under a separately reviewed protocol; do not adapt from this held-out pilot.

Identical payloads across distinct arm/order bindings share a hash. The union
indexes by payload hash **and** canonical schedule binding, retaining these
separate arm events instead of collapsing them or inflating independent support.

## G12 development/calibration pilot and consumed held-out draw

The first held-out draw (`wave4-constructed/4`, seed 4406, families 12–31)
was consumed by an unqualified pilot and is excluded from qualification claims.
It must not be reused as a prospective qualification holdout. No semantic scorer,
prompt, payload, price, model or provider policy is changed by this scheduling fix.

The planner now requires `--split development|calibration|held-out`, including
stage-2 plans. Missing split selection refuses before reading the corpus. The
scorer accepts the same split selection and verifies complete request and local
result topology for that split. Other splits remain unobserved.

The development and calibration plans each retain all 60 roots of the existing
frozen pilot corpus and the same four arms: 240 root/arm evaluations and 216
provider requests per split. Each full schedule has expected cost $0.890013600;
worst-case reservations are $4.969337250 for development and $4.970257500 for
calibration. These are full-schedule estimates, not a claim that all calls fit the
cap. Two independent budget-bounded campaigns each receive $0.75, enforcing a
combined $1.50 allowance. Admission verifies every request reservation offline;
execution will stop before admitting a request that exceeds remaining allowance.
Budget-stopped requests retain unavailable outcomes in the full denominator.
The expected-cost transfer from the earlier aggregate remains unverified.

Artifacts and exact run/reconcile/score commands are in
the external pilot artifact directory as `operator-commands.sh`.
The copied frozen binary has SHA-256
`2429c019e63e8c4179fc1fe54ddd771ad77046101791119cdd4e41d1054e365f`.
The original corpus is verified against source snapshot
`ab305c00bc4536d4ff482b5a3645fe5b67f9a6d4`; the binary is the existing
reasoning-hint pilot executor from the starting worktree revision. Frozen source
verification checks unchanged scoring, receipt and policy files, renderer and
payload functions and prompt/payload constants. Scheduling/freeze changes do not
silently admit altered answer semantics.

A new unscheduled draw, `g12-fresh-heldout/1`, uses seed 975031 and held-out
families 32–51, disjoint from every original family, seed and root. Its 45 held-out
roots have never received provider observations or individual human inspection;
automated construction and oracle verification are recorded separately from
observation. The manifest/oracle hashes and never-observed status are recorded in
the external pilot artifact directory as `fresh-heldout-status.json`.
This fresh pilot-sized draw does not establish sufficient qualification support.
No held-out request plan is generated. No provider calls or key reads occurred.

## G12 development audit and mandatory scorer proof (epoch 3)

Before any paid run, `scripts/assist/scorer_selftest.py --corpus
DEVELOPMENT_CORPUS --source-revision FROZEN_REVISION` must pass. Both the
qualification wrapper and the direct OpenRouter example enforce this gate before
credentials or campaign creation; missing or empty `SACCADE_SCORER_DEV_CORPUS`
or `SACCADE_SCORER_SOURCE_REVISION`, a failed proof, or an epoch-2 paid stage-2
request refuses execution. Offline `--validate-only` and reconciliation retain
their separate purposes. This audit made no provider calls and read no keys.

The gate builds **synthetic oracle-injected** answers for every development root,
workload and G12 arm, including unavailable roots and routing counterfactuals.
It checks the closed answer shape, same-slot declared citations and request hash,
then uses `dev_policy.normalize`, `pilot_score.semantic`, and the real atomic
assertion/task scorer. All four arms are synthetic injections: this establishes
scorer expressibility, not model, routing or runtime performance. Unavailable
roots correctly abstain; precision excludes them while important recall retains
every challenge root. Perfect answers must attain exactly 100% precision and
important recall with zero false reassurance for **every workload/arm**.
Missing answers, hallucinated facts, wrong literal text, wrong geometry and
conflicting order answers must have zero correct roots and be flagged in every
workload/arm. A gate test injects a broken task scorer and verifies failure.

The 60-root development proof has 96 workload/arm/variant rows. The repaired
policy passes all rows. Its legacy diagnostic shows perfect `explain` and
`check_ui` at 100% precision/recall, perfect `audit_mask` at 0%/0%, and routing at
75% precision with 100% important recall. These establish two protocol/oracle
mapping defects, rather than a need to relax correctness:

- The old mask scorer requires exclusion-ID citations, but the live protocol
  permits only same-slot region citations. New public regions explicitly map
  each exclusion to `P1:Rn` / `P2:Rn`. The epoch-3 normalizer preserves the region
  reference and adds its declared exclusion ID for task scoring. Each exclusion
  still needs an independently correct appearance assertion and full coverage;
  `R0` alone, wrong-slot references, points and undersized boxes cannot satisfy it.
- Routing controls ask for `non_overlap` while the old atomic oracle rejects
  `overlap:separate`. The new mapping independently checks the verified capture
  source bounds, requires both node citations and coverage of both nodes, and
  scores their strict rectangle intersection. Text alone cannot prove non-overlap.
  Unknown overlap statements without bound source evidence remain unsupported.

The policy is `g12-pilot/3`, prompt `assist-openrouter-task-evidence/3`, scorer
mapping `assist-region-exclusion-mapping/1`, implemented in `dev_policy.py`.
`plan.py --stage2 --task-evidence-policy` emits this policy; `pilot_score.py
--task-evidence-policy` checks its exact payload binding and refuses epoch-2
requests as epoch 3. The prompt defines pairwise appearance independent of slot
order, task-specific outcomes, required task evidence, and decorative changes
outside the target. It explicitly requires conservative enclosing answer boxes
when decimal rounding could trim a boundary. This is a model answer requirement,
not scorer padding. Literal text, Unicode, case, whitespace, exact glyph/target
coverage and exact normalized order agreement remain unchanged. Wider boxes
must still support every assertion over their declared extent. No IoU threshold,
rounding tolerance, fuzzy matching, dropped assertions or reduced quality gate
was introduced. Expected appearance statements come from independent comparisons
of oracle-bound rendered pixels, not from asking the scorer what it accepts. A
mutation regression inverts the appearance scorer and requires the proof to fail.
Focused tests use invented images, source packets and oracle
fixtures, never development response fragments.

Development inspection reviewed all 188 completed answers, including every
`explain`/`audit_mask` disagreement and all completed `check_ui`/routing answers.
There are 139 disagreeing completed responses: 75 primarily model error,
63 task-definition problem and 1 scorer artefact. Causes overlap: 104 include
model error, 63 task-definition problems and 1 the independently supported source
mapping artefact. The mask incompatibility affects 48 responses; independent-view
versus pairwise wording affects 7, missing pairwise evidence 8, and exact target
coverage fails in 36 `explain` responses. These are response counts, not independent
quality events. Only one observed scorer-artefact example exists; none were
invented to meet an example quota. The private audit provides five model-error
examples, five task-definition examples, that one scorer example, five order
examples and every individual comparison. Twenty-nine root/arms disagree across
orders: geometry in 23, statement in 21, role/citation in 20 each, observation
kind in 17, observation count/uncertainty/visibility in 13 each, outcome in 3.
These overlapping causes withhold roots; agreement requirements remain strict.

Replaying old development answers with the fixed source mapping and preserving
missing exclusion evidence gives the following precision / important recall:

| Workload | Single Gemini | Two Gemini | Cascade |
|---|---:|---:|---:|
| explain | 0% / 0% | 0% / 0% | unavailable / 0% |
| audit_mask | 0% / 0% | 0% / 0% | 0% / 0% |
| check_ui | 50.00% / 33.33% | 25.00% / 0% | 58.33% / 44.44% |
| routing | 18.18% / 11.11% | 9.09% / 11.11% | 45.45% / 22.22% |

False reassurance is zero for these available committed roots; missing/disagreeing
roots remain unavailable, not successes. Cascade `explain` has zero committed
roots, so its precision is unavailable rather than zero. These small correlated
development samples do not qualify any model. The old and replayed score reports
retain counts, nominal bounds, provenance hashes and unchanged billing receipts.

Decision **(b): a new development run is required first**. New prompt semantics
and exclusion regions cannot be retroactively supplied to old answers. The
216-request epoch-3 development schedule currently reserves **$5.14548975** at
pinned worst-case prices, exceeding both the development allowance and the $5
single-campaign ceiling. This is an offline reservation estimate, not billed or
expected cost, and the plan remains unauthorized. A later operator must choose
an admitted bounded schedule or separately authorized campaigns within the
existing ceilings; this audit authorizes neither. Fresh held-out qualification
must wait for the new development run and passing oracle proof.

The audit runner verifies the pinned renderer/source and opaque oracle hash,
lexically skips non-development oracle values without decoding them, and verifies
only development images/truths. It refuses a fresh-heldout corpus or a request
outside development before opening responses. Neither held-out responses nor
held-out oracle entries were observed; `g12-fresh-heldout/1` remains unobserved.
Private reports and request/proof artifacts stay outside the repository.

## G12 candidate-slot instruction (epoch 4)

The epoch-3 development smoke identified correct pairwise appearance assertions
attached to the baseline P1, causing task/order rejection despite correct raster
assertions. Epoch 4 (`g12-pilot/4`, `assist-openrouter-task-evidence/4`) explicitly
reports both `appearance:changed` and `appearance:unchanged` on the candidate,
with evidence references from that slot. Normal presentation uses the later/second
image P2. Reversed presentation uses P1 for the candidate; each request binds
`candidate_slot` into its request hash. This preserves the existing reversed-order
schedule and normalized candidate identity. Other statements remain per image.
The check_ui, explain and audit_mask examples state their slot and citations.

The scorer, oracle, geometry and order agreement rules are unchanged. The paid
executor requires epoch 4; epoch-3 requests must be regenerated. The mandatory
offline scorer proof passes all 96 rows on 60 development roots (synthetic
oracle-injected evidence, not model qualification). Five synthetic regressions
and 26 native executor tests pass. No provider calls, keys or held-out data were
used.

Artifacts: the operator-owned epoch-4 evidence directory. `commands.sh`
contains exact proof, smoke, reconciliation and scoring commands, including
`SACCADE_SCORER_DEV_CORPUS` and `SACCADE_SCORER_SOURCE_REVISION`. `show_smoke.py`
prints request-bound assertion/task verdicts; its ten synthetic checks pass.

| Plan | Roots | Calls | Worst reservation USD | Cap USD | Unverified cost proxy USD |
|---|---:|---:|---:|---:|---:|
| Development smoke | 10 | 10 | 0.252169500 | 0.252199500 | 0.046281000 |
| Targeted | 10 | 20 | 0.510225000 | 0.60 | 0.092562000 |

Both retain epoch-3 sizes, caps and selected schedules (five roots per workload).
The proxy is the inherited epoch-2 two-image estimate, not measured epoch-4 spend.
Plans remain unauthorized; no campaign was executed. Native validate-only
reservations agree exactly with the Python plans. The owned Cargo target is
removed after preserving the executable and receipts.
