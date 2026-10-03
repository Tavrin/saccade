# Command reference

Generated from compiled capabilities and `--help`; do not edit by hand.

Generation: `python3 scripts/gen-docs.py --saccade target/release/saccade`.
Use the official default features plus `prechecks` to include every supported operation.

Compiled features: `ai`, `evaluation`, `graphics`, `mcp`, `parallel`, `prechecks`, `workbench`.

Exit 1 means a failed measurement/evaluation gate or located divergence.
Inspection, review, rank and ablation completion grant no acceptance authority.
Exit 2 means the operation cannot run. Demo intentionally exits 1.

## saccade

```text
Perceptual (FLIP) visual-regression diffing

Usage: saccade [OPTIONS] <COMMAND>

Commands:
  init        Bootstrap a commented configuration and print baseline adoption steps
  demo        Run the bundled example and explain its expected regression
  compare     Compare a directory of captures against a directory of baselines
  identity    Establish exact native decoded-sample equality in the selected scope
  noise       Calibrate thresholds from repeated captures of an unchanged build
  view        Write a self-contained review viewer for 2 to 6 image directories
  approve     Copy captures over baselines
  serve       (127.0.0.1 only; the archive is never written to)
  mcp         passes must resolve under `--root`
  inspect     Read, explain, prepare or export existing evidence
  review      Preview a review plan or handle a local closed decision request
  experiment  Analyze existing graphics captures

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
  -h, --help                     Print help
  -V, --version                  Print version
```

## saccade init

```text
Bootstrap a commented configuration and print baseline adoption steps

Usage: saccade init [OPTIONS]

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --template <TEMPLATE>      [default: renderer] [possible values: renderer, ui, identity, ml, ci, nightly, lookdev]
      --dir <DIR>                [default: .]
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
      --force                    
  -h, --help                     Print help
```

## saccade demo

```text
Run the bundled example and explain its expected regression

Usage: saccade demo [OPTIONS]

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --out <OUT>                
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
  -h, --help                     Print help
```

## saccade compare

```text
Compare a directory of captures against a directory of baselines

Usage: saccade compare [OPTIONS] <BASELINE_DIR> <CAPTURE_DIR>

Arguments:
  <BASELINE_DIR>  Directory of approved baseline images
  <CAPTURE_DIR>   Directory of fresh captures

Options:
      --allow-out-near-captures
          Silence warnings when --out is next to capture metadata
      --out <OUT>
          Report output directory [default: report]
      --record-absolute-paths
          Opt in to absolute local paths in reports and machine-readable output
      --threshold <THRESHOLD>
          Default pass threshold (overrides the config file's top level)
      --metric <METRIC>
          Default deciding metric (overrides the config file's top level) [possible values: mean, p95, p99, max]
      --config <CONFIG>
          Config file; defaults to ./saccade.toml when it exists
      --fail-on-new
          Treat new images (no baseline) as a regression
      --allow-empty
          Accept a run that compared no pair (for example the first run, with an empty baseline directory). Without it, nothing compared exits 1
      --json
          Print a bounded machine-readable result
      --ppd <PPD>
          FLIP pixels per degree
      --labels <A,B>
          Display names of the two sides, `baseline,capture`
      --hdr-tonemapper <NAME>
          Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>
          Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)
      --entry <GLOB>
          Include only matching names (repeatable; union of globs)
      --junit <FILE.xml>
          Write one JUnit testcase per entry
      --meta-name <NAME>
          Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>
          Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --require-matching-meta
          Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>
          Sidecar keys (or globs) that may differ with --require-matching-meta
      --perf-name <PERF_NAME>
          Run performance sidecar file name (default saccade-perf.json)
      --perf-noise <PERF_NOISE>
          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <PERF_NOISE_K>
          Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <PERF_RESOLUTION>
          Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <PERF_RESOLUTION_TICKS>
          Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <PERF_MIN_DELTA_MS>
          Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PERF_MIN_DELTA_PCT>
          Minimum meaningful delta as a percentage of the baseline frame (default 0.5)
      --intent <INTENT>
          
      --intent-file <INTENT_FILE>
          
      --changes-file <CHANGES_FILE>
          
  -h, --help
          Print help
```

