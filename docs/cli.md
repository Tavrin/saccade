# Command reference

Generated from compiled capabilities and `--help`; do not edit by hand.

Generation: `cargo build --release -p saccade --all-features`, then `python3 scripts/gen-docs.py --saccade target/release/saccade`.
The all-features binary includes every supported operation.

Compiled features: `ai`, `evaluation`, `graphics`, `mcp`, `parallel`, `prechecks`, `schema`, `workbench`.

Exit 1 means a failed image measurement/evaluation gate or located divergence.
Exit 0 for compare/identity means no image regression; inspect `performance` for qualification.
Inspection, review, rank and ablation completion grant no acceptance authority.
Exit 2 means the operation cannot run. Demo intentionally exits 1.

## saccade

```text
Tell when visual or performance evidence is not good enough to support a claim

Usage: saccade [OPTIONS] <COMMAND>

Commands:
  compare  Compare a directory of captures against a directory of baselines
  prove    Check whether image identity or performance evidence proves a claim
  review   Preview a review plan or handle a local closed decision request

Options:
  -h, --help     Print help
  -V, --version  Print version

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output


Start here:
  saccade compare baseline/ captures/ --out report
  saccade prove identity parent/ candidate/ --out proof
  saccade prove performance --base 'base_r*' --arm 'candidate=candidate_r*'
  saccade review report/saccade-report.v1.json --out review

Exit codes: 0 no image regression, 1 image regression found, 2 the command could not run.
Advanced: demo, identity, noise, view, inspect, experiment, approve, init,
serve, mcp, ingest, bisect, history, doctor. Existing commands keep working; use `saccade COMMAND --help`.
```

## saccade history

```text
Record and inspect local visual-test variation across runs

Usage: saccade history [OPTIONS] <COMMAND>

Commands:
  record   Add one existing comparison report to the local history store
  analyze  Show measured variation and threshold advice for comparable entries

Options:
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade history record

```text
Add one existing comparison report to the local history store

Usage: saccade history record [OPTIONS] --store <STORE> <REPORT>

Arguments:
  <REPORT>

Options:
      --store <STORE>
      --json
  -h, --help           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade history analyze

```text
Show measured variation and threshold advice for comparable entries

Usage: saccade history analyze [OPTIONS] --store <STORE>

Options:
      --store <STORE>
      --entry <ENTRY>
      --limit <LIMIT>  [default: 10]
      --json
  -h, --help           Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade bisect

```text
Locate the first commit whose fresh capture fails its baseline

Usage: saccade bisect [OPTIONS] --capture <CAPTURE> --baseline <BASELINE>

Options:
      --good <GOOD>          Known good revision in the current repository
      --bad <BAD>            Known bad revision descended from --good
      --capture <CAPTURE>    Shell capture command; write images to SACCADE_CAPTURE_DIR (sh on Unix, cmd on Windows)
      --baseline <BASELINE>  Stable baseline directory, copied before Git changes revisions
      --perf                 Require qualified performance evidence and count a slower frame as bad
      --out <OUT>            Evidence directory outside the repository; defaults to a new sibling
      --json                 Print bounded JSON
  -h, --help                 Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest

```text
Convert a test runner's screenshot artifacts into compared image pairs

Usage: saccade ingest [OPTIONS] <COMMAND>

Commands:
  blender     Pair Blender render report category/ref images with category renders
  bevy        Pair Bevy screenshot-N.png files from two runs
  unity       Pair Unity Graphics Test Framework ReferenceImages and ActualImages
  unreal      Read Unreal screenshot comparison result paths from JSON
  playwright  Compare expected and actual Playwright screenshot attachments

Options:
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest blender

```text
Pair Blender render report category/ref images with category renders

Usage: saccade ingest blender [OPTIONS] --out <OUT> <ROOT>

Arguments:
  <ROOT>

Options:
      --out <OUT>
      --json
  -h, --help       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest bevy

```text
Pair Bevy screenshot-N.png files from two runs

Usage: saccade ingest bevy [OPTIONS] --out <OUT> <REFERENCE> <CAPTURE>

Arguments:
  <REFERENCE>
  <CAPTURE>

Options:
      --out <OUT>
      --json
  -h, --help       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest unity

```text
Pair Unity Graphics Test Framework ReferenceImages and ActualImages

