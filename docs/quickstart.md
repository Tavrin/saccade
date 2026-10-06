# Copyable walkthrough

Run these commands from the repository root with an installed CLI. Reports use
new output directories. No provider calls or baseline writes occur.

## Which command for which task

Pick by task. The canonical column is what the documentation uses; the
other-spellings column lists names that run the same code or enforce it, and keep working
(no command has been removed or renamed; spellings marked deprecated print their replacement). `saccade --help` ends with the exit codes.

| Task | Canonical command | Other spellings | Guide |
| --- | --- | --- | --- |
| Did screenshots or renders change? | `saccade compare BASE CAPTURE --out DIR` | `watch` (deprecated, prints the replacement) | [visual CI](guides/visual-ci.md) |
| Is a refactor pixel-identical? | `saccade prove identity PARENT CANDIDATE` | `saccade identity PARENT CANDIDATE` | [visual CI](guides/visual-ci.md) |
| Did it get faster, accounting for noise? | `saccade prove performance --base ... --arm ...` | `saccade experiment ablate` | [controlled rendering](guides/controlled-rendering.md) |
| Are two capture setups comparable? | `saccade arms check A B` | standalone check; `compare --require-valid-arms` enforces it | [controlled rendering](guides/controlled-rendering.md) |
| Did a document or page export change? | `saccade compare BEFORE AFTER` (PNG pages, or SVG/PDF with `documents`) | | [document export](guides/document-export.md) |
| Which images are near-duplicates, which are unreadable? | `saccade dedupe DIR`, `saccade hash DIR` | | [media intake](guides/media-intake.md) |
| Is a re-encoded image still close enough? | `saccade compare SOURCE SHIPPED`, `saccade imgtune search` | | [delivery tuning](guides/delivery-tuning.md) |
| Score one image with a learned metric | `saccade quality-score IMAGE` | `saccade score IMAGE` | |
| What does a finished report say? | `saccade inspect REPORT`, `saccade review REPORT` | `explain` and `snapshot` (deprecated) | [visual CI](guides/visual-ci.md) |

Each guide has a known-good case, a known-bad case, a missing-input case and an
unavailable-dependency case, and `scripts/test-guides.py` runs them all against a
built binary, so the exit codes in the text are the tested ones.

## Exit codes

| Exit | Meaning |
| --- | --- |
| 0 | Success: no regression, claim proven, or the requested output was written |
| 1 | Regression found, or the claim was not proven (a pair differs, is missing, new or unreadable; nothing was compared) |
| 2 | The command could not run: usage, config, unreadable input, or a feature or model this build does not have |
| 3 | Strict producer check refused: an undeclared difference (`--require-valid-arms`, `arms check`) |
| 4 | Strict producer check refused: a required key is missing (`--require-valid-arms`, `arms check`) |

A command that cannot produce a measurement never exits 0. Exit 1 is evidence,
not a tool failure.

## What this build can run

```sh
saccade doctor
```

`doctor` prints the compiled features, which command groups are available or
unavailable in this build (with the feature each needs), and which optional
models, runtimes and binaries are present or missing, with the fix command for
each. Model-based commands (`locate`, `faces`, `crop-check`, `quality-score`)
need a build with `local-models`, a pulled runtime and a pulled model; on a stock
install they exit 2 and say so. They have no default that silently downloads
anything.

## Threshold units

- `--threshold` on `compare` and `prove identity` is a FLIP score from 0 to 1
  (0 means identical). A pair fails when its `--metric` value (`mean`, `p95`,
  `p99`, `max`) is above it. Identity has no threshold: any differing sample fails.
- `--ppd` is the viewing condition in pixels per degree of visual angle
  (default 67); it changes how visible a difference is, not which pixels differ.
- `dedupe --threshold` is a perceptual-hash distance in bits, 0 to 64.
- Rectangles in config files are `[x, y, width, height]` as fractions of the
  image (0 to 1).
- Side names: the directory you trust is the baseline or reference; the one you
  are checking is the capture or candidate.

## Generate a demo

```sh
saccade demo --out saccade-demo
saccade view saccade-demo
```

The demo exits 1 deliberately: its examples include changed and missing captures.
The view command exits 0 and prints the page to open.

First-run rules worth knowing before wiring a pipeline:

- An empty baseline directory compares nothing, and nothing compared exits 1
  ("nothing compared"); `--allow-empty` accepts it for the very first run.
- An image present in the capture but not in the baseline is reported as `new`
  and does not fail by default; add `--fail-on-new` to treat it as a regression.
  An image in the baseline with no capture is `missing` and fails.

## Compare and inspect

```sh
saccade compare examples/baseline/sphere_shadow.png examples/capture/sphere_shadow.png --out file-report
saccade compare examples/baseline examples/capture --out directory-report --json
saccade inspect directory-report/saccade-report.v1.json --status fail,error,missing,new --limit 5 --json
```

Both comparisons exit 1 because the procedural captures differ. Inspection exits
0 and pages through the failed entries. Exit 2 means the command could not run.
See [capture policies](captures.md) and [result contracts](contracts.md).

```sh
saccade inspect evidence directory-report/saccade-report.v1.json --entry sphere_shadow.png --out evidence-pack
saccade inspect export directory-report/saccade-report.v1.json --format png --entry sphere_shadow.png --out shadow.png
```

These commands exit 0 and write an evidence pack and an exported image.

## Configure comparisons

```sh
saccade init --template renderer --dir capture-project
saccade inspect config --config capture-project/saccade.toml --entry scene.png --json
```

Both exit 0. Effective settings record declared thresholds, regions, masks and
capture requirements; command-line settings override project settings.

## Prove exact image identity

```sh
saccade identity saccade-demo/identity/baseline saccade-demo/identity/capture --out identity-report --json
```

This exits 0: the demo files differ in encoded bytes but have equal native decoded
samples. The proof covers the supplied complete pairs and does not qualify a
speedup or application correctness. See [identity/performance](identity-and-performance.md).

## Prepare local review

```sh
saccade review directory-report/saccade-report.v1.json --out review-plan --json
saccade review request file-report/saccade-report.v1.json --question triage.route.v1 --out triage-request.json
saccade review ask triage-request.json --out human-review
```

All exit 0 without calling a provider. The file report supplies a complete image
pair for the closed question. The preview includes exact payloads and token/cost
estimates; proposals remain advice. See [review](review.md).

## Start the agent server

```sh
saccade mcp --root examples --out-root agent-reports
```

The server exits 0 on a normal EOF/shutdown. It limits reads and report writes to
separate roots and grants no provider or baseline-write authority. See
[agents](agents.md) and [plugin setup](plugins.md).