## saccade identity

```text
Establish exact native decoded-sample equality in the selected scope

Usage: saccade identity [OPTIONS] <PARENT_DIR> <CANDIDATE_DIR>

Arguments:
  <PARENT_DIR>     Directory of images from the parent build
  <CANDIDATE_DIR>  Directory of images from the candidate build

Options:
      --allow-out-near-captures
          Silence warnings when --out is next to capture metadata
      --out <OUT>
          Report output directory [default: report]
      --allow-empty
          Accept a run that compared no pair. Without it, nothing compared exits 1
      --record-absolute-paths
          Opt in to absolute local paths in reports and machine-readable output
      --threshold <THRESHOLD>
          Rejected for identity; use compare for perceptual thresholds
      --metric <METRIC>
          Rejected for identity; use compare for perceptual metrics [possible values: mean, p95, p99, max]
      --config <CONFIG>
          Config file; defaults to ./saccade.toml when it exists
      --json
          Print a bounded machine-readable result
      --ppd <PPD>
          FLIP pixels per degree
      --labels <A,B>
          Display names of the two sides, `parent,candidate`
      --entry <GLOB>
          Include only matching names (repeatable; union of globs)
      --junit <FILE.xml>
          Write one JUnit testcase per entry
      --meta-name <NAME>
          Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>
          Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --require-matching-meta
          Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>
          Sidecar keys (or globs) that may differ with --require-matching-meta
      --perf-name <PERF_NAME>
          Run performance sidecar file name (default saccade-perf.json)
      --perf-noise <PERF_NOISE>
          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <PERF_NOISE_K>
          Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <PERF_RESOLUTION>
          Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <PERF_RESOLUTION_TICKS>
          Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <PERF_MIN_DELTA_MS>
          Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PERF_MIN_DELTA_PCT>
          Minimum meaningful delta as a percentage of the baseline frame (default 0.5)
      --intent <INTENT>
          
      --intent-file <INTENT_FILE>
          
      --changes-file <CHANGES_FILE>
          
  -h, --help
          Print help
```

## saccade noise

```text
Calibrate thresholds from repeated captures of an unchanged build

Usage: saccade noise [OPTIONS] <DIRS> <DIRS>...

Arguments:
  <DIRS> <DIRS>...  

Options:
      --allow-out-near-captures
          Silence warnings when --out is next to capture metadata
      --kind <KIND>
          Image calibration (default) or qualified performance noise in ms [default: image] [possible values: image, performance]
      --perf-name <PERF_NAME>
          Run performance sidecar file name (default saccade-perf.json)
      --record-absolute-paths
          Opt in to absolute local paths in reports and machine-readable output
      --perf-noise <PERF_NOISE>
          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <PERF_NOISE_K>
          Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <PERF_RESOLUTION>
          Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <PERF_RESOLUTION_TICKS>
          Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <PERF_MIN_DELTA_MS>
          Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PERF_MIN_DELTA_PCT>
          Minimum meaningful delta as a percentage of the baseline frame (default 0.5)
      --config <CONFIG>
          
      --margin <MARGIN>
          [default: 1.5]
      --metric <METRIC>
          [default: p95] [possible values: mean, p95, p99, max]
      --out <OUT>
          [default: saccade.noise.toml]
      --json
          
  -h, --help
          Print help
```

## saccade view

