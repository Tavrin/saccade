# CI

The local [Action example](../examples/action/README.md) documents the implemented
inputs and immutable selected baseline-update workflow.
The proposed release tag `Tavrin/saccade@v1` is not claimed to be published.

Compare exits 0 for a passing gate, 1 for a failed gate (including missing or
invalid inputs), and 2 when the command cannot run. Upload the report and JUnit
results even after exit 1. Inspection/review completion supplies no acceptance
authority. Ablation/rank exit 0 when analysis completes; inspect their findings.

Binary installation verifies release checksums. Source builds are explicit.
Fork comparisons require only read permissions and no AI keys. Summaries are
default; PR comments are opt-in. Outputs include `verdict`, `exit-code`,
`report-url` and immutable `artifact-id`.

Trusted baseline-update dispatch binds a reviewed manifest, selected entries,
exact report/candidate/baseline hashes and immutable artifact ID. It creates a
review PR and does not auto-merge. Do not fetch mutable latest-successful data.
CLI receipts remain unattested even when trusted CI authorizes the invocation.

Local validation:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
.tools/actionlint
```

Full showcase validation requires `cargo build --release -p saccade --features
prechecks` and that release executable on PATH. Run `scripts/run-showcases.sh`.
For the default-feature Pages build, run:

```sh
cargo build --release --locked -p saccade
python3 docs/showcase/build.py --saccade target/release/saccade --out target/pages/showcase
```

The experimental case is listed as unvalidated without prechecks. With prechecks,
the gallery executes it too. The gallery never animates flashing fixtures.
Local build success does not prove deployment permissions, fork execution,
native installation or live provider quality. Those require release qualification.