Usage: saccade ingest unity [OPTIONS] --out <OUT> <ASSETS>

Arguments:
  <ASSETS>

Options:
      --out <OUT>
      --json
  -h, --help       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest unreal

```text
Read Unreal screenshot comparison result paths from JSON

Usage: saccade ingest unreal [OPTIONS] --out <OUT> <RESULTS>

Arguments:
  <RESULTS>

Options:
      --out <OUT>
      --json
  -h, --help       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade ingest playwright

```text
Compare expected and actual Playwright screenshot attachments

Usage: saccade ingest playwright [OPTIONS] --out <OUT> <MANIFEST>

Arguments:
  <MANIFEST>  Manifest written by integrations/playwright/reporter.cjs

Options:
      --out <OUT>              New directory for paired inputs and the comparison report
      --json                   Print the bounded comparison result
      --threshold <THRESHOLD>  FLIP threshold for the comparison
  -h, --help                   Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade doctor

```text
Print installed version, features and supported evidence schemas

Usage: saccade doctor [OPTIONS]

Options:
      --json  Print machine-readable JSON
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade init

```text
Bootstrap a commented configuration and print baseline adoption steps

Usage: saccade init [OPTIONS]

Options:
      --template <TEMPLATE>  [default: renderer] [possible values: renderer, ui, identity, ml, ci, nightly, lookdev]
      --dir <DIR>            [default: .]
      --force
  -h, --help                 Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade demo

```text
Run the bundled example and explain its expected regression

Usage: saccade demo [OPTIONS]



Example:
  saccade demo --out saccade-demo
  saccade view saccade-demo          Print where the demo report is

The demo exits 1 on purpose: it contains a regression and a missing capture.

Options:
      --out <OUT>  Directory for the demo images and reports (default: a new temporary directory)
  -h, --help       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade compare

```text
Compare a directory of captures against a directory of baselines

Usage: saccade compare [OPTIONS] <BASELINE_DIR> <CAPTURE_DIR>



Images are paired by relative path. Each pair gets a FLIP score; a pair fails when
its deciding metric is above the threshold. The report directory holds index.html
(open it in a browser) and saccade-report.v1.json.

Examples:
  saccade compare baseline/ captures/ --out report
  saccade compare baseline/ captures/ --threshold 0.02 --metric p95
  saccade compare baseline/ captures/ --entry 'ui/*' --junit report/junit.xml
  saccade compare baseline/ captures/ --json        One bounded JSON result on stdout

Exit codes: 0 no regression, 1 regression found, 2 the command could not run.

Arguments:
  <BASELINE_DIR>  Directory of approved baseline images
  <CAPTURE_DIR>   Directory of fresh captures

Options:
  -h, --help  Print help

Output:
      --out <OUT>         Report output directory [default: report]
      --json              Print a bounded machine-readable result
      --labels <A,B>      Display names of the two sides, `baseline,capture`
      --junit <FILE.xml>  Write one JUnit testcase per entry