```text
Write a self-contained review viewer for 2 to 6 image directories

Usage: saccade view [OPTIONS] [DIRS]...

Arguments:
  [DIRS]...  Directories to compare, paired by relative image path (2 to 6)

Options:
      --allow-out-near-captures
          Silence warnings when --out is next to capture metadata
      --unblind <UNBLIND>
          Resolve recorded anonymous choices after review
      --key <KEY>
          
      --record-absolute-paths
          Opt in to absolute local paths in reports and machine-readable output
      --labels <LABELS>
          Comma-separated labels, one per directory (default: directory names)
      --reference <REFERENCE>
          FLIP reference: a label or one of the directories (default: the first)
      --blind
          Pairwise judging: shuffle panes and hide labels until "Reveal"
      --seed <SEED>
          Seed for the blind shuffle (default: random). A blind page never embeds it; it is recorded in the key
      --key-out <PATH>
          Where a blind view's key goes (default: `blind-key.json` inside `--out`; put it elsewhere to hand the view directory to a judge)
      --out <OUT>
          Output directory [default: view]
      --ppd <PPD>
          FLIP pixels per degree
      --config <CONFIG>
          Config file whose `[[region]]` tables become preset ROIs (default: `./saccade.toml` when present)
      --json
          Print a JSON summary (`saccade-view-summary.v1`) instead of text
      --hdr-tonemapper <NAME>
          Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>
          Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)
      --entry <GLOB>
          Include only matching names (repeatable; union of globs)
      --meta-name <NAME>
          Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>
          Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --perf-name <PERF_NAME>
          Run performance sidecar file name (default saccade-perf.json)
      --perf-noise <PERF_NOISE>
          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <PERF_NOISE_K>
          Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <PERF_RESOLUTION>
          Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <PERF_RESOLUTION_TICKS>
          Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <PERF_MIN_DELTA_MS>
          Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PERF_MIN_DELTA_PCT>
          Minimum meaningful delta as a percentage of the baseline frame (default 0.5)
  -h, --help
          Print help
```

## saccade approve

```text
Copy captures over baselines

Usage: saccade approve [OPTIONS] [CAPTURE_DIR] [BASELINE_DIR] [NAMES]...

Arguments:
  [CAPTURE_DIR]   Directory of fresh captures
  [BASELINE_DIR]  Baseline directory to update
  [NAMES]...      Image names (relative paths) to approve

Options:
      --allow-out-near-captures      Silence warnings when --out is next to capture metadata
      --report <REPORT>              Derive the input directories from this report
      --entry <NAME>                 Select a report entry without positional directories; repeatable
      --record-absolute-paths        Opt in to absolute local paths in reports and machine-readable output
      --all-failing [<REPORT_JSON>]  Also approve every fail and new entry of this report JSON
      --decisions <DECISIONS_JSON>   Explicit canonical CLI decision bound to this report, inputs and scope
      --include-errors               With --all-failing: also approve `error` entries (for example a size change) whose capture exists and decodes
      --prune-missing                With --all-failing: delete the baselines of every `missing` entry of the report (capture absent). Only files inside the baseline directory are removed; each removal is printed
      --json                         Print `{"schema":"saccade-approve.v1","copied":[...],"pruned":[...]}` instead of one line per file
      --force                        Removed: stale reviewed content cannot be overridden
      --dry-run                      Prepare a selected update manifest and unattested CLI decision draft
      --out <OUT>                    Empty directory for the plan, decision and applied receipt
  -h, --help                         Print help
```

## saccade serve

```text
(127.0.0.1 only; the archive is never written to)

Usage: saccade serve [OPTIONS] [ROOTS]...

Arguments:
  [ROOTS]...  Archive roots to browse (read-only). With several, each is a top-level entry named after its directory

Options:
      --allow-out-near-captures
          Silence warnings when --out is next to capture metadata
      --root <REGISTERED_ROOTS>
          Additional read-only archive roots (repeatable)
      --out-root <OUT_ROOT>
          Explicit generated-artifact root
      --record-absolute-paths
          Opt in to absolute local paths in reports and machine-readable output
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
      --hdr-tonemapper <NAME>
          Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>
          Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)
      --meta-name <NAME>
          Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>
          Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --perf-name <PERF_NAME>
          Run performance sidecar file name (default saccade-perf.json)
      --perf-noise <PERF_NOISE>
          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <PERF_NOISE_K>
          Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <PERF_RESOLUTION>
          Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <PERF_RESOLUTION_TICKS>
          Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <PERF_MIN_DELTA_MS>
          Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PERF_MIN_DELTA_PCT>
          Minimum meaningful delta as a percentage of the baseline frame (default 0.5)
  -h, --help
          Print help
```

## saccade mcp

