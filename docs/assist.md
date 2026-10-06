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
credits alone. Neither parseable means `openrouter_ceiling_unavailable`.
The allowance must fit the ceiling. Before every dispatch, after pacing, another
fresh read checks the outstanding reservations (including this request) and both
usage deltas against our settled spend plus a fixed $0.000001 tolerance. A missing
check, insufficient remaining balance, regressing usage or another consumer stops
the campaign, recording the reason. The ledger lock serializes these checks;
crashed and unknown-cost reservations remain nonzero. USD decimals are parsed
without floating-point rounding at the accounting boundaries.

Every request includes `usage: {"include": true}`. Returned USD cost settles the
original money receipt even when the answer fails validation. End-of-campaign
`/api/v1/generation?id=` reads attach total cost and response hash to each receipt;
unknown/missing generations or cost differences above one nanodollar are recorded
failures and stop spending. Failed calls and artifact-write failures also reach
reconciliation. Process crashes retain durable reservations for operator review.
The external remaining ceiling is distinct from the local campaign allowance;
local token-price estimates alone do not prove a provider invoice bound.

The request file is a JSON array of objects with exactly `root`, `model`,
`revision` (expected returned fingerprint), and `payload` (the existing adapter's
closed chat-completions shape). Ten distinct roots are required for `--roots 10`.
Every payload requires a namespaced model, messages, temperature zero, bounded
`max_tokens`, JSON-object output, disabled routing fallbacks, required parameters
and included usage accounting. This runner does not consume oracle answers,
generate corpora, score outputs or confer immutable model identity.

```sh
cargo run --locked -p saccade-core --features assist --example assist_openrouter_smoke -- \
  --requests /path/to/reviewed-openrouter-requests.json --roots 10 \
  --max-spend-usd 1 --user-policy ~/.config/saccade/user.toml \
  --out /path/to/new-openrouter-smoke
```

Every CLI rejects allowances above $25 by default. The smoke's separately named
`--allow-spend-above-25-usd` flag allows up to the existing $30 campaign parent;
it never bypasses provider checks. No verified `:batch` model/compatible shape was
supplied, so batch arms are omitted and `:batch` requests are refused. No separate
batch API is invented. All lane evidence is synthetic/in-memory; endpoint
compatibility, current model availability and actual provider enforcement require
the coordinator's reviewed live smoke.