Gate:
      --threshold <THRESHOLD>  Default pass threshold (overrides the config file's top level)
      --metric <METRIC>        Default deciding metric (overrides the config file's top level) [possible values: mean, p95, p99, max]
      --config <CONFIG>        Config file; defaults to ./saccade.toml when it exists
      --fail-on-new            Treat new images (no baseline) as a regression
      --allow-empty            Accept a run that compared no pair (for example the first run, with an empty baseline directory). Without it, nothing compared exits 1
      --ppd <PPD>              FLIP pixels per degree

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)

Selection:
      --entry <GLOB>  Include only matching names (repeatable; union of globs)

Metadata sidecars:
      --meta-name <NAME>        Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>  Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --require-matching-meta   Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>       Sidecar keys (or globs) that may differ with --require-matching-meta

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Review context:
      --intent <TEXT>        What the change is meant to do, in one sentence, recorded in the evidence
      --intent-file <FILE>   Structured evidence intent or visual declaration JSON, written before capture
      --changes-file <FILE>  JSON list of expected changes; needs --intent or --intent-file

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade identity

```text
Establish exact native decoded-sample equality in the selected scope

Usage: saccade identity [OPTIONS] <PARENT_DIR> <CANDIDATE_DIR>



Use it to prove a refactor or optimization renders the same pixels. There is no
threshold: any differing sample fails. Different file encodings of equal pixels pass.

Examples:
  saccade identity parent/ candidate/ --out report
  saccade identity parent/ candidate/ --json      One bounded JSON result on stdout

Exit codes: 0 every pair identical, 1 identity not proven (a pair differs, is missing,
new or unreadable), 2 the command could not run.

Arguments:
  <PARENT_DIR>     Directory of images from the parent build
  <CANDIDATE_DIR>  Directory of images from the candidate build

Options:
  -h, --help  Print help

Output:
      --out <OUT>         Report output directory [default: report]
      --json              Print a bounded machine-readable result
      --labels <A,B>      Display names of the two sides, `parent,candidate`
      --junit <FILE.xml>  Write one JUnit testcase per entry

Gate:
      --allow-empty      Accept a run that compared no pair. Without it, nothing compared exits 1
      --config <CONFIG>  Config file; defaults to ./saccade.toml when it exists
      --ppd <PPD>        FLIP pixels per degree, used only to describe differences

Selection:
      --entry <GLOB>  Include only matching names (repeatable; union of globs)

Metadata sidecars:
      --meta-name <NAME>        Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>  Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --require-matching-meta   Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>       Sidecar keys (or globs) that may differ with --require-matching-meta

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Review context:
      --intent <TEXT>        What the change is meant to do, in one sentence, recorded in the evidence
      --intent-file <FILE>   Structured evidence intent or visual declaration JSON, written before capture
      --changes-file <FILE>  JSON list of expected changes; needs --intent or --intent-file

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade prove

```text
Check whether image identity or performance evidence proves a claim

Usage: saccade prove [OPTIONS] <COMMAND>

Commands:
  identity     Prove exact native decoded-sample equality over the selected images
  performance  Evaluate performance claims from ablation arms and repeat noise

Options:
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade prove identity

```text
Prove exact native decoded-sample equality over the selected images

Usage: saccade prove identity [OPTIONS] <PARENT_DIR> <CANDIDATE_DIR>

Arguments:
  <PARENT_DIR>
  <CANDIDATE_DIR>

Options:
      --out <OUT>         [default: report]
      --config <CONFIG>
      --json
      --allow-empty
      --ppd <PPD>
      --labels <A,B>
      --junit <FILE.xml>
      --entry <GLOB>
  -h, --help              Print help

Metadata sidecars:
      --meta-name <NAME>        Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>  Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --require-matching-meta   Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>       Sidecar keys (or globs) that may differ with --require-matching-meta

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Review context:
      --intent <TEXT>        What the change is meant to do, in one sentence, recorded in the evidence
      --intent-file <FILE>   Structured evidence intent or visual declaration JSON, written before capture
      --changes-file <FILE>  JSON list of expected changes; needs --intent or --intent-file

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade prove performance

```text
Evaluate performance claims from ablation arms and repeat noise

Usage: saccade prove performance [OPTIONS] [BASE] [ARMS]...

Arguments:
  [BASE]
  [ARMS]...

Options:
      --base <RUN_DIR>...     Base repeat directories. Accepts a directory or a quoted glob; repeatable
      --arm <LABEL=RUN_GLOB>  Labelled arm repeats, e.g. --arm 's2=s2_r*'; repeatable
      --out <OUT>             [default: ablation]
      --config <CONFIG>
      --json
      --top <TOP>             Per-term deltas beyond noise to show per arm [default: 5]
  -h, --help                  Print help

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade noise

```text
Calibrate thresholds from repeated captures of an unchanged build

Usage: saccade noise [OPTIONS] <DIRS> <DIRS>...

Options:
      --kind <KIND>  Image calibration (default) or qualified performance noise in ms [default: image] [possible values: image, performance]
  -h, --help         Print help

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)
      --config <CONFIG>
      --margin <MARGIN>            [default: 1.5]
      --metric <METRIC>            [default: p95] [possible values: mean, p95, p99, max]
      --out <OUT>                  [default: saccade.noise.toml]
      --json
  <DIRS> <DIRS>...

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade view

```text
Write a self-contained review viewer for 2 to 6 image directories

Usage: saccade view [OPTIONS] [DIRS]...



Examples:
  saccade view before/ after/ --out view          Swipe, flicker and heatmap viewer
  saccade view a/ b/ c/ --labels a,b,c --reference a
  saccade view my-report                          Print where an existing report's page is
  saccade view a/ b/ --blind --key-out ../key.json --out judge-view

Arguments:
  [DIRS]...  Directories to compare, paired by relative image path (2 to 6)

Options:
  -h, --help  Print help

Blind judging:
      --unblind <UNBLIND>  Resolve recorded anonymous choices after review
      --key <KEY>          The key written by --blind, used with --unblind
      --blind              Pairwise judging: shuffle panes and hide labels until "Reveal"
      --seed <SEED>        Seed for the blind shuffle (default: random). A blind page never embeds it; it is recorded in the key
      --key-out <PATH>     Where a blind view's key goes. Required with --blind; keep it outside --out so the judge never receives it

Output:
      --labels <LABELS>  Comma-separated labels, one per directory (default: directory names)
      --out <OUT>        Output directory [default: view]
      --json             Print a JSON summary (`saccade-view-summary.v1`) instead of text

Comparison:
      --reference <REFERENCE>  FLIP reference: a label or one of the directories (default: the first)
      --ppd <PPD>              FLIP pixels per degree
      --config <CONFIG>        Config file whose `[[region]]` tables become preset ROIs (default: `./saccade.toml` when present)

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)