```text
passes must resolve under `--root`

Usage: saccade mcp [OPTIONS] --root <ROOTS>

Options:
      --allow-out-near-captures
          Silence warnings when --out is next to capture metadata
      --root <ROOTS>
          Read-only roots (repeatable)
      --out-root <OUT_ROOT>
          Generated artifacts require this separate root
      --record-absolute-paths
          Opt in to absolute local paths in reports and machine-readable output
      --follow-symlinks-within-roots
          
      --symlink-target <SYMLINK_TARGETS>
          
      --allow-provider-calls
          Explicitly authorize provider calls for this MCP server lifetime
      --budget-calls <BUDGET_CALLS>
          Finite startup attempt cap; no implicit MCP allowance
      --user-config <USER_CONFIG>
          Human-owned endpoints, credential bindings and root egress policy
  -h, --help
          Print help
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
      --allow-out-near-captures
          Silence warnings when --out is next to capture metadata
      --record-absolute-paths
          Opt in to absolute local paths in reports and machine-readable output
      --entry <ENTRY>
          
      --status <STATUS>
          
      --limit <LIMIT>
          [default: 10]
      --cursor <CURSOR>
          
      --expected-case-id <EXPECTED_CASE_ID>
          
      --json
          
  -h, --help
          Print help
```

## saccade inspect evidence

```text
Prepare context, crops, facts and references without a provider

Usage: saccade inspect evidence [OPTIONS] --out <OUT> <REPORT>

Arguments:
  <REPORT>  

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --out <OUT>                
      --entry <ENTRIES>          
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
      --top <TOP>                [default: 5]
      --stretch                  
      --blind                    
      --key-out <KEY_OUT>        
      --seed <SEED>              
      --json                     
  -h, --help                     Print help
```

## saccade inspect export

```text
Export an existing artifact or selected entry

Usage: saccade inspect export [OPTIONS] --format <FORMAT> --out <OUT> <ARTIFACT>

Arguments:
  <ARTIFACT>  

Options:
      --allow-out-near-captures      Silence warnings when --out is next to capture metadata
      --format <FORMAT>              [possible values: json, markdown, junit, png, labels]
      --out <OUT>                    
      --record-absolute-paths        Opt in to absolute local paths in reports and machine-readable output
      --entry <ENTRY>                
      --state <STATE>                
      --width <WIDTH>                [default: 1024]
      --artifact-url <ARTIFACT_URL>  
      --comment-key <COMMENT_KEY>    
  -h, --help                         Print help
```

## saccade inspect config

```text
Explain effective measurement settings and their sources

Usage: saccade inspect config [OPTIONS]

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --config <CONFIG>          
      --entry <PATH_OR_NAME>     
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
      --json                     
  -h, --help                     Print help
```

## saccade inspect capabilities

```text
List compiled modules, operations and contracts

Usage: saccade inspect capabilities [OPTIONS]

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --json                     
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
  -h, --help                     Print help
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
      --allow-out-near-captures      Silence warnings when --out is next to capture metadata
      --record-absolute-paths        Opt in to absolute local paths in reports and machine-readable output
      --run                          
      --budget-calls <BUDGET_CALLS>  
      --out <OUT>                    
      --user-config <USER_CONFIG>    
      --intent-file <INTENT_FILE>    
      --intent <INTENT>              
      --json                         
  -h, --help                         Print help
```

## saccade review request

```text
Prepare a closed request from an existing canonical case, locally

Usage: saccade review request [OPTIONS] --question <QUESTION> --out <OUT> <REPORT>

Arguments:
  <REPORT>  

Options:
      --allow-out-near-captures    Silence warnings when --out is next to capture metadata
      --question <QUESTION>        
      --out <OUT>                  
      --record-absolute-paths      Opt in to absolute local paths in reports and machine-readable output
      --user-config <USER_CONFIG>  
      --json                       
  -h, --help                       Print help
```

## saccade review propose

```text
Validate and record proposed answers against the exact request

Usage: saccade review propose [OPTIONS] --answers <ANSWERS> <REQUEST>

Arguments:
  <REQUEST>  

Options:
      --allow-out-near-captures    Silence warnings when --out is next to capture metadata
      --answers <ANSWERS>          
      --out <OUT>                  
      --record-absolute-paths      Opt in to absolute local paths in reports and machine-readable output
      --user-config <USER_CONFIG>  
      --json                       
  -h, --help                       Print help
```

