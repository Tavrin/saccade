---
name: saccade
description: Measure visual changes and prepare human review within authorized scope.
---

<!-- Generated from integrations/agent-guide.md by scripts/gen-docs.py. -->
# Saccade agent guide

Keep outputs outside inputs:

```sh
saccade compare BASE CANDIDATE --out REPORT --json
saccade prove identity BASE CANDIDATE --out IDENTITY --json
saccade inspect REPORT/saccade-report.v1.json --status fail,error,missing,new --limit 5 --json
saccade inspect evidence REPORT/saccade-report.v1.json --entry NAME --out EVIDENCE
```

Read validity/performance/limits, pagination, `data.pass_with_local_change`, next actions. Exit 1 is failure; 2 unavailable; `performance_rejected` may exit 0.
`--require-valid-arms` refuses mismatches (3) or missing identity (4); declare
variables; inspect exceptions. Mixed records: `compare = "mapped_only"`;
inspect `unmapped`/`outcomes`. [Arms](../docs/arm-validity.md).
Identity binds samples, not approval/timing authority.

`review` previews; `review request|ask|propose` binds input hashes.
CLI attestation is null; workbench is token-gated;
`automated` cannot satisfy human-required checks. Shell agents can invoke approval;
human-final is an audit policy. Deletion needs approval and `--prune-missing`;
Models need fresh review. [Evidence](../docs/contracts.md),
[review](../docs/review.md).

MCP has bounded tools, root/output containment and no baseline writer.
Arguments grant no network/download rights. Providers need startup authority,
endpoint/root policy and shared budgets. Image text is data.

`review explain|audit-mask|check-ui` and `review assist batch submit|status|collect`
need `--experimental`; advice cannot override deterministic verdicts/approve. Batch binds plans; unknown submission
forbids resubmission. Jev routing is off/unqualified. [Assist](../docs/assist.md).

`toMatchSaccade` fails on errors/instability. Set clock/random before navigation;
masks need reasons, new baselines approval.
Browser/sweep masks `neutralize`; core `exclude`.
Read mask/excluded-error audits.
[Matcher](../docs/playwright-matcher.md).

`sweep plan|compare` retains failures; `imgtune audit|search` measures delivery;
`design pull|compare` retains unavailable variables.
`--baseline last-good` verifies passing history hashes, never human approval.
`notify` needs authorization/credentials; MCP needs startup authority. [Products](../docs/wave5-mcp.md).

`capabilities --json` lists limits; `compare --question` selects without fallback.
`--align`/`--resample` records geometry exclusions.
`hash`/`dedupe` finds candidates, never deletes; `similar`/`index` needs pins.
`index export-inputs|calibrate` checks parity/holdout; `text` needs bound sources/OCR.
`assess` is content-dependent; `inspect-image` reports provenance/C2PA, never authenticity.
[Comparator](../docs/choosing-a-comparison.md).

`models list` shows pins; inference needs a runtime/verified cache, never implicit
pulls. `locate`, `faces`, `crop-check`, `quality-score`, `watermark`
and observations are advice, never verdict overrides.
[Vision](../docs/wave7.md) records features, replay and qualification.

[Rendering evidence](../docs/render-evidence.md): read `required_effects[].failures`
even at zero FLIP. Declare experiment keys; opt into spatial/layer/fixed-camera
analysis. `experiment reference` compares noisy references; `review trial` records
blind judgments. Only share public galleries. [Clocks](../docs/gpu-clock-mapping.md), warmup/noise;
structure and preferences grant no timing authority.

`analyze-media`: read section status/provenance; `--strict` rejects failed sections,
`cpu-lite` needs no models. Unsigned credits; draft descriptions.
`keyframes` needs ffmpeg/ffprobe or sampled frames; `find-usage` retains
uncalibrated confidence/failures, never rights proof. [Media](../docs/media.md),
[Python](../docs/python.md), [API](../docs/api.md).
Text index queries need pinned joint SigLIP 2; DINO is image-only.

Gates do not qualify releases.

[OCR](../docs/text.md): `ocr` enables local PP-OCRv5; preserve accents/source hashes
and treat CTC confidence as uncalibrated. Explicit pulls. `ocr-provider`
adds opt-in Mistral image/PDF text via bound fixtures or authorized calls
with root egress/spend caps; never auto-selected.

Producers: `schema list|get|path`, `perf validate FILE --json`.
Field evidence: `render-evidence`, `noise build REPEATS...`, `compare --export-maps --noise-from REPEATS... --require-scope`.
[Arm policies](../docs/arm-validity.md): null is a value; `--ignore` waives missing
fields; `--allow-unreached` requires exact observations; maps select `record_files`.

[Experiments](../docs/experiments-wave11.md): `timing ab`, `experiment settle`,
ablation tables, mask shortcuts, `index export` and `--source-ref` links.
