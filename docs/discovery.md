# Finding, linking and navigating outputs

An output directory is described by one `saccade-manifest.json` (`saccade-manifest.v1`). It is
built on the report identity every report already carries (`report_id`, the shared
`reports/index.jsonl`); there is no second index and no second hash scheme.

```
saccade manifest build DIR [--approved-anchor FILE] [--last-good FILE]
saccade manifest classify PATH        # report_directory | json_document | api_response | manifest | link
saccade manifest link DIR --report-id sha256:... --out link.json
saccade manifest verify DIR|link.json # fails with link_missing or stale_link
saccade export-regions REPORT.json --out DIR --top 5
saccade view DIR --open
```

## What the manifest records

- `artifacts`: every file under `DIR` (depth 6, symlinks skipped and counted) by SHA-256.
  Byte-identical files are listed once; `also_at` names the copies, so duplicates never inflate counts.
- `reports`: one row per `report_id`, merged from the report files and the index rows. A report
  indexed twice, or stored twice, is one row (`indexed_rows` shows the repeats).
- `completion` is `recorded`; it never says approved.
- `approval` is separate from every verdict. `state` is `none` unless you declare an
  `--approved-anchor`, which is recorded as `anchor_declared`. `--last-good` is a different role
  (see [last-good](last-good.md)): a passing run is never read as approval.

Nothing is written by `build` except the manifest, regenerated atomically. A different file with
the manifest's name is refused. Nothing is retained, moved or deleted.

## Links and stale-link failures

`link` writes `saccade-link.v1`: the `report_id`, the report's path relative to the link file and its
content hash. `verify` re-hashes what a manifest or link names. A recorded file that moved or
vanished fails with `link_missing`; one whose bytes changed fails with `stale_link`. Files added
since are not failures. Both are exit code 2 with the stable error codes in the JSON envelope.

## Reviewer last mile

- `view DIR --open` opens the report page in the default browser. It is best effort and never an error.
- The HTML report orders rows worst first: failing and local-change entries first, then the strongest
  single-pixel error, then the deciding value. A tiny severe defect outranks many mild differences.
  `[` and `]` step in that order; `w` jumps to the worst entry.
- `export-regions` crops the worst hotspots (baseline, capture, heatmap) with their pixel and
  fractional coordinates into `saccade-region-export.v1.json`. It records no decision and never
  writes a baseline. Re-running into its own output replaces it; a foreign non-empty directory is refused.
