# Copyable walkthrough

Run these commands from the repository root with an installed CLI. Reports use
new output directories. No provider calls or baseline writes occur.

## Generate a demo

```sh
saccade demo --out saccade-demo
saccade view saccade-demo
```

The demo exits 1 deliberately: its examples include changed and missing captures.
The view command exits 0 and prints the page to open.

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
