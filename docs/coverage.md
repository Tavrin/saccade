# Coverage, variants and baseline health

One `saccade-manifest.v2` drives a grouped view of declared cases, variant axes
and reference health. Each case remains a row when both inputs are absent, when
capture is refused, or when its report cannot be resolved. Completeness covers
only declared cases; it does not establish domain coverage.

```sh
saccade manifest build reports --cases cases.json --json
saccade manifest views reports --group-by page,viewport --out coverage --json
saccade manifest views reports --group-by language,size --out by-asset --json
```

`manifest build` without `--cases` still writes v1. With `--cases` it writes the
v2 successor, preserving v1 artifacts, report-ID index, counts and approval roles.
Existing v1 manifests can be upgraded explicitly. Rebuilding v2 without cases
is refused to prevent declaration loss. `manifest verify` and `manifest link`
work with v2. No `batch` command is required.

The declaration input is
[`saccade-cases.v1`](../crates/saccade-core/schemas/saccade-cases.v1.schema.json):

```json
{
  "schema": "saccade-cases.v1",
  "axes": {"page": ["start"], "viewport": ["narrow", "wide"]},
  "cases": [
    {"case_id": "start-narrow", "variants": {"page": "start", "viewport": "narrow"},
     "required": true, "baseline": null, "capture": null,
     "approved_anchor": null, "last_good": null, "approved_at_unix": null,
     "refusal": null, "latest": null, "anchor_comparison": null,
     "last_good_comparison": null},
    {"case_id": "start-wide", "variants": {"page": "start", "viewport": "wide"},
     "required": true, "baseline": null, "capture": null,
     "approved_anchor": null, "last_good": null, "approved_at_unix": null,
     "refusal": "producer could not acquire this variant", "latest": null,
     "anchor_comparison": null, "last_good_comparison": null}
  ]
}
```

Every case declares one value for every axis. Allowed values constrain the
vocabulary; only the explicit case list defines expectations, not a Cartesian
product. Duplicate case IDs and undeclared axis values are errors. Filenames
never imply size, language, scene or theme. Several cases may intentionally use
the same bytes or comparison.

For available inputs, replace null with `{"path":"inputs/example.png",
"sha256":"<64 lowercase hex characters>"}`. Paths resolve relative to the
output manifest, even if `cases.json` is elsewhere. Missing files can also have
recorded references; they remain missing rows. SHA-256 binds encoded bytes.
Changed bytes are stale; unreadable or oversized inputs are unavailable. Reads
are capped at 64 MiB, declarations at 20,000 cases and axes at 32.

A comparison reference is `{"report_id":"sha256:<64 lowercase hex characters>",
"entry":"exact report entry name"}`. `latest`, `anchor_comparison` and
`last_good_comparison` resolve through the existing manifest `reports` index.
The reader verifies report bytes and report identity, then binds the entry's
baseline/capture hashes to the declared role and current capture. Missing,
changed, invalid, ambiguous or mismatched evidence never supplies a measured
verdict. Pair report v1 and its linked v2 successor are supported. Other report
families remain unavailable rather than being guessed into pair evidence.

The HTML uses the shared report/view design system and links back to existing
pair reports/viewers. Its navigation exposes coverage, variant groups and
baseline health. `coverage.json` is
[`saccade-coverage.v1`](../crates/saccade-core/schemas/saccade-coverage.v1.schema.json),
with every case, group counts, role-specific measurements and health findings.
`--json` returns a bounded summary and an artifact reference. Exit 0 means all
required cases have measured coverage; exit 1 means incomplete coverage; exit 2
is a configuration, input or output error. A measured failure counts as measured
coverage. Its failure verdict remains visible. Advisory cases remain rows but
do not block required completeness. Reference health findings are informational
and never grant approval or silently change references.

The approved-anchor column describes cumulative drift. The last-good column
describes operational continuity. A last-good pass and anchor fail can coexist.
Only an explicit `approved_anchor` declaration records approval; report passing
or content hashes never infer it. Case references are explicit overrides for
that variant; directory-level approval is preserved as metadata and is not
applied to every case. No command writes or approves baseline bytes.

Health includes unavailable/stale baselines and approved anchors,
`never_approved` (no approval declared), old known approvals, unknown approval
age, missing/old measured runs, and future timestamps. Absence of approval
metadata does not prove historical non-approval. `approved_at_unix` is an
explicit approval timestamp, separate from report generation time. Use
`--now-unix` and `--max-age-seconds` to reproduce findings; defaults are current
Unix time and 30 days. At the exact age limit evidence is still recent.

Documentation screenshots and light/dark states are ordinary declared cases
with axes such as `purpose`, `state` or `theme`. Historical screenshots can be
advisory (`required: false`). Theme coverage establishes declared input and
measurement presence only. Source presence, visible rendering and contrast
need their own evidence; the view does not infer semantic correspondence.

Generate and assert the three proof families without network access:

```sh
python3 scripts/gen-coverage-fixtures.py proof-coverage --bin path/to/saccade
```

It creates page/viewport, asset size/language and scene/quality-tier sets with
one absent-on-both-sides case and one refusal per family. Every row and count
is checked; measured rows have a last-good pass and an approved-anchor fail.
A recorded one-hour freshness policy also flags the deliberately old approvals.
The procedural PNGs (panels, assets with authored language marks and scene geometry)
and declarations are generated, dedicated to CC0-1.0, and
recorded in `PROVENANCE.json`. There are no third-party assets or fonts.
MCP mirrors are a follow-up; no provider calls or semantic theme checks are part
of this command.
