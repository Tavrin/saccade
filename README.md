# saccade

Saccade compares captures, records the limits of the comparison, and prepares
visual evidence for a human or coding agent to review.
It uses NVIDIA FLIP for perceptual image differences and native decoded samples
for exact identity. Models can supply observations and proposals. A human
authorizes baseline changes.

![Report with image differences and numbered hotspots](docs/images/report.png)

## Install

Download a binary archive and its checksum file from
[Releases](https://github.com/Tavrin/saccade/releases), when available.
Choose the archive for your operating system and CPU, verify its SHA-256 against
the release checksum file, extract it, and put `saccade` on PATH.
No account or Rust installation is required for a prebuilt binary.
The repository does not claim that a 1.0 release or `v1` Action tag is published.

For a source build, use Rust 1.88 or newer:

```sh
cargo build --release --locked -p saccade
```

The executable is `target/release/saccade` (`saccade.exe` on Windows).
Set `CARGO_TARGET_DIR` before building if you use another build directory.
Add that directory's `release` folder to PATH for the commands below.

The official CLI enables graphics analysis, optional AI review, the local
workbench, MCP and evaluation. AI execution requires authorization.
The library defaults to parallel comparison only. Experimental prechecks are off.

## Demo

```sh
saccade demo --out saccade-demo
saccade view saccade-demo
```

The demo exits **1** because it contains an intentional regression and a missing
capture. It retains the real comparison verdict. `view` identifies the report;
open `saccade-demo/report/index.html` in your browser.

The bundle includes a renderer defect, a UI label defect, equal native samples
with different PNG encodings, and an intended-change review fixture.
Review responses and resolution are illustrative offline records.
No provider is called and no baseline is updated.
[Demo provenance](crates/saccade/assets/demo/provenance.json) records sources,
licenses and hashes. The CPU sphere render uses the repository's own analytic
renderer; it contains no imported assets.

## Compare files or directories

```sh
saccade compare examples/baseline/sphere_shadow.png examples/capture/sphere_shadow.png --out file-report
saccade compare examples/baseline examples/capture --out directory-report --json
saccade inspect directory-report/saccade-report.v1.json --status fail,error,missing,new --limit 5 --json
saccade inspect evidence directory-report/saccade-report.v1.json --entry sphere_shadow.png --out evidence-pack
saccade inspect export directory-report/saccade-report.v1.json --format png --entry sphere_shadow.png --out shadow.png
```

Both comparisons intentionally exit 1. Inspection and export exit 0 on completion.
The default metric is mean FLIP, the threshold is 0.01, and the viewing setting
is 67 pixels per degree. These are configured criteria. A threshold pass does
not prove that every human would miss a change or that the renderer is correct.

Use `saccade.toml` for measurement settings, regions, masks, capture requirements,
and declared changes. Explicit CLI settings override the selected project values.
`inspect config` explains the effective settings and their sources.

```sh
saccade init --template renderer --dir capture-project
saccade inspect config --config capture-project/saccade.toml --entry scene.png --json
```

Pairing uses relative image names. New and missing images fail by default.
An empty comparison is not evidence. Capture metadata can refuse a comparison
made under different settings; absent context remains unknown.
See [capture configuration](docs/captures.md).

## Optimization identity

```sh
saccade identity saccade-demo/identity/baseline saccade-demo/identity/capture --out identity-report --json
```

Exit 0 establishes exact native decoded-sample equality for the selected images.
The demo's PNG files have different bytes but equal samples.
Dimensions, sample type and channel interpretation must match; every selected
pair must exist and decode. Perceptual tolerances, masks and region-only
acceptance cannot establish identity.

The proof covers the supplied captures and named scope. Missing metadata means
sample equality can be established while capture comparability stays unknown.
Image identity alone cannot establish a speedup.
See [identity and performance](docs/identity-and-performance.md).

## Optional BYOK review

Start with a local preview:

```sh
saccade review directory-report/saccade-report.v1.json --out review-plan --json
saccade review request file-report/saccade-report.v1.json --question triage.route.v1 --out triage-request.json
saccade review ask triage-request.json --out human-review
```

Use the completely paired file report for the request; missing required facts
block question creation. These commands do not call a provider. Requests identify
exact evidence. `review propose` validates answers against a request and records
advice. It supplies no approval authority.

For live use, the human configures provider endpoints and dedicated credentials
in user configuration, allows egress for the source roots, and explicitly runs
`review REPORT --run --budget-calls N`. Unclassified roots deny egress.
Project files may select approved models and reduce budgets; they cannot grant
network, credential, path or approval authority.

Every retry and fallback spends from the shared attempt ledger. Preview payload
hashes and source policy before authorizing execution. Denied capture provenance
also restricts derived reports, crops and observations.
See [review](docs/review.md) for the workflow and limits.

## GitHub Action

The release syntax is proposed; `v1` is not claimed to be published:

```yaml
- uses: Tavrin/saccade@v1
  with:
    baseline-dir: tests/baseline
    capture-dir: artifacts/captures
```

Binary installation is the default and verifies checksums.
`install-mode: source` selects an explicit source build.
A failed comparison still uploads the report and JUnit results.
Fork comparisons require read permission and no AI keys.

Outputs distinguish verdict, exit code, report URL and immutable artifact ID.
Baseline updates require a trusted dispatch bound to selected reviewed content.
They create a review PR without auto-merge.
See [CI](docs/ci.md) and [Action examples](examples/action/README.md).

## Coding agents

Read the [agent workflow](docs/agents.md), then install the compact
[Claude Code skill](integrations/claude-code/skills/saccade/SKILL.md) or
[Codex instructions](integrations/codex/AGENTS.saccade.md).
Both are generated from [one guide](integrations/agent-guide.md).

A local MCP server confines inputs to registered read-only roots and outputs to
an explicit output root:

```sh
saccade mcp --root examples --out-root agent-reports
```

The six tools measure, inspect, prepare evidence, review, propose and ask a human.
Provider calls are off unless a human starts the server with both
`--allow-provider-calls` and a positive finite `--budget-calls`.
There is no baseline-write tool.

Read bounded JSON before full artifacts. Summaries preserve validity, missingness,
counts and up to three next actions. Follow pagination instead of reading every
image. Keep measured facts, model observations and inference distinct.
Never relax a threshold or update a baseline to make a task pass.

## Reproducible showcases

<!-- showcase-count:start -->
[9 reproducible cases](showcases/README.md) with commands, expected exits and measured output.
[Pages gallery](https://tavrin.github.io/saccade/showcase/).
<!-- showcase-count:end -->

The datasets are procedural; simulated timings are labeled illustrative.
Generation requires Python 3, Pillow and numpy. No network, GPU or provider is
used. Showcase validation requires a build with `--features prechecks`:

```sh
cargo build --release --locked -p saccade --features prechecks
scripts/run-showcases.sh
python3 docs/showcase/build.py --saccade target/release/saccade --out target/pages/showcase
```

Put the built binary on PATH before running the showcase script.
The script checks exit codes and reproduces measured `EXPECTED.txt` transcripts.
It never accepts changed output automatically.
The Pages gallery also builds with the default CLI; its experimental fixture is
listed as unvalidated when `prechecks` is absent.

## Limits

Saccade measures supplied captures. It does not run a renderer or capture
scheduler, infer application readiness, or establish correctness from equality.
FLIP depends on viewing conditions; HDR display transforms and numerical buffers
need explicit interpretation. Missing repeat noise is unknown, not zero.
Performance claims need qualified timing, repeat noise and attribution.

Models cannot establish equality, qualify timing, approve baselines or resolve a
tie into acceptance. Contradictory blind orders and ambiguous intent remain
unresolved. Independent blind review requires a reviewer without the mapping or
implementation context; a private key cannot blind its creator.

Human-final is an application policy and audit boundary in the user's trust
environment. An unrestricted shell agent can invoke CLI approval.
Workbench receipts use scoped token attestation; CLI receipts disclose
`human_attestation: null`. Historical promoted records require fresh review.

Native platform installation, live provider quality and release deployment need
separate qualification. Local fixtures do not establish those outcomes.

## Documentation and license

- [Captures](docs/captures.md) and [identity/performance](docs/identity-and-performance.md)
- [Review](docs/review.md), [agents](docs/agents.md), [CI](docs/ci.md)
- [Contracts and generated schema index](docs/contracts.md)
- [Evaluation](docs/evaluation.md), [command reference](docs/cli.md), [experimental checks](docs/experimental.md)
- [Architecture](docs/design.md), [contributing](CONTRIBUTING.md), [changes](CHANGELOG.md)

Saccade is licensed MIT OR Apache-2.0. NVIDIA's FLIP code is BSD-3-Clause;
see [third-party notices](THIRD_PARTY.md).
