# CI

The [Action example](../examples/action/README.md) documents the inputs and the
baseline-update workflow, which applies an immutable selection of reviewed
entries. The release tag `Tavrin/saccade@v1` has not been published yet.

Compare exits 0 for a passing gate, 1 for a failed gate (including missing or
invalid inputs), and 2 when the command cannot run. Upload the report and JUnit
results even after exit 1. A completed inspection or review does not count as
acceptance. The ablation and ranking commands exit 0 when the analysis
finishes, so read their findings instead of relying on the exit code.

Binary installation verifies release checksums; a source build has to be
requested explicitly. Comparisons on forks need only read permissions and no
AI keys. The job summary is written by default, and PR comments are opt-in. Outputs include `verdict`, `exit-code`,
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

A trusted baseline-update dispatch is bound to a reviewed manifest, the
selected entries, the exact report, candidate and baseline hashes, and the
immutable artifact ID. It opens a pull request for review and does not merge
it automatically. Do not fetch mutable latest-successful data.
CLI receipts remain unattested even when trusted CI authorizes the invocation.

All-features builds, tests and packaging enable `imgtune-avif` and require
**dav1d >= 1.3.0** development files plus **pkg-config**; see the
[installation commands](imgtune.md#system-prerequisites). The CI feature matrix's
all-features entry, CI package and release-check jobs, and release package job
install and check these dependencies before invoking Cargo. These jobs run on
Ubuntu; the current macOS and Windows jobs build without `imgtune-avif`.
Any future job enabling it must install the native dependencies for its OS too.

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
A successful local build says nothing about deployment permissions, fork
runs, native installation or live provider quality; those need release
qualification.
