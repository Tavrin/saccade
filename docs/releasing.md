# Releasing saccade

Run `scripts/release-check.sh` against the exact revision to be tagged. It prints each local PASS/FAIL and names the CI-only gates. Keep `Cargo.lock` fixed. Inspect its package inventory, unpacked-workspace package build and dependency notices output. The CI feature matrix, schema and historical-reader checks run on PRs; `release.yml` runs on a tag or manual dispatch.

1. Push the reviewed branch and open a pull request. The release workflow runs a `v0.2.4` dry run at the PR head commit when release machinery changes; this also works before GitHub recognizes the new `workflow_dispatch` trigger on the default branch. After the workflow lands on the default branch, `gh workflow run release.yml --ref <branch> -f tag=v0.2.4` is another dry-run route. Download `release-dist` and four `qualification-*` artifacts from that run. Confirm `SHA256SUMS` lists all four archives, each result identifies the expected full commit/hash, and all four jobs passed. Neither dry-run route creates a GitHub release.
2. After the dry run and other release checklist evidence are reviewed, create and push the `v0.2.4` tag on **the same commit**. The tag workflow rebuilds on four clean target runners, checks `doctor --json` identity, assembles the checksums, runs `qualify.yml`, and creates a draft only if every job succeeds.
3. Inspect the draft's four archives, `SHA256SUMS`, generated notices and qualification results. Check the release notes and compare the draft's commit with the reviewed tag. Publish the draft manually when the human release decision is complete.

`qualify.yml` is called automatically from `release.yml`. Once the workflow file is on the default branch, it can also be dispatched with the producing release workflow `run_id`, full `commit`, and `tag` to rerun clean-machine checks on its existing artifacts. It verifies the entire checksum manifest, extracts the target archive, checks version/commit, reproduces all showcase `EXPECTED.txt` transcripts, validates schema-bearing showcase JSON, and uploads one result per target. It does not build or publish.

The local script proves formatting, Clippy, workspace and feature tests, both crate archives build after unpacking, package allowlist, license inventory generation, Action lint, historical readers, docs generation, and showcase/schema validation in the current worktree. CI and the tag release workflow also build both packaged crates. CI qualification proves the four clean runner installations and artifact identity. Browser checks, fork PR/update demonstrations, downstream consumer integration, live pilot support, and notarisation need separate human evidence; green local gates alone do not establish them.

Maintainers own all Git pushes, tags, workflow dispatches, and publication. See the [release decisions](design-decisions/release.md) for the notice and fixture policy.

## Portable gate configuration

`scripts/check-public-hygiene.sh` always scans tracked filenames, index bytes,
worktree bytes and symlink targets for author home/mount paths, internal tool
names and root process notes. CI runs it for documentation changes too, and
`scripts/release-check.sh` runs it before builds. Client terms belong only in the
external denylist documented in [genericity](genericity.md).

Gates use Cargo's `target` directory unless `CARGO_TARGET_DIR` is set. Builds
require 25 GiB free on that filesystem. `SACCADE_HEAVY_WRAPPER`, when set, is a
single executable path which receives the command and each argument unchanged.
It has no default; use it for your own queue/admission policy. It must return the
command's exit status. Release commands have a 900-second limit.

Model recipes accept `SACCADE_MODEL_CACHE`; its default is
`$XDG_CACHE_HOME/saccade/models` or `~/.cache/saccade/models`. Feature gates retain
explicit overrides such as `WAVE7_MODEL_CACHE`, `SACCADE_OCR_CACHE` and
`SACCADE_W8_MODEL_DIR`. Heavy model tests require a reviewed `WAVE7_MODEL_CACHE`;
they do not infer a machine-specific fixture location. For the C2PA fixture
example, set `SACCADE_C2PA_FIXTURE_DIR` to a directory containing your local test
`certificate.pem` and `private-key.pem`. Generated files are written there;
private keys must stay outside the repository.

## Python / PyPI Trusted Publishing

One-time setup, performed by the maintainer:

1. On PyPI, create a pending Trusted Publisher for project `saccade-vision`.
   Select GitHub with owner `Tavrin`, repository `saccade`, workflow
   `python-wheels.yml`, and environment `pypi` (names are case-sensitive).
2. Create the GitHub repository environment `pypi`. Configure required reviewers
   and restrict deployment to release tags. No PyPI password or API token is used.
3. Review a manual or PR run of `python-wheels.yml` and download its
   `python-dist-*` artifacts. Confirm the version matches the intended tag and
   inspect the wheel/sdist metadata, README and licenses. All four native wheel
   jobs must pass their installed-wheel smoke and fixture tests.
4. Once approved, push the matching `v0.2.4` tag on the reviewed commit. Only a tag
   push triggers publishing: the `publish` job needs every wheel job and the
   sdist job, requests the `pypi` environment, and exchanges GitHub OIDC identity
   for PyPI access using `pypa/gh-action-pypi-publish`. PRs, branches and manual
   dispatches cannot publish. Approve the environment deployment after review.

Artifacts include Python 3.10+ abi3 wheels for manylinux x86_64/aarch64, macOS
arm64 and Windows x86_64, plus a source distribution. The package installs as
`pip install saccade-vision` and imports as `saccade`. Standard wheels exclude
optional model features. Test a downloaded wheel in a clean virtual environment
with `python crates/saccade-py/tests/smoke_wheel.py`; it generates its image and
uses `cpu-lite` with downloads disabled. The changelog describes the intended
release; publication is complete only after the tag workflow succeeds on PyPI.
