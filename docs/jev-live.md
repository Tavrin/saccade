# Jev under a local allowance

The `assist_jev_live` example is the explicit experimental Jev development path.
It requests `jev-1.13.0` and labels every receipt and report
`ceiling: local_allowance_not_provider_verified`. A local allowance does not verify
an account balance, invoice ceiling, credit exhaustion behavior, other clients,
or billing finality. No live qualification is established by the offline tests.
The unbudgeted Python adapter is retired. Interactive Gemini/Jev transport remains
disabled; the explicit Jev runner alone selects the local-allowance boundary.

## Admission and accounting

The operator must explicitly pass `--run --jev-prepaid-no-refill-attested` to assert
prepaid credits and disabled automatic refill. The default campaign allowance is
$1; the absolute maximum is $5. There are no retries, top-ups, fallback, implicit
resume, or parallel dispatches. A fresh campaign directory is required. Calls,
variants and workload descendants share the locked monetary ledger and campaign
scope. A schedule must fit in full before credential access. Each dispatch reserves
again under the shared ledger lock; concurrent ledger users cannot race admission.

Pricing is pinned to the public [Models documentation](https://docs.typesafe.ai/models),
observed 2026-10-07: $0.042 per million input tokens, output free. The policy expires
2026-11-07 at 00:00 UTC. Refresh requires a reviewed policy/source change and new
source-bound plans. Input admission counts every serialized UTF-8 payload byte as
one possible token and adds 4096 framing tokens, with a 32,000-token local maximum.
This intentionally conservative assumption is not a provider-certified billing
bound. It includes batched questions and instructions; `bytes / 4` is never used
for admission. Returned input usage above this bound stops further spending.

Returned usage settles as tokens times the frozen tariff, never an observed debit.
Missing usage settles at the complete conservative reservation and remains unknown
for cost-saving evaluation. Malformed or negative usage, identity drift and bound
breaches fail closed. HTTP errors and transport failures retain the full reservation;
unknown charging is never zero. Responses may expose a provider ID, but one is not
required or manufactured. Every attempt has a local ID, exact request and response
hashes, observed identity and usage metadata. Request/response bytes and receipts
remain in the operator's output directory. There is no provider billing-reconciliation
interface; receipts explicitly mark reconciliation unsupported and actual debit unknown.

Native identity requires the returned `model` to match `jev-1.13.0`. No undocumented
`modelVersion` is required. If present, it must match the expected revision. Otherwise
the exposed revision is the provider's versioned model ID; it is not a weights hash
or serving fingerprint. Encoder epoch 5 invalidates earlier assist caches.

## Constructed evaluation

`jev_eval.py` constructs deterministic exposure, shift, local edit, noise, config,
control, missing-evidence, contradictory and information-loss cases. An independent
numeric witness measures the generated before/after pixels, verifies intervention
consequences, and binds the witness bytes. Candidate native payloads contain only
structured evidence. Truth, intervention labels and scoring predicates live in a
separate oracle artifact. Two distinct hidden causes intentionally have identical
encoded evidence and require ambiguity. Causes remain proposals, never proven causes.

Five workloads have finite outputs: evidence routing, change classification,
cause-category proposals, priority and observation support. Arithmetic, validity and
threshold decisions remain code. Unsupported claims, missing necessary vision,
unavailable evidence and contradictions cannot become approval or exclusion authority.

Development uses five scene families, calibration another five, and held-out another
twenty. All descendants, option orders and evidence variants keep their root and split.
The scorer counts independent roots per arm; inconsistent order choices are withheld.
Frozen manifests bind generator/witness/encoder/scorer source hashes, corpus, oracle,
request topology, rubric, thresholds, model and tariff. The runner compares current
sources against its compiled-in hashes and repeats the actual Python scorer and Rust
native-parser self-tests before any credential access. A forged or stale self-test
receipt cannot authorize dispatch. The binary hash is retained in the campaign ledger.

Baselines/arms are deterministic rules, compact Jev, enriched Jev, oracle-text Jev
(diagnostic only), rules plus vision, vision plus Jev support, and cascade plus optional
Jev routing. The development schedule runs only compact/enriched text arms. Vision
arms need separately frozen paired vision outputs; no vision calls are prepared here.
Priority and explicit-flag classification may simply reproduce rules: that is not
incremental value. Candidate/oracle agreement is constructed-domain evidence only.

Preregistered gates in `POLICY` require availability >=95%, committed coverage >=60%,
one-sided precision lower95 >=95%, challenge-recall lower95 >=90%, zero control false
positives and zero unavailable commitments, order disagreement <=5%, and all family
gates with a fixed-seed clustered bootstrap. Cause ambiguity is reported separately.
Routing additionally requires necessary-vision-skip upper99 <=1%, cost reduction >=20%
and matched-coverage recall loss <=2 percentage points. Support needs >=30% fewer
unsupported assertions and <=5 percentage points of coverage loss against the same
vision baseline; zero baseline unsupported assertions makes incremental benefit
undefined. Unknown costs prevent cost-saving PASS. Cold-call p95 is frozen at 30 seconds;
queue, retries (zero here), call and workflow latency must be reported separately.
No model may approve, mutate exclusions or override deterministic stops.

Full held-out allocation is separately proposed at 600 challenge, 500 control and
500 unavailable roots per task across twenty families. At least 459 independent
necessary-vision cases with zero skips are needed for the stated 99% bound; correlated
families still require clustered uncertainty. Small development/self-test samples
cannot qualify all statistical, latency, real-world or incremental-value gates.

## Offline commands and prepared plans

Set `OUT` to an operator-owned output directory. These commands perform no HTTP and
read no credentials. Existing plan directories are refused rather than overwritten.

```sh
python3 scripts/assist/jev_eval.py --self-test --out "$OUT/selftest.json"
python3 scripts/assist/jev_eval.py --plan "$OUT/smoke" --smoke
python3 scripts/assist/jev_eval.py --plan "$OUT/development"
python3 scripts/assist/jev_eval.py --show "$OUT/smoke/requests.json"
cargo run -p saccade-core --features assist --example assist_jev_live -- \
  --requests "$OUT/smoke/requests.json"
cargo run -p saccade-core --features assist --example assist_jev_live -- \
  --requests "$OUT/development/requests.json" --max-spend-usd 0.50
```

Smoke schedules five calls per workload, 25 total, reserving **$0.00515361**. Hand-check
the printed evidence/choices before spending and inspect responses with the same show
script. Development schedules 50 roots per workload, two evidence arms and two option
orders: 1000 physical calls, reserving **$0.20677440**, below the **$0.50** allowance.
These are full local reservations; expected/actual debits are unverified. Both plans
stop at the finite five-minute campaign deadline; unfinished rows remain failures.

The following commands are prepared for a separately authorized operator run. They
were not executed to implement this path. The human-owned policy must authorize export
of the generated request file's root. Credentials remain in the fixed local key policy;
no key or provider body is printed. Each output campaign directory must be new.

```sh
cargo run -p saccade-core --features assist --example assist_jev_live -- \
  --requests "$OUT/smoke/requests.json" --out "$OUT/smoke-live" \
  --user-policy "$USER_POLICY" --campaign jev-development-smoke \
  --max-spend-usd 0.50 --run --jev-prepaid-no-refill-attested
cargo run -p saccade-core --features assist --example assist_jev_live -- \
  --requests "$OUT/development/requests.json" --out "$OUT/development-live" \
  --user-policy "$USER_POLICY" --campaign jev-development \
  --max-spend-usd 0.50 --run --jev-prepaid-no-refill-attested
python3 scripts/assist/jev_eval.py --show "$OUT/smoke-live/results.json"
python3 scripts/assist/jev_eval.py --score "$OUT/development-live/results.json" \
  --oracle "$OUT/development/oracle.json" --out "$OUT/development-score.json"
```

Self-test success proves local parser/scorer handling only: native envelopes without
synthetic IDs/revisions, correct/corrupt choices, distributions, numeric Noul, missing
or mismatched identity, missing/malformed/negative usage, unknown answers, option
permutations, contradictions, hidden-cause collisions, unsupported claims, missing
vision, reused receipts, changed hashes, unknown billing, timeouts, oversized payloads
and exhausted allowance. It makes no provider call and establishes no provider quality.
