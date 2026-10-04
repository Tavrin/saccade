# R12 paid-tier resume decisions

The binding brief is `SPEC-r12-paid-resume.md`. The frozen manifest, case split, truth, rubrics, job plan, and provider payload encoding remain unchanged. The published amendment starts `r12-paid-tier/1` with a 12-hour elapsed window. The ledger at the boundary held 151/400 Jev and 122/250 Gemini attempts, leaving 249 and 128 attempts. User-level Gemini pacing is 60 requests per minute and four in flight, both provider-wide and per model. Jev keeps its default. Credentials remain in the production key reader's dedicated env files.

## Private data relocation

The prior worktree's ignored storage alias was absent. The existing private preparation copy still had every generated recorded packet. The resume copied only job receipts, the ledger, mapping, and run state into ignored local storage. It remapped the private mapping and transitive egress roots to the existing preparation copy; no packet bytes or constructed truth were edited. The amendment records the two local adapter identities required for this relocation.

Another 244 recorded upstream source files are absent from their original archive locations. The validator rehashes every frozen generated packet and every available source, but cannot rehash those missing originals. Section 16 authorizes this corpus's private provider egress. The transport retains unavailable original root identities in private receipts and authorizes the extant derived case folder that supplies the payload. This is a source-revalidation shortfall, not evidence of a changed packet. The public result reports it explicitly; no qualification gate is lowered.

## Probe reset

The copied ledger inherited free-tier next-probe times for all four Gemini models. A first paid pass made zero new attempts. Before any paid dispatch, the copied Gemini probe cooldowns were cleared, while all attempt receipts and provider caps were retained. The original private ledger and responses were not changed.

## Price and gates

Paid-epoch Gemini spend is estimated from API `usageMetadata`: input tokens at the published standard paid-tier input price, and output as total minus input tokens so thinking tokens are included. Successful responses without usage are counted separately. The result labels this an estimate rather than an invoice. The fixed §9 gates still require 50 truth-known held-out cases per question, a declared important-miss tolerance, matching qualified calibration, and 95% attempted completion. The source-revalidation gap is an additional limitation.

## Final outcome

All 520 scheduled jobs were processed. The ledger ended at 389/400 Jev and 250/250 Gemini attempts; this paid epoch used 238 and 128 attempts respectively. Of the jobs, 514 answered, one Gemini response remained invalid at the frozen output limit, one Gemini job was blocked by the cap, and four enriched Jev jobs lacked their shared visual observation. Thus five provider jobs remain unresolved, plus one terminal invalid response. The Gemini spend estimate is $3.456622 USD from 128 paid-epoch responses with usage metadata. All four pinned model probes were valid in both orders (4/4 each), so the newest-first chain remains the operational default. This corpus supports no quality-based model ranking.

No question qualifies. Four questions have 20 truth-known held-out cases each and need 30 more; `perf.interpret.v1` has two and needs 48 more. Important-miss tolerance is still undeclared, and no calibration identity is qualified. Formatting, Clippy, Rust tests, runnable evaluation Python tests, the lane verifier, and the public-path scan passed. Eight superseded preparation tests were skipped because their private fixture is absent from this checkout. The required Cargo target was removed after testing.
