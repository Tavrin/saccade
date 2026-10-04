# saccade

saccade tells you when visual or performance evidence is not good enough to
support a claim. `compare` measures image changes, `prove` checks exact image
identity or performance claims, and `review` prepares a human decision. It
pairs captures by name, scores visual differences with NVIDIA FLIP, and writes
an offline HTML report and JSON evidence. Baselines change only when a human
approves.

![Report with image differences and numbered hotspots](docs/images/report.png)

## Install

**Release archive.** Each release has four archives and a `SHA256SUMS` file:
`saccade-x86_64-unknown-linux-gnu.tar.gz`, `saccade-aarch64-unknown-linux-gnu.tar.gz`,
`saccade-aarch64-apple-darwin.tar.gz` and `saccade-x86_64-pc-windows-msvc.zip`.
Download the one for your system from
[Releases](https://github.com/Tavrin/saccade/releases), check it, extract it,
and put `saccade` on your PATH:

```sh
sha256sum -c SHA256SUMS --ignore-missing
tar -xzf saccade-x86_64-unknown-linux-gnu.tar.gz
```

On macOS use `shasum -a 256 -c SHA256SUMS --ignore-missing`.
No Rust toolchain is needed. Each archive includes the licences and
third-party notices. How releases are built and checked: [releasing](docs/releasing.md).

**From source with Cargo** (Rust 1.88 or newer):

```sh
cargo install --locked --git https://github.com/Tavrin/saccade saccade
```

From a clone, `cargo install --locked --path crates/saccade` does the same.
Add `--features prechecks` for the experimental safety and accessibility checks.

Check the install with `saccade --version` or `saccade doctor --json`.

## Quickstart (60 seconds)

```sh
saccade demo --out saccade-demo
saccade view saccade-demo
```

The demo writes three reports and exits **1** on purpose: it contains a moved
shadow, a changed UI label and a missing capture. `view` prints the report
page to open: `saccade-demo/report/index.html`. Then compare your own images:

```sh
saccade compare BASELINE_DIR CAPTURE_DIR --out report
saccade prove identity PARENT_DIR CANDIDATE_DIR --out proof
```

Exit 0 means no image regression, 1 means at least one image failed, and 2
means the command could not run. Open `report/index.html` to see the result.
No network, GPU or account is used.

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

This uses the demo from the quickstart:

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

## Optional model review (bring your own key)

Start with a local preview. These commands do not call a provider:

```sh
saccade review directory-report/saccade-report.v1.json --out review-plan --json
saccade review request file-report/saccade-report.v1.json --question triage.route.v1 --out triage-request.json
saccade review ask triage-request.json --out human-review
```

The preview writes the exact payloads to `review-plan/requests.json`, with
estimated tokens and, if you configure model rates, an estimated cost.
Use the completely paired file report for the request; missing required facts
block question creation. Requests identify exact evidence. `review propose`
validates answers against a request and records advice. It supplies no
approval authority.

For live use, the human configures provider endpoints and dedicated credentials
in user configuration, allows egress for the source roots, and explicitly runs
`review REPORT --run --budget-calls N`. Unclassified roots deny egress.
Project files may select approved models and reduce budgets; they cannot grant
network, credential, path or approval authority. Every retry and fallback
spends from the shared attempt budget.
See [review](docs/review.md) for the workflow and limits.

## GitHub Action

```yaml
- uses: Tavrin/saccade@v1
  with:
    baseline-dir: tests/baseline
    capture-dir: artifacts/captures
```

The `v1` tag is not published yet; this is the intended syntax.
Binary installation is the default and verifies checksums.
`install-mode: source` selects an explicit source build.
A failed comparison still uploads the report and JUnit results.
Fork comparisons require read permission and no AI keys.

Outputs distinguish verdict, exit code, report URL and immutable artifact ID.
Baseline updates require a trusted dispatch bound to selected reviewed content.
They create a review PR without auto-merge.
See [CI](docs/ci.md) and [Action examples](examples/action/README.md).

## For AI agents

Start with the [agent guide](docs/agents.md). It explains the bounded JSON
results (`verdict`, `worst`, `next_actions`), how to page through failures,
and what an agent may not do. Ready-made packs are generated from
[one guide](integrations/agent-guide.md): a
[Claude Code skill](integrations/claude-code/skills/saccade/SKILL.md) and
[Codex instructions](integrations/codex/AGENTS.saccade.md).

```sh
saccade mcp --root examples --out-root agent-reports
```

The MCP server has six tools. Inputs are confined to read-only `--root`
directories and outputs to `--out-root`. Provider calls are off unless a human
starts the server with `--allow-provider-calls` and a positive `--budget-calls`.
There is no baseline-write tool. Agents never relax a threshold or update a
baseline to make a task pass.

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

## Status

saccade 0.1.0 is the first public release.

- **Stable:** `compare`, `identity`, `approve`, `view`, `inspect`, `init`,
  `noise`, `demo`, `serve`, the local `review` preview and request commands,
  the MCP server, the HTML and JSON reports, exit codes, and the GitHub Action
  inputs. The active formats are listed in [contracts](docs/contracts.md).
  Scripts should gate on `doctor --json` capability names
  ([list](docs/contracts.md#build-identity-and-behavior-capabilities)).
- **Experimental:** the `experiment` commands (ablation, sequences, ranking,
  bisection, and the safety and accessibility prechecks) and `review eval`.
  Their output may change.
- **AI review accuracy is not yet qualified.** The pilot evaluation has fewer
  labeled cases per question than qualification requires, so no claim is made
  about how accurate any model's answers are, or which model is better. Treat
  every model answer as a proposal for a human to check.
  See [evaluation](docs/evaluation.md).

Native installation on each platform and live provider behaviour need their
own checks; passing local tests does not establish them.

## Limits

saccade measures supplied captures. It does not run a renderer or capture
scheduler, infer application readiness, or establish correctness from equality.
FLIP depends on viewing conditions; HDR display transforms and numerical buffers
need explicit interpretation. Missing repeat noise is unknown, not zero.
Performance claims need qualified timing, repeat noise and attribution.

Models cannot establish equality, qualify timing, approve baselines or resolve a
tie into acceptance. Contradictory blind orders and ambiguous intent remain
unresolved. Independent blind review requires a reviewer without the mapping or
implementation context; a private key cannot blind its creator.

"Human-final" approval is a policy and audit record, not authentication: an
unrestricted shell agent can still run `saccade approve`. Workbench receipts
record a scoped token attestation; CLI receipts record `human_attestation: null`.

## Documentation

- [Captures](docs/captures.md) and [identity/performance](docs/identity-and-performance.md)
- [Review](docs/review.md), [agents](docs/agents.md), [CI](docs/ci.md)
- [Contracts and generated schema index](docs/contracts.md)
- [Evaluation](docs/evaluation.md), [command reference](docs/cli.md), [experimental checks](docs/experimental.md)
- [Architecture](docs/design.md), [contributing](CONTRIBUTING.md), [changes](CHANGELOG.md)

## License

saccade is licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE), at your option. FLIP comes from the pure-Rust
[flip-rs](https://crates.io/crates/flip-rs) port of NVIDIA FLIP, which is
BSD-3-Clause. See [third-party notices](THIRD_PARTY.md); release archives
also carry a generated `THIRD_PARTY_NOTICES.md` covering every dependency.
