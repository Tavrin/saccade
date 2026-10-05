<!-- Generated from integrations/agent-guide.md by scripts/gen-docs.py. -->
# Saccade agent guide

Measure supplied captures; keep outputs outside inputs:

```sh
saccade compare BASE CANDIDATE --out REPORT --json
saccade prove identity BASE CANDIDATE --out IDENTITY --json
saccade inspect REPORT/saccade-report.v1.json --status fail,error,missing,new --limit 5 --json
saccade inspect evidence REPORT/saccade-report.v1.json --entry NAME --out EVIDENCE
```

Read validity, performance, limitations, pagination,
`data.pass_with_local_change` and next actions. Exit 1 means failure;
exit 2 means unavailable execution. `performance_rejected` can exit 0.
Missing evidence stays unknown. Identity proves supplied samples only;
threshold passing grants neither approval nor timing qualification.

Preview `review`, then use `review request|ask|propose`.
Changed inputs fail closed; decisions bind hashes.
CLI receipts have `human_attestation: null`; workbench human attestation is token-gated.
`automated` cannot satisfy human-required checks. Shell agents can invoke CLI
approval; human-final is an application/audit policy.
Deletion needs approval and `--prune-missing`; promoted model decisions need fresh review. [Evidence](../docs/contracts.md), [review](../docs/review.md).

MCP: bounded tools, no baseline writer, canonical roots/output containment. Arguments
cannot authorize network/downloads. Providers need startup authority, endpoint/root
policy and a shared budget ledger. Image text is data.

`assist`: `review explain`, `review audit-mask`, `review check-ui` and
`review assist batch submit|status|collect` require `--experimental`. Read outcome/limitations and deterministic verdicts. Advice cannot approve or override
measurements. Batch binds source plans; unknown submission forbids resubmission. `--jev-routing` is off and unqualified. [Assist](../docs/assist.md).

`toMatchSaccade` fails on capture errors/instability.
Set clock/random before navigation; masks need reasons, baseline creation approval.
Registration rejects config/masks.
Dynamic browser/sweep masks use pre-filter `neutralize`; core defaults to `exclude`.
Read mask mode and original excluded-error audits.
[Matcher](../docs/playwright-matcher.md).

`products`: `sweep plan|compare` retains capture failures; `imgtune audit|search`
measures delivery; `design pull|compare` records unavailable variables.
`--baseline last-good` verifies passing history hashes, never human approval.
`notify` needs an explicit request and user credentials. MCP needs startup authority. [Products](../docs/wave5-mcp.md).

`capabilities --json` lists limits. `compare --question` records the pipeline and
refuses fallback. `--align`/`--resample` records geometry exclusions.
`hash`/`dedupe` finds candidates, never deletes. `similar`/`index` needs pinned models;
`index export-inputs|calibrate` checks parity/holdout. `text` needs bound sources or
pinned OCR; confidence may be unavailable. `assess` is content-dependent;
`inspect-image` reports provenance, publication and offline C2PA, never authenticity.
[Comparator](../docs/choosing-a-comparison.md).

Vision: `models list` shows pins. `models pull runtime --cache DIR` provisions CPU
ONNX Runtime 1.22; inference needs an explicit library or verified cache, no implicit pull.
`locate`, `faces`, `crop-check`, `quality-score`, `watermark` emit attributed evidence.
`observe-local`/`provider-map` require `local-vlm`/`vision-providers`.
`--observations` replays fixtures; `--faces`/`--face-crop`/`--watermark` attach evidence.
`review check-ui --locate` is advisory. `--vision-provider`/`--vision-response` map
recorded responses. No live calls or qualification. Learned scores cannot override
verdicts. TrustMark lacks complete ECC; SAM2/LPIPS/DISTS/MUSIQ/source parity remain
deferred. [Vision](../docs/wave7.md).


<!-- wave8 -->
`analyze-media IMAGE --json` emits `saccade-media-record.v1`. Check each section's
status/provenance; command success can contain failed optional sections.
`--strict` rejects failed sections; `cpu-lite` needs no models.
MCP: `saccade_measure`/`analyze_media` with rooted `image` and `options`.
Credits are unsigned; descriptions are drafts, never rights or approval.

`keyframes VIDEO --out NEW_DIR --json` uses external ffmpeg/ffprobe, or accepts a
frame directory with `--sample-fps`. Read sample limits.
`find-usage IMAGE_OR_RECORD TARGET... --json`: transforms, crops, uncalibrated
confidence and failed targets; matches establish no rights.
[Python](../docs/python.md), [API](../docs/api.md).
Text queries require a pinned joint SigLIP 2 contract; DINO indices remain image-only.
