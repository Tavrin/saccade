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
