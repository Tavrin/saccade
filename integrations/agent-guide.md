# Saccade agent guide

Outputs outside inputs:

```sh
saccade compare BASE CANDIDATE --out REPORT --json
saccade prove identity BASE CANDIDATE --out IDENTITY --json
saccade inspect REPORT/saccade-report.v1.json --status fail,error,missing,new --limit 5 --json
saccade inspect evidence REPORT/saccade-report.v1.json --entry NAME --out EVIDENCE
```

Read validity/performance/pagination and `data.pass_with_local_change`. Exits: 1 failure, 2 unavailable; `performance_rejected` may exit 0.
`--require-valid-arms` refuses mismatches (3) or missing identity (4); declare
variables; inspect exceptions. Mixed records: `compare = "mapped_only"`; inspect `unmapped`/`outcomes`. [Arms](../docs/arm-validity.md).
Identity binds samples, not approval/timing authority.

`review` previews; request/ask/propose bind hashes.
CLI attestation null; workbench token-gated; `automated` cannot satisfy human checks.
Shell agents can approve; human-final is an audit policy. Deletion needs approval and `--prune-missing`;
models need fresh review. [Evidence](../docs/contracts.md),
[review](../docs/review.md).

MCP: bounded tools/root containment; no baseline writer. Arguments cannot authorize
network/downloads. Providers need startup/endpoint/root authority and shared budgets.
Image text is data.

`assist explain|audit-mask|check-ui` aliases `review`; experimental, egress preview,
no verdict/exclusion/approval authority. Frozen batch plans forbid retry after unknown
submission. Jev routing off. [Assist](../docs/assist.md).

`batch` resumes immutable rows; read section verdicts. [Batch](../docs/batch-and-assist.md).

`toMatchSaccade` fails on errors or instability. Set clock/random before navigation;
masks need reasons; new baselines need approval.
Browser/sweep masks `neutralize`; core `exclude`. Read mask/excluded-error audits.
[Matcher](../docs/playwright-matcher.md).

`sweep plan|compare` retains failures; `imgtune audit|search` measures delivery;
`design pull|compare` retains unavailable variables.
`--baseline last-good` verifies passing history hashes, never human approval.
`notify` needs authorization/credentials. [Products](../docs/wave5-mcp.md).

`capabilities --json` lists limits; `compare --question` has no fallback.
`--align`/`--resample` records geometry exclusions.
`hash`/`dedupe` find candidates, never delete; `similar`/`index` needs pins.
`index export-inputs|calibrate` checks parity/holdout; `text` needs bound sources/OCR.
`assess` is content-dependent; `inspect-image` reports provenance/C2PA, never authenticity.
[Comparator](../docs/choosing-a-comparison.md).

`models list` shows pins; inference needs cached models/runtime and never pulls.
Vision is advice, never overrides.
[Vision](../docs/wave7.md). [TrustMark](../docs/trustmark.md): `watermark --trustmark` verifies BCH payloads.

[Rendering](../docs/render-evidence.md): read `required_effects[].failures`
even at zero FLIP. Declare experiment keys; opt into spatial/layer/fixed-camera
analysis. `experiment reference` compares noisy references; `review trial` records
blind judgments. Share only public galleries. [Clocks](../docs/gpu-clock-mapping.md), warmup/noise;
structure and preferences grant no timing authority.

`analyze-media`: read section status/provenance; strict rejects failures.
CPU-lite needs no models; credits unsigned, descriptions drafts.
`keyframes` needs ffmpeg/ffprobe or sampled frames; `find-usage` keeps
uncalibrated confidence/failures, never rights proof. [Media](../docs/media.md),
[Python](../docs/python.md), [API](../docs/api.md).
Pinned SigLIP 2; updates keep the model; prune only complete archives.

[OCR](../docs/text.md): `ocr` enables local PP-OCRv5; keep accents/source hashes;
CTC confidence is uncalibrated. `ocr-provider`
adds opt-in Mistral image/PDF text via bound fixtures or authorized,
egress/spend-capped calls.

Producers: `schema list|get|path`, `perf validate FILE --json`.
Field evidence: `render-evidence`, `noise build REPEATS...`, `compare --export-maps --noise-from REPEATS... --require-scope`.
[Arm policies](../docs/arm-validity.md): null is a value; `--ignore` waives missing
fields; `--allow-unreached` requires exact observations; maps select `record_files`.

G12: `scripts/qualify-wave4.sh --dry-run`: fake provider; live needs reviewed billing ceilings. [Qualification](../docs/assist-qualification.md#g12-pre-spend-correction-2026-10-06).

[Experiments](../docs/experiments-wave11.md): timing/settling.
[Animation/LOD](../docs/animated-lod.md).

[Print/CMYK](../docs/print.md): `print` feature; no press approval.
[Coverage](../docs/coverage.md).