Selection:
      --entry <GLOB>  Include only matching names (repeatable; union of globs)

Metadata sidecars:
      --meta-name <NAME>        Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>  Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade approve

```text
Copy reviewed captures over baselines

Usage: saccade approve [OPTIONS] [CAPTURE_DIR] [BASELINE_DIR] [NAMES]...



Example (two steps: plan, then apply the reviewed decision):
  saccade approve --report report/saccade-report.v1.json --entry ui.png --dry-run --out plan
  saccade approve --report report/saccade-report.v1.json --decisions plan/decision.json --out receipt

Review the report, plan/manifest.json and plan/decision.json between the two steps.
The dry run writes no baseline; content hashes must still match when applying.

Arguments:
  [CAPTURE_DIR]   Directory of fresh captures
  [BASELINE_DIR]  Baseline directory to update
  [NAMES]...      Image names (relative paths) to approve

Options:
      --report <REPORT>              Derive the input directories from this report
      --entry <NAME>                 Select a report entry without positional directories; repeatable
      --all-failing [<REPORT_JSON>]  Also approve every fail and new entry of this report JSON
      --decisions <DECISIONS_JSON>   Explicit canonical CLI decision bound to this report, inputs and scope
      --include-errors               With --all-failing: also approve `error` entries (for example a size change) whose capture exists and decodes
      --prune-missing                With --all-failing: delete the baselines of every `missing` entry of the report (capture absent). Only files inside the baseline directory are removed; each removal is printed
      --json                         Print `{"schema":"saccade-approve.v1","copied":[...],"pruned":[...]}` instead of one line per file
      --dry-run                      Prepare a selected update manifest and unattested CLI decision draft
      --out <OUT>                    Empty directory for the plan, decision and applied receipt
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade serve

```text
Browse report and image archives in a local web workbench

Usage: saccade serve [OPTIONS] [ROOTS]...



The server listens on 127.0.0.1 only. Archive roots are read-only: sessions,
thumbnails and uploads go to the cache directory, decisions to the decisions directory.

Examples:
  saccade serve captures/ --open               Browse and compare runs in the browser
  saccade serve captures/ reports/ --port 0    Several roots; pick a free port

Arguments:
  [ROOTS]...  Archive roots to browse (read-only). With several, each is a top-level entry named after its directory

