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

### Inline PR images (opt in)

Set `comment: 'true'` and `inline-images: 'true'` on the action, with a job token
granted `contents: write` and `pull-requests: write`. `asset-branch` defaults to
`saccade-assets`; it must be a dedicated orphan branch. The action renders at
most three failing entries as 640-pixel thumbnails and heatmap views, with a
2 MiB limit per PNG. It writes a root commit with no parents when the branch is
new, then makes non-force commits on that branch. The sticky comment embeds
raw URLs pinned to the asset commit, while retaining the immutable report
artifact link. On publish failure the comment remains artifact-only. Fork PRs
skip both branch and comment writes; their job summary retains the artifact
link. The action never pushes to the checked-out PR branch.

The token can write repository contents, and the images may disclose private
capture data. Use a separate trusted comment job that consumes only report
artifacts, pins the action code, and does not run PR-supplied scripts with that
token. Avoid `pull_request_target` with an untrusted checkout. The publisher
accepts report entry names only as command arguments and encodes selected
images into fixed asset paths. A pre-existing asset branch without its marker
is rejected. Raw image display for private repositories depends on GitHub's
access handling; use the artifact link if the images do not render. The asset
branch retains old images until the repository owner prunes it. See GitHub's
[Git database API](https://docs.github.com/en/rest/git) and
[workflow token permissions](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#permissions).

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
