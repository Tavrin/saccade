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

The coordinator runs `scripts/gates-wave4.sh` through the heavy queue. It checks
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
denominators. Reopening an epoch cannot raise its monetary allowance. Exact input
counting consumes reservations too. Counting API billing is currently unknown;
its conservative charge remains consumed and cost value gates stay unqualified
until that price has been established. Unknown thinking/usage is never zero cost.
Batch's public network CLI/MCP surface is deferred; interactive commands never
wait on an asynchronous evaluation job.

Six paired arms are recorded: rules, diagnostic oracle-text Jev, single Gemini,
two Gemini plus mechanics, two Gemini plus mechanics and Jev, and routed cascade.
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
