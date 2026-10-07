# Batch intake and advisory AI

Use `saccade batch` for a local folder or an explicit list of image inputs. It wraps
existing single-image commands and keeps each section's full versioned JSON result.
It adds no classifier or universal quality verdict. `ok` means the selected commands
completed; read their measurements and verdicts before making a decision.

```sh
saccade batch images --out intake-results --section analyze-media --section inspect --json
saccade batch images --out intake-results --section analyze-media --section inspect --json
```

The second invocation resumes the first. Every completed receipt, including failures,
is immutable. Removed inputs and changed content retain their earlier rows; changed
content gets a new identity. An interrupted item has no terminal receipt and is retried
in a new attempt directory. There is no implicit retry of completed failures: use a new
output directory for a new analysis configuration or a repaired runtime. Never place
outputs inside the intake folder. Original files are never modified. Single-image analysis reads hash-bound snapshots.
Comparison reads original paths to retain adjacent capture sidecars and verifies input
hashes again afterward; changed bytes make the row partial. One writer can own an output directory at a time.

`rows.jsonl`, `rows.csv` and `index.html` provide complete rows, a tabular summary and a
thumbnail contact-sheet index. `saccade-manifest.json` registers these artifacts,
individual receipts, thumbnails and comparison reports using `saccade-manifest.v1`.
The CSV quotes every field. The HTML escapes path data and contains no script.
`rows/*.json` is the authoritative durable journal; summaries are rebuilt on resume.
Input identity includes path, input content hash and paired reference content hash.
Duplicate basenames are disambiguated even when their bytes match. Repeated list
entries each retain an occurrence and row. Unknown section/provenance states remain
partial even when the underlying measurement passes.

Rows have explicit `ok`, `corrupt`, `unsupported`, `duplicate-basename`, `timed-out`,
`partial` or `skipped` status. Duplicate detection also has its own boolean so a corrupt
or partially processed duplicate retains both facts. A failed/unavailable section stays
in its row. An absent pair reference skips that section and makes the item partial.
An explicitly skipped input still has a receipt. Exit 4 means some retained rows are
incomplete or failed; exit 0 means processing completed, including explicitly skipped
rows. Invalid configuration/output ownership exits 2. These exits grant no approval.

By default two items run concurrently with a 30-second whole-item deadline. Set
`--concurrency 1..16` and `--timeout-ms 1..300000`. Intake is limited to 1000 items and
16 unique sections. Files and section outputs are limited to 64 MiB; decoding has a
256 MiB allocation limit and 32768-pixel edge limit. Unix workers have separate process
groups, terminated on timeout; other platforms terminate the direct worker. Files are
local and regular; symlinks and special entries from folders are skipped. No downloads
or provider dispatch flags are admitted. Large input reads and snapshot writes use
bounded local IO; they are checked against the deadline before decoding starts.

For pairs, `--reference-dir references` matches each folder-relative path. A JSON list
can declare arbitrary pairs, deliberate skips and a tighter per-item `timeout_ms`
(at most the run timeout); relative paths resolve beside it:

```json
{"schema":"saccade-batch-input.v1","inputs":[
  {"path":"images/a.png","reference":"references/a.png"},
  {"path":"images/b.png","skip":true}
]}
```

`--section` accepts `analyze-media`, `inspect`, `compare`, `text-quality`, `tofu`,
`watermark` and `mask-metrics`. `inspect` delegates to `inspect-image`;
`text-quality` delegates to the paired `text-legibility` command. Optional capabilities
still need the features/models documented by those commands. To pass their existing
flags, supply `--options options.json` instead of command-line resource/section flags:

```json
{"sections":[{"command":"text-quality","args":["--region","0,0,80,40"]},
             {"command":"watermark","args":["--trustmark"]}],
 "concurrency":2,"timeout_ms":30000}
```

