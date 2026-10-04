# Docs pass before the public release

Binding brief: `SPEC-docs-pass.md`. Docs, images and the README only; no CLI,
report or schema change.

## Decisions

1. **Internal records moved out.** The dated `finish-*`, `safety-validation-*`,
   `judge-live-*` and `review-live-*` files were session evidence. They now
   live outside the repository with the maintainers' release evidence. The one
   public reference (`examples/evaluation/HISTORICAL.md`) now states the fact
   it needed in place. `docs/design.md` no longer mentions them.
   Reversal: free (move the files back).
2. **CHANGELOG rewritten in Keep a Changelog form.** No version was ever
   tagged, so the old `0.1.0` section and the long "Unreleased" list were
   development history of unpublished builds, and named internal providers and
   tools. The new file has one `[0.1.0]` section (Added, Deprecated, Removed)
   describing what ships, and keeps the deprecated-command and removed-flag
   tables that pre-release users need. The old text is in Git history.
   Rejected: appending a new section above the old ones, which would have
   published a second, contradictory "0.1.0". Reversal: cheap.
3. **README restructured** around pitch, install, 60-second quickstart,
   status and licence. The build-identity/capability table moved to
   `docs/contracts.md` (the README links it from the Status section), because
   it is a scripting contract, not a first-run step. Every command that
   `scripts/docs-smoke.json` expects in the README is still present verbatim.
   Reversal: cheap.
4. **Status section.** It calls `experiment` and `review eval` experimental,
   and says plainly that AI-review accuracy is not qualified, without numbers.
   The stable list follows the active schema families in `docs/contracts.md`.
5. **`cargo install`.** Documented as `--git` (works once the repository is
   public) and `--path crates/saccade` from a clone. crates.io publication is
   not claimed.
6. **Doc screenshots.** No generator existed for `report.png`,
   `report-detail.png` and `viewer.png`. `docs/showcase/tools/capture-doc-images.cjs`
   now captures them from `saccade demo` output (images from saccade's own
   analytic renderer) and a two-directory `saccade view`. It sits next to the
   existing GIF capture tool and is docs tooling, not product code.
   Reversal: free.
7. **Command and link checks live with the release tooling**, outside the
   repository (`check-doc-commands.py`, `check-doc-links.py`), because the
   brief allows no new code here. `scripts/docs-smoke.json` and the
   `r13_docs` test remain the in-repo gate. Reversal: free (copy into
   `scripts/`).
8. **Temporal command uses the existing upscaler frames.** The captures guide
   points `experiment temporal` at the generated 12-frame
   `showcases/upscaler/sequence/{baseline,capture}` fixture. The doc command
   checker runs concrete shell examples from the repository root, and the old
   `baseline-frames/` and `capture-frames/` paths did not exist. Reusing this
   numbered SDR sequence keeps the example runnable through the repository's
   established showcase generator without adding a redundant showcase or
   relying on the checker's placeholder skip. The expected regression exit is
   1, which the checker accepts. Reversal: free.

## Regenerating the visuals

```sh
python3 docs/showcase/build.py --saccade target/release/saccade --out target/pages/showcase
# copy target/pages/showcase/{index.html,assets,media} into docs/showcase/
saccade demo --out target/doc-demo
saccade view target/doc-demo/baseline target/doc-demo/capture --labels baseline,capture --out target/doc-view
PLAYWRIGHT=<playwright-core> node docs/showcase/tools/capture-doc-images.cjs target/doc-demo target/doc-view docs/images
PLAYWRIGHT=<playwright-core> node docs/showcase/tools/capture-gif-frames.cjs target/gif-frames
python3 docs/showcase/tools/assemble-gif.py target/gif-frames docs/images/showcase-swipe.gif
```

## Not done

- `docs/images/blind.png`, `diagnostics.png`, `explain-strip.png`,
  `serve-browse.png` and `serve-landing.png` are not referenced by any doc and
  still show the old `flipdiff` branding. They were outside this brief's list;
  delete or regenerate them before publishing.
- External links (GitHub, Pages, crates.io) were listed, not fetched.
- `docs/showcase/index.html` links to `reports/`, which exists only in the
  Pages build.

Removed by the coordinator at merge (2026-10-04): the five unreferenced images above. They showed the old flipdiff name and remain in git history.