Options:
      --root <REGISTERED_ROOTS>
          Additional read-only archive roots (repeatable)
      --out-root <OUT_ROOT>
          Explicit generated-artifact root
      --follow-symlinks-within-roots
          Let a symlink that resolves inside any of the roots be browsed and served; a symlink to anywhere else stays refused
      --symlink-target <SYMLINK_TARGETS>
          Allow symlinks reached below a root to resolve into DIR (repeatable)
      --fs-timeout-ms <FS_TIMEOUT_MS>
          Storage deadline in milliseconds (default: 3000)
      --port <PORT>
          Port on 127.0.0.1 (0 picks a free one) [default: 7878]
      --cache-dir <CACHE_DIR>
          Cache directory for sessions, thumbnails and uploads (default: `$XDG_CACHE_HOME/saccade`)
      --decisions-dir <DECISIONS_DIR>
          Directory the viewer's decisions are written to (default: `$XDG_DATA_HOME/saccade/decisions`)
      --config <CONFIG>
          Config file for sidecar settings and preset regions (default: `./saccade.toml` when present)
      --ppd <PPD>
          FLIP pixels per degree
      --open
          Open the page in the default browser
  -h, --help
          Print help

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)

Metadata sidecars:
      --meta-name <NAME>        Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>  Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade mcp

```text
Serve the agent tools over MCP on stdio, confined to the given roots

Usage: saccade mcp [OPTIONS] --root <ROOTS>



Every path a client passes must resolve under a --root. Generated reports go under
--out-root, which must be separate from the read-only roots.

Example:
  saccade mcp --root examples --out-root agent-reports

Options:
      --root <ROOTS>                  Read-only roots (repeatable)
      --out-root <OUT_ROOT>           Generated artifacts require this separate root
      --follow-symlinks-within-roots  Let a symlink that resolves inside any of the roots be read
      --symlink-target <DIR>          Allow symlinks reached below a root to resolve into DIR (repeatable)
      --allow-provider-calls          Explicitly authorize provider calls for this MCP server lifetime
      --budget-calls <BUDGET_CALLS>   Finite startup attempt cap; no implicit MCP allowance
      --user-config <USER_CONFIG>     Human-owned endpoints, credential bindings and root egress policy
  -h, --help                          Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect

```text
Read, explain, prepare or export existing evidence

Usage: saccade inspect [OPTIONS] [ARTIFACT] [COMMAND]

Commands:
  evidence      Prepare context, crops, facts and references without a provider
  export        Export an existing artifact or selected entry
  config        Explain effective measurement settings and their sources
  capabilities  List compiled modules, operations and contracts

Arguments:
  [ARTIFACT]

Options:
      --entry <ENTRY>
      --validity-reasons                     List every capture-validity reason, with pagination
      --status <STATUS>
      --limit <LIMIT>                        [default: 10]
      --cursor <CURSOR>
      --expected-case-id <EXPECTED_CASE_ID>
      --json
  -h, --help                                 Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect evidence

```text
Prepare context, crops, facts and references without a provider

Usage: saccade inspect evidence [OPTIONS] --out <OUT> <REPORT>

Arguments:
  <REPORT>

Options:
      --out <OUT>
      --entry <ENTRIES>
      --top <TOP>          [default: 5]
      --stretch
      --blind
      --key-out <KEY_OUT>
      --seed <SEED>
      --json
  -h, --help               Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect export

```text
Export an existing artifact or selected entry

Usage: saccade inspect export [OPTIONS] --format <FORMAT> --out <OUT> <ARTIFACT>

Arguments:
  <ARTIFACT>

Options:
      --format <FORMAT>              [possible values: json, markdown, junit, png, labels]
      --out <OUT>
      --entry <ENTRY>
      --state <STATE>
      --width <WIDTH>                [default: 1024]
      --artifact-url <ARTIFACT_URL>
      --comment-key <COMMENT_KEY>
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect config

```text
Explain effective measurement settings and their sources

Usage: saccade inspect config [OPTIONS]

Options:
      --config <CONFIG>
      --entry <PATH_OR_NAME>
      --json
  -h, --help                  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade inspect capabilities

```text
List compiled modules, operations and contracts

Usage: saccade inspect capabilities [OPTIONS]

Options:
      --json
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review

