# Releasing saccade

Run `scripts/release-check.sh` against the exact revision to be tagged. It prints each local PASS/FAIL and names the CI-only gates. Keep `Cargo.lock` fixed after the version bump. Inspect its package inventory, unpacked-workspace package build and dependency notices output. The CI feature matrix, schema and historical-reader checks run on PRs; `release.yml` runs on a tag or manual dispatch.

After the one-time owner setup below, a release needs only a version bump,
CHANGELOG update, and a pushed `v<version>` tag on the reviewed commit. Update the
workspace version, internal crate dependency versions, `Cargo.lock`, Python
package version, and both version fields in `server.json` together.

Before tagging, review CI and release checklist evidence. Changes to release
machinery run a `v0.2.4` dry run on the PR head; a manual
`gh workflow run release.yml --ref <branch> -f tag=v0.2.4` also validates a revision.
Inspect `release-dist`, `SHA256SUMS`, and the four `qualification-*` artifacts.
PR and manual runs do not publish to GitHub, crates.io, or the MCP registry.

On a tag push, three independent paths run:

- Binary builds, packaging and four-platform qualification publish the GitHub
  release automatically, including archives, checksums and generated notes.
- `python-wheels.yml` builds and publishes PyPI wheels and the sdist.
- `release.yml` publishes `saccade-core`, then `saccade`, then the MCP manifest.
  These jobs are independent of binary qualification and PyPI. A failure leaves
  any successful publications in place and makes the affected job fail.

Inspect all publication jobs after tagging. For a partial crates.io failure,
rerun failed jobs on the original tag run: an existing, non-yanked version is
skipped with a notice. Registry lookup errors and yanked versions fail; they are
not treated as missing crates. Cargo waits for each new crate to become available
before returning. The MCP job runs only after crates.io succeeds and refuses to
publish unless `server.json` and its cargo package match the tag version.

`qualify.yml` is called automatically from `release.yml`. Once the workflow file is on the default branch, it can also be dispatched with the producing release workflow `run_id`, full `commit`, and `tag` to rerun clean-machine checks on its existing artifacts. It verifies the entire checksum manifest, extracts the target archive, checks version/commit, reproduces all showcase `EXPECTED.txt` transcripts, validates schema-bearing showcase JSON, and uploads one result per target. It does not build or publish.

The local script proves formatting, Clippy, workspace and feature tests, both crate archives build after unpacking, package allowlist, license inventory generation, Action lint, historical readers, docs generation, and showcase/schema validation in the current worktree. CI and the tag release workflow also build both packaged crates. CI qualification proves the four clean runner installations and artifact identity. Browser checks, fork PR/update demonstrations, downstream consumer integration, live pilot support, and notarisation need separate human evidence; green local gates alone do not establish them.

Maintainers own all Git pushes, tags, and workflow dispatches. Tag workflows perform publication automatically. See the [release decisions](design-decisions/release.md) for the notice and fixture policy.

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

## crates.io and MCP one-time owner setup

1. Create the GitHub repository environment `release` in `Tavrin/saccade`.
   Restrict deployment to release tags (`v*`). For unattended tag publication,
   do not require a reviewer or another manual deployment approval.
2. On **each** of `saccade-core` and `saccade` on crates.io, open
   **Settings → Trusted Publishing** and configure GitHub repository
   `Tavrin/saccade`, workflow `release.yml`, environment `release` (case-sensitive).
3. No stored crates.io token is needed. The jobs grant `id-token: write` and use
   the official [crates.io authentication action](https://github.com/rust-lang/crates-io-auth-action)
   pinned to `v1.0.5`, with a fresh temporary token for each crate needing publication.
4. MCP uses the repository owner's `io.github.Tavrin/saccade` namespace from
   `server.json`. GitHub OIDC proves ownership without an interactive login or
   stored registry token. The job downloads
   [mcp-publisher v1.8.1](https://github.com/modelcontextprotocol/registry/releases/tag/v1.8.1),
   verifies its Linux amd64 archive against the SHA-256 pinned in `release.yml`,
   then runs `mcp-publisher login github-oidc` and
   `mcp-publisher publish server.json`. When upgrading, review the upstream
   release and update both the version and archive digest together.

A successful local lint does not verify owner configuration or live publication.
The first tag run must establish those results; authentication, download,
checksum, cargo and MCP publication errors fail the workflow without suppression.

## Python / PyPI Trusted Publishing

One-time setup, performed by the maintainer:

1. On PyPI, create a pending Trusted Publisher for project `saccade-vision`.
   Select GitHub with owner `Tavrin`, repository `saccade`, workflow
   `python-wheels.yml`, and environment `pypi` (names are case-sensitive).
2. Create the GitHub repository environment `pypi` and restrict deployment to
   release tags. For unattended publication, do not configure required reviewers
   or manual deployment approvals. No PyPI password or API token is used.
3. Review a manual or PR run of `python-wheels.yml` and download its
   `python-dist-*` artifacts. Confirm the version matches the intended tag and
   inspect the wheel/sdist metadata, README and licenses. All four native wheel
   jobs must pass their installed-wheel smoke and fixture tests.
4. Once approved, push the matching `v0.2.4` tag on the reviewed commit. Only a tag
   push triggers publishing: the `publish` job needs every wheel job and the
   sdist job, requests the `pypi` environment, and exchanges GitHub OIDC identity
   for PyPI access using `pypa/gh-action-pypi-publish`. PRs, branches and manual
   dispatches cannot publish. No deployment approval is needed with the
   unattended environment configuration above.

Artifacts include Python 3.10+ abi3 wheels for manylinux x86_64/aarch64, macOS
arm64 and Windows x86_64, plus a source distribution. The package installs as
`pip install saccade-vision` and imports as `saccade`. Standard wheels exclude
optional model features. Test a downloaded wheel in a clean virtual environment
with `python crates/saccade-py/tests/smoke_wheel.py`; it generates its image and
uses `cpu-lite` with downloads disabled. The changelog describes the intended
release; publication is complete only after the tag workflow succeeds on PyPI.
