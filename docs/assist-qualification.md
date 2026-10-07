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
