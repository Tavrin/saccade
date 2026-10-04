# Action examples

The proposed release invocation is `Tavrin/saccade@v1`; that tag is not yet
published. Binary installation is the default and requires a matching release
archive and SHA-256. Missing releases, unsupported targets, bad checksums and
non-running executables are installation errors. `install-mode: source` is an
explicit alternative requiring Rust. With no version it builds the downloaded
action checkout; with a version it builds that git revision with `--locked`.

`.github/workflows/example-usage.yml` uses the local action and explicit source
installation on procedural examples. Fork comparisons need only `contents:
read`, no persisted checkout credentials, and no AI keys. Summaries are enabled;
PR comments are opt-in and are skipped for forks. A regression still uploads
the report, `.saccade-run` marker and `junit.xml` before failing. Outputs are
`verdict` (`pass`, `fail`, `command-error`), `exit-code` (0, 1, 2), `report-url`
and the immutable `artifact-id`. The report artifact alone is not an update
bundle: updates also need the original captures and selected plan.

For inline images, use `comment: 'true'` and `inline-images: 'true'` in a
trusted same-repository PR comment job with `contents: write` and
`pull-requests: write`; see [the security model](../../docs/ci.md#inline-pr-images-opt-in).
The default `saccade-assets` branch is created as an orphan and only holds
bounded preview PNGs. Fork PRs and failed asset pushes keep the artifact-linked
summary without branch writes. Never run PR-supplied build scripts in the job
that holds this write token.

For a reviewed update, prepare evidence from the repository root:

```sh
saccade compare examples/baseline examples/capture --out saccade-report
# A comparison gate failure (exit 1) still produces reviewable evidence.
saccade approve --report saccade-report/saccade-report.v1.json \
  --entry sphere_shadow.png --dry-run --out saccade-plan
```

Review the exact report, selected `manifest.json`, and adjacent `decision.json`.
The decision is an unattested CLI record; the trusted dispatch supplies CI
authorization. Archive these three trees together, preserving their relative
layout: `examples/capture/`, `saccade-report/`, `saccade-plan/`. Upload that data
bundle with `actions/upload-artifact@v4` in your trusted evidence workflow and
record its run ID and artifact ID. Compute the SHA-256 of the reviewed manifest
bytes with `sha256sum saccade-plan/manifest.json` (or `shasum -a 256` on macOS).
Do not overwrite, recompute or fetch a newer report or captures for the update.

Dispatch `.github/workflows/example-update-baselines.yml` on the default branch
with that run ID, immutable artifact ID and manifest hash. It restores only data
trees, verifies current baseline/candidate/report/decision hashes, checks that
the dry-run equals the pinned manifest, and applies only those entries. A changed
baseline requires new evidence and review. Deletions require both the decision's
explicit deletion approval and `update-prune-missing: 'true'`. The workflow
creates a review PR and never auto-merges it. The original comparison verdict
and exit code remain visible, including a failing reviewed comparison.

The report outputs identify the current report upload. To make an update bundle,
retain the evidence workflow's separate bundle artifact ID, not a mutable
artifact-name or latest-successful-run lookup. Local tests mock installation and
GitHub writes; end-to-end fork behavior and native clean installation remain
release qualification tasks.