## saccade review ask

```text
Create or retrieve a local human review item for an unresolved request

Usage: saccade review ask [OPTIONS] <REQUEST>

Arguments:
  <REQUEST>  

Options:
      --allow-out-near-captures    Silence warnings when --out is next to capture metadata
      --out <OUT>                  
      --record-absolute-paths      Opt in to absolute local paths in reports and machine-readable output
      --user-config <USER_CONFIG>  
      --json                       
  -h, --help                       Print help
```

## saccade review eval

```text
Plan or run a resumable evaluation manifest

Usage: saccade review eval [OPTIONS] --manifest <MANIFEST>

Options:
      --allow-out-near-captures    Silence warnings when --out is next to capture metadata
      --manifest <MANIFEST>        
      --record-absolute-paths      Opt in to absolute local paths in reports and machine-readable output
      --run                        
      --user-config <USER_CONFIG>  
      --json                       
  -h, --help                       Print help
```

## saccade experiment

```text
Analyze existing graphics captures

Usage: saccade experiment [OPTIONS] <COMMAND>

Commands:
  ablate    Compare ablation arms against a base with image and performance evidence
  sequence  Compare numbered colour frames by sorted index and measure added flicker
  rank      Rank candidate directories against one common FLIP reference
  bisect    Find the first diverging run or revision in an ordered series
  safety    Photosensitivity PRE-CHECK only; not certification or formal compliance
  a11y      Accessibility PRE-CHECK only; not certification or formal compliance

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
  -h, --help                     Print help
```

## saccade experiment ablate

```text
Compare ablation arms against a base with image and performance evidence

Usage: saccade experiment ablate [OPTIONS] <BASE> <ARMS>...

Arguments:
  <BASE>     
  <ARMS>...  

Options:
      --allow-out-near-captures
          Silence warnings when --out is next to capture metadata
      --out <OUT>
          [default: ablation]
      --config <CONFIG>
          
      --record-absolute-paths
          Opt in to absolute local paths in reports and machine-readable output
      --json
          
      --top <TOP>
          Per-term deltas beyond noise to show per arm [default: 5]
      --perf-name <PERF_NAME>
          Run performance sidecar file name (default saccade-perf.json)
      --perf-noise <PERF_NOISE>
          Noise JSON or TOML from unchanged-build repeats
      --perf-noise-k <PERF_NOISE_K>
          Repeat spread multiplier in the effective noise threshold (default 3)
      --perf-resolution <PERF_RESOLUTION>
          Timer quantum in ms; overrides the estimate from repeated captures
      --perf-resolution-ticks <PERF_RESOLUTION_TICKS>
          Minimum timer ticks in the noise threshold (default 2)
      --perf-min-delta-ms <PERF_MIN_DELTA_MS>
          Minimum meaningful delta in ms (default 0.05)
      --perf-min-delta-pct <PERF_MIN_DELTA_PCT>
          Minimum meaningful delta as a percentage of the baseline frame (default 0.5)
  -h, --help
          Print help
```

## saccade experiment sequence

```text
Compare numbered colour frames by sorted index and measure added flicker

Usage: saccade experiment sequence [OPTIONS] <BASELINE_DIR> <CAPTURE_DIR>

Arguments:
  <BASELINE_DIR>  
  <CAPTURE_DIR>   

Options:
      --allow-out-near-captures       Silence warnings when --out is next to capture metadata
      --pattern <PATTERN>             Relative-name glob; frames must end in an integer before the extension [default: *]
      --out <OUT>                     [default: sequence-report]
      --record-absolute-paths         Opt in to absolute local paths in reports and machine-readable output
      --threshold <THRESHOLD>         
      --metric <METRIC>               [possible values: mean, p95, p99, max]
      --config <CONFIG>               
      --ppd <PPD>                     
      --fail-on-new                   
      --allow-empty                   
      --labels <LABELS>               
      --json                          
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)
      --junit <FILE.xml>              Write one JUnit testcase per entry
      --meta-name <NAME>              Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>        Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --require-matching-meta         Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>             Sidecar keys (or globs) that may differ with --require-matching-meta
  -h, --help                          Print help
```