```text
Preview a review plan or handle a local closed decision request

Usage: saccade review [OPTIONS] [REPORT] [COMMAND]

Commands:
  request  Prepare a closed request from an existing canonical case, locally
  propose  Validate and record proposed answers against the exact request
  ask      Create or retrieve a local human review item for an unresolved request
  eval     Plan or run a resumable evaluation manifest

Arguments:
  [REPORT]

Options:
      --run
      --budget-calls <BUDGET_CALLS>
      --out <OUT>
      --user-config <USER_CONFIG>
      --intent-file <INTENT_FILE>
      --intent <INTENT>
      --json
  -h, --help                         Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review request

```text
Prepare a closed request from an existing canonical case, locally

Usage: saccade review request [OPTIONS] --question <QUESTION> --out <OUT> <REPORT>

Arguments:
  <REPORT>

Options:
      --question <QUESTION>
      --out <OUT>
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review propose

```text
Validate and record proposed answers against the exact request

Usage: saccade review propose [OPTIONS] --answers <ANSWERS> <REQUEST>

Arguments:
  <REQUEST>

Options:
      --answers <ANSWERS>
      --out <OUT>
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review ask

```text
Create or retrieve a local human review item for an unresolved request

Usage: saccade review ask [OPTIONS] <REQUEST>

Arguments:
  <REQUEST>

Options:
      --out <OUT>
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade review eval

```text
Plan or run a resumable evaluation manifest

Usage: saccade review eval [OPTIONS] --manifest <MANIFEST>

Options:
      --manifest <MANIFEST>
      --run
      --user-config <USER_CONFIG>
      --json
  -h, --help                       Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment

```text
Analyze existing graphics captures: ablation, sequences, ranking, bisection

Usage: saccade experiment [OPTIONS] <COMMAND>

Commands:
  ablate    Compare ablation arms against a base with image and performance evidence
  temporal  Compare numbered SDR frames with the ColorVideoVDP temporal model
  sequence  Compare numbered colour frames by sorted index and measure added flicker
  rank      Rank candidate directories against one common FLIP reference
  bisect    Find the first diverging run or revision in an ordered series
  safety    Photosensitivity PRE-CHECK only; not certification or formal compliance
  a11y      Accessibility PRE-CHECK only; not certification or formal compliance

Options:
  -h, --help  Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment ablate

```text
Compare ablation arms against a base with image and performance evidence

Usage: saccade experiment ablate [OPTIONS] [BASE] [ARMS]...

Arguments:
  [BASE]
  [ARMS]...

Options:
      --base <RUN_DIR>...     Base repeat directories. Accepts a directory or a quoted glob; repeatable
      --arm <LABEL=RUN_GLOB>  Labelled arm repeats, e.g. --arm 's2=s2_r*'; repeatable
      --out <OUT>             [default: ablation]
      --config <CONFIG>
      --json
      --top <TOP>             Per-term deltas beyond noise to show per arm [default: 5]
  -h, --help                  Print help

Performance:
      --perf-name <NAME>           Run performance sidecar file name (default saccade-perf.json)
      --gpu-clocks-not-applicable  Declare that GPU clocks do not apply to this performance measurement
      --perf-noise-override        Use an explicit --perf-noise floor even if complete base repeats derive a higher floor
      --perf-noise <FILE>          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <K>           Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <MS>       Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <N>  Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <MS>     Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PCT>   Minimum meaningful delta as a percentage of the baseline frame (default 0.5)

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment temporal

```text
Compare numbered SDR frames with the ColorVideoVDP temporal model

Usage: saccade experiment temporal [OPTIONS] --fps <FPS> <BASELINE_DIR> <CAPTURE_DIR>

Arguments:
  <BASELINE_DIR>  Directory of numbered baseline PNG/JPEG frames
  <CAPTURE_DIR>   Directory of numbered capture PNG/JPEG frames

Options:
      --fps <FPS>          Frame rate used by the temporal visibility model
      --display <DISPLAY>  Embedded ColorVideoVDP display model [default: standard_4k]
      --pattern <PATTERN>  Relative-name glob for numbered frames [default: *]
      --out <OUT>          Output directory for the sequence and temporal reports [default: temporal-report]
      --min-jod <MIN_JOD>  Optional minimum acceptable video quality in JOD units
      --json               Print a bounded JSON summary
  -h, --help               Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment sequence

