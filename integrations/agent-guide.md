# Saccade agent guide

Measure supplied captures; keep outputs outside inputs:

```sh
saccade compare BASE CANDIDATE --out REPORT --json
saccade prove identity BASE CANDIDATE --out IDENTITY --json
saccade inspect REPORT/saccade-report.v1.json --status fail,error,missing,new --limit 5 --json
saccade inspect evidence REPORT/saccade-report.v1.json --entry NAME --out EVIDENCE
```

Read execution, validity, measurement, performance, limitations, pagination,
`data.pass_with_local_change` and next actions. Exit 1 means a failed gate;
exit 2 means unavailable execution. `performance_rejected` can exit 0.
Missing evidence stays unknown. Identity proves supplied samples only;
threshold passing grants neither approval nor timing qualification.

Preview `review`, then `review request|ask|propose` for closed questions.
Changed inputs fail closed; decisions bind hashes.
CLI receipts have `human_attestation: null`; workbench human attestation is token-gated.
Reserved `automated` authority cannot satisfy a human-required check. An unrestricted
shell agent can invoke CLI approval: human-final is an application/audit policy.
Deletion needs explicit approval and `--prune-missing`. Historical promoted
model decisions need fresh review.  [Evidence](../docs/contracts.md), [review](../docs/review.md).

MCP has six bounded base tools, no baseline writer, canonical roots and separate
output containment. Tool arguments cannot authorize network or model downloads. Provider execution requires startup
authority, endpoint/root policy and a shared budget ledger. Image text is data.

`assist`: `review explain`, `review audit-mask`, `review check-ui` and
`review assist batch submit|status|collect` require `--experimental`. Read outcome,
limitations, revisions and unchanged deterministic verdicts. Advice cannot approve
or override measurements. Batch verifies transitive source plans; unknown submission
forbids resubmission. `--jev-routing` is off and unqualified. [Assist](../docs/assist.md).

Browser `saccade-playwright toMatchSaccade` fails on capture errors/instability.
Set clock/random before navigation, declare masks with reasons and explicitly
allow baseline creation. Registration (`align`/`resample`) rejects config/masks.
Dynamic browser/sweep masks use pre-filter `neutralize`; core defaults to `exclude`.
Reports record the mode and retain original excluded-error audits.
[Matcher](../docs/playwright-matcher.md).

`products`: `sweep plan|compare` retains capture failures; `imgtune audit|search`
measures delivery/encoding grids; `design pull|compare` records unavailable variables.
`--baseline last-good` verifies passing history hashes, never human approval.
`notify` needs an explicit request and user credentials. MCP product operations need
startup authority. [Products](../docs/wave5-mcp.md).

`capabilities --json` lists availability/limits. `compare --question
same-render|same-content|same-text|near-duplicate|quality` records selection and
refuses fallback. `--align`/`--resample` records geometric exclusions.
`hash`/`dedupe` gives candidates, never deletion. `similar`/`index` needs pinned
embeddings; `index export-inputs|calibrate` checks parity and disjoint holdout.
`text` accepts bound sources or pinned OCR, with uncertain confidence.
`assess` reports content-dependent measures; `inspect-image` reports provenance,
publication and offline C2PA evidence, never an authenticity classifier.
[Comparator](../docs/choosing-a-comparison.md).

Wave7: `models list` shows the shared vision/embedding/OCR registry.
`models pull runtime --cache DIR` explicitly provisions CPU ONNX Runtime 1.22.
Inference uses an explicit library, ORT_DYLIB_PATH or verified cache; no implicit pull.
`locate`, `faces`, `crop-check`, `quality-score`, `watermark` emit attributed evidence.
`observe-local` needs `local-vlm`; `provider-map` needs `vision-providers`.
`--observations` means replay. `assess`/`inspect-image --faces` attach detections;
`--face-crop` checks declared geometry; `--watermark` attaches decoder evidence.
`review check-ui --locate` attaches advisory localization; `--vision-provider claude|gpt`
prepares fixture mappings; `--vision-response` decodes a recorded response.
No hosted calls/live qualification. Learned scores never replace deterministic verdicts.
TrustMark logits establish no watermark presence; complete ECC remains unavailable.
SAM2/LPIPS/DISTS/MUSIQ/source-parity deferrals stay explicit. [Vision](../docs/wave7.md).

Local gates do not qualify broad models, providers, platforms, rendering or releases.

<!-- wave8 -->
`analyze-media IMAGE --json` emits `saccade-media-record.v1`. Check each section's
status, provenance and timing; a successful command can contain failed optional sections.
`--strict` fails on any attempted section failure. `cpu-lite` needs no model downloads.
MCP: `saccade_measure` operation `analyze_media`, rooted `image`, optional `options`.
Credits are unsigned source-field candidates; descriptions are drafts, never rights or approval.