## saccade experiment rank

```text
Rank candidate directories against one common FLIP reference

Usage: saccade experiment rank [OPTIONS] <REFERENCE_DIR> <CANDIDATE_DIRS>...

Arguments:
  <REFERENCE_DIR>      
  <CANDIDATE_DIRS>...  

Options:
      --allow-out-near-captures       Silence warnings when --out is next to capture metadata
      --labels <LABELS>               One unique, safe directory label per candidate, comma separated
      --metric <METRIC>               [default: mean] [possible values: mean, p95, p99, max]
      --record-absolute-paths         Opt in to absolute local paths in reports and machine-readable output
      --out <OUT>                     [default: rank-report]
      --config <CONFIG>               
      --threshold <THRESHOLD>         
      --ppd <PPD>                     
      --fail-on-new                   
      --allow-empty                   
      --json                          
      --hdr-tonemapper <NAME>         Tone mapper for `.exr`/`.hdr` images: aces (default), hable or reinhard
      --hdr-exposures <START:STOP:N>  Exposure range in stops and count, `START:STOP:N` (default: computed from the baseline image)
      --junit <FILE.xml>              Write one JUnit testcase per entry
      --meta-name <NAME>              Sidecar file name (default `saccade-meta.json`); the per-image sidecar is `<stem>.<name>` and overrides the directory-level one
      --meta-ignore <GLOB,...>        Extra sidecar key globs to ignore, added to the built-in timing, timestamp and run-id defaults
      --require-matching-meta         Make an entry an error when a sidecar key differs and is not declared
      --declare <KEY,...>             Sidecar keys (or globs) that may differ with --require-matching-meta
  -h, --help                          Print help
```

## saccade experiment bisect

```text
Find the first diverging run or revision in an ordered series

Usage: saccade experiment bisect [OPTIONS]

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --runs <RUNS>...           Ordered run directories, oldest first (repeatable)
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
      --runs-from <RUNS_FROM>    One ordered run path per line
      --good <GOOD>              Reference for existing runs (default: first run)
      --threshold <THRESHOLD>    Explicit FLIP threshold relaxes native sample identity
      --metric <METRIC>          mean, p95, p99 or max (default max)
      --entries <ENTRIES>        Select image names by glob
      --out <OUT>                Report directory, separate from inputs [default: bisect-report]
      --json                     Print saccade-bisect.v1 JSON
  -h, --help                     Print help
```

## saccade experiment safety

```text
Photosensitivity PRE-CHECK only; not certification or formal compliance

Usage: saccade experiment safety [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Numbered frames or mp4/mov/mkv (requires external ffmpeg)

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --fps <FPS>                Frame rate override; otherwise metadata, or 60 for frame directories
      --display <DISPLAY>        WxH@diagonal_inches,distance_metres (default 1920x1080@55,4)
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
      --standard <STANDARD>      itu-bt1702 or wcag. PRE-CHECK only, never certification [default: itu-bt1702]
      --json                     Print full saccade-safety.v1 JSON
      --out <OUT>                Output directory for JSON, text, HTML, static frames and risk heatmaps [default: safety-report]
      --junit <JUNIT>            Optional JUnit XML destination
  -h, --help                     Print help
```

## saccade experiment a11y

```text
Accessibility PRE-CHECK only; not certification or formal compliance

Usage: saccade experiment a11y [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Opaque sRGB image or image directory

Options:
      --allow-out-near-captures  Silence warnings when --out is next to capture metadata
      --config <CONFIG>          Explicit saccade.toml with [[region]] kind="text" or "ui"
      --json                     Print full saccade-a11y.v1 JSON
      --record-absolute-paths    Opt in to absolute local paths in reports and machine-readable output
      --out <OUT>                Output directory for JSON, text, HTML and simulation/heatmap artifacts [default: a11y-report]
      --junit <JUNIT>            Optional JUnit XML destination
      --suggest-regions          Explicitly upload 16 crops/image to Gemini for unconfirmed region proposals
      --keys-dir <KEYS_DIR>      Judge key policy: gemini.env/SACCADE_GEMINI_API_KEY, never ambient keys
  -h, --help                     Print help
```