```text
Compare numbered colour frames by sorted index and measure added flicker

Usage: saccade experiment sequence [OPTIONS] <BASELINE_DIR> <CAPTURE_DIR>

Arguments:
  <BASELINE_DIR>
  <CAPTURE_DIR>

Options:
      --pattern <PATTERN>      Relative-name glob; frames must end in an integer before the extension [default: *]
      --out <OUT>              [default: sequence-report]
      --threshold <THRESHOLD>
      --metric <METRIC>        [possible values: mean, p95, p99, max]
      --config <CONFIG>
      --ppd <PPD>
      --fail-on-new
      --allow-empty
      --labels <LABELS>
      --json
  -h, --help                   Print help

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)
      --junit <FILE.xml>              Write one JUnit testcase per entry

Metadata sidecars:
      --meta-name <NAME>        Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>  Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --require-matching-meta   Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>       Sidecar keys (or globs) that may differ with --require-matching-meta

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment rank

```text
Rank candidate directories against one common FLIP reference

Usage: saccade experiment rank [OPTIONS] <REFERENCE_DIR> <CANDIDATE_DIRS>...

Arguments:
  <REFERENCE_DIR>
  <CANDIDATE_DIRS>...

Options:
      --labels <LABELS>        One unique, safe directory label per candidate, comma separated
      --metric <METRIC>        [default: mean] [possible values: mean, p95, p99, max]
      --out <OUT>              [default: rank-report]
      --config <CONFIG>
      --threshold <THRESHOLD>
      --ppd <PPD>
      --fail-on-new
      --allow-empty
      --json
  -h, --help                   Print help

HDR images:
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)
      --junit <FILE.xml>              Write one JUnit testcase per entry

Metadata sidecars:
      --meta-name <NAME>        Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>  Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --require-matching-meta   Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>       Sidecar keys (or globs) that may differ with --require-matching-meta

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment bisect

```text
Find the first diverging run or revision in an ordered series

Usage: saccade experiment bisect [OPTIONS]

Options:
      --runs <RUNS>...         Ordered run directories, oldest first (repeatable)
      --runs-from <RUNS_FROM>  One ordered run path per line
      --good <GOOD>            Reference for existing runs (default: first run)
      --threshold <THRESHOLD>  Explicit FLIP threshold relaxes native sample identity
      --metric <METRIC>        mean, p95, p99 or max (default max)
      --entries <ENTRIES>      Select image names by glob
      --out <OUT>              Report directory, separate from inputs [default: bisect-report]
      --json                   Print saccade-bisect.v1 JSON
  -h, --help                   Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment safety

```text
Photosensitivity PRE-CHECK only; not certification or formal compliance

Usage: saccade experiment safety [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Numbered frames or mp4/mov/mkv (requires external ffmpeg)

Options:
      --fps <FPS>            Frame rate override; otherwise metadata, or 60 for frame directories
      --display <DISPLAY>    WxH@diagonal_inches,distance_metres (default 1920x1080@55,4)
      --standard <STANDARD>  itu-bt1702 or wcag. PRE-CHECK only, never certification [default: itu-bt1702]
      --json                 Print full saccade-safety.v1 JSON
      --out <OUT>            Output directory for JSON, text, HTML, static frames and risk heatmaps [default: safety-report]
      --junit <JUNIT>        Optional JUnit XML destination
  -h, --help                 Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```

## saccade experiment a11y

```text
Accessibility PRE-CHECK only; not certification or formal compliance

Usage: saccade experiment a11y [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Opaque sRGB image or image directory

Options:
      --config <CONFIG>      Explicit saccade.toml with [[region]] kind="text" or "ui"
      --json                 Print full saccade-a11y.v1 JSON
      --out <OUT>            Output directory for JSON, text, HTML and simulation/heatmap artifacts [default: a11y-report]
      --junit <JUNIT>        Optional JUnit XML destination
      --suggest-regions      Explicitly upload 16 crops/image to Gemini for unconfirmed region proposals
      --keys-dir <KEYS_DIR>  Judge key policy: gemini.env/SACCADE_GEMINI_API_KEY, never ambient keys
  -h, --help                 Print help

Global options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
```
