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
still stops. Jev routing is not enabled pending its cost/recall qualification.
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

Live dispatch requires `--run`, source-root export permission and a separate
output root in the existing `~/.config/saccade/user.toml` policy. Keys come only
from `~/.config/saccade/{gemini,jev}.env`, with `SACCADE_GEMINI_API_KEY` and
`JEV_API_KEY`. Ambient keys and alternative credential directories are ignored.
Requested models are fixed to `gemini-3.8-flash` and `jev-1.13.0`.
`--gemini-revision` is required; `--jev-revision` defaults to the pinned Jev ID.
The coordinator must establish actual revision selectors with provider receipts.
Drift is refused rather than substituted. Provider errors are retained as bounded
local classifications, never echoed as instructions or raw secret-bearing text.

The envelope caps one entry at $0.15, at most eight actual requests (six by default, including exact input counting) and a shared overall
300-second deadline. Conservative reservations precede dispatch under the
existing locked ledger; absent/inconsistent usage remains unknown and consumes
its full allowance. Thinking tokens are billed without adding provider totals
again. Daily assist is capped at $5; prices expire on 2027-01-01. Payloads over
the conservative input-size limit are refused. Exact Gemini prompt counts are
checked before generation against 16,000 tokens. Count requests consume attempt
and monetary allowance. The brief does not establish token-count endpoint
billing: its full reservation remains consumed and Gemini aggregate cost remains
unknown until provider conformance establishes that price.
Unknown counting-price/revision behavior requires provider conformance before
qualification. No live calls run in development or ordinary tests.

MCP extends `saccade_review` with `explain`, `audit-mask`, `check-ui`. Use
`artifact` for the report/image, `out`, `experimental:true`, and the corresponding
CLI controls in snake_case; `box` is an integer array. Tool arguments cannot
increase human startup authorization. All nested source, replay and mask files
stay inside registered roots. The tool is annotated as potentially networked;
local/offline operations remain explicit. CLI and MCP share the implementation
and bounded result schema. Neither surface returns approval authority.

The core Batch interface separates immutable plan, durable submission intent,
operation receipt, status and collection. States are planned, submitted,
submission_unknown, pending, completed, partial and failed. An uncertain submit
cannot repeat silently. Exact frozen request hashes and revisions are checked
per item; job success does not make failed cells successful. Reservations remain
consumed when per-item billing is unknown. Interactive workflows never wait for
Batch completion. Qualification and heavy gate commands are described in
[constructed qualification](assist-qualification.md).