Arguments are arrays, never shell fragments. `--out` and `--json` are managed by the
wrapper. Changing options or the executable bytes refuses resume; use a new directory.
Executable hashing/configuration checks happen once per invocation before item deadlines.
Raw paths are recorded in local receipts; review them before sharing an output bundle.
New contracts are `saccade-batch-input.v1`, `saccade-batch-run.v1`,
`saccade-batch-row.v1` and `saccade-batch-result.v1`. Unknown intake versions are refused.

The `saccade-vision` Python package exports a synchronous API returning all rows:

```python
import saccade
rows = saccade.batch("images", "intake-results")
failed = [row for row in rows if row["status"] in {"corrupt", "partial", "timed-out"}]
```

The wheel calls the Rust core in-process and needs no CLI or `SACCADE_BIN`.
It shares intake, input snapshots, concurrency admission, immutable receipts, row
statuses, resume, JSONL/CSV/HTML summaries and manifests with the CLI. `options_json`
accepts the options object above as a JSON string; `reference_dir` supports folder
pairs. Compute releases the Python GIL. Resume pins the loaded extension bytes,
resolved model configuration and registry digest; switching between CLI and Python
workers requires a new output directory.

Python deadlines are cooperative: checked before and after hashing/snapshot, probe
and each section. An active bounded Rust decode/analysis finishes before the function
returns, even if it crosses its deadline; that row is `timed-out` and no later section
starts. There are no detached timed-out workers or subprocesses. Byte, decoding,
item, section and concurrency limits still apply. Applications needing a hard wall
clock limit must isolate their Python job in their own supervised process.

The in-process sections use the existing Rust producers and preserve their report
schemas. Supported flags are:

| Section | In-process flags |
|---|---|
| `analyze-media` | `--profile`, `--options`, `--strict`, `--output-size`; shared operator model configuration |
| `compare` | `--config` (shared TOML settings), `--threshold`, `--metric`, `--ppd`, `--allow-empty`, `--fail-on-new`, `--require-matching-meta`; full core report and artifacts |
| `inspect` | `--include-gps`; headers, copy-move candidates, quality, credentials and error-level layer |
| `mask-metrics` | `--class`, `--each-label`, `--void`, `--boundary-px` |
| `watermark` | `--expected-payload`, `--quantization-step`, `--minimum-agreement`; primary decoder remains explicitly unavailable |
| `tofu` | `--mask`, `--expected-text`; requires `text-quality` in the wheel build |
| `text-quality` | `--region`, the four `--minimum-*` pixel thresholds; requires `text-quality` in the wheel build |

Other CLI transport flags, including imported OCR/vision observations, archive
lookup, explicit registration and TrustMark runtime selection, remain explicit
`partial` section failures in Python. They never trigger a CLI fallback or get
ignored. Missing optional evidence retains its unavailable state. Standard wheels
provide CPU-lite media analysis; custom model/feature builds and provisioning remain
operator owned.

With the `assist` feature, `saccade assist explain`, `audit-mask`, `check-ui` and
`batch submit|status|collect` are additive aliases for the existing `review` advice
commands. Their existing experimental acknowledgement, budgets, source-root egress
permissions, immutable revisions and offline replay checks remain enforced.

```sh
saccade assist explain --report report/saccade-report.v1.json --out advice \
  --experimental --route all-vision --gemini-revision declared-revision --json
```

No provider is dispatched without `--run`. Before each interactive dispatch,
`egress-preview.jsonl` records `saccade-egress-preview.v1`: provider, model, required
revision, exact payload file/hash/bytes, a conservative cost estimate and denied
baseline/exclusion/timing authority. The same preview is printed to stderr before a
live call. Missing or expired pricing remains null, never free. The estimate is a ceiling,
not measured usage. The frozen evaluation lifecycle previews the request plan and spend
allowance before submission or status HTTP. Cached/offline observations remain attributed
advice and do not become fresh qualification evidence.

Image text and provider output are data. Advice cannot create/apply exclusions, approve
baselines, override deterministic failures or qualify timing. The aliases expose no
approval operation. Human decisions use the existing separate review/approval workflow.
MCP batch mirrors and streaming Python output are follow-ups; this page promises local
CLI and synchronous Python intake only.
