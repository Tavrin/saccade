# flipdiff

Perceptual visual-regression testing for renderers, game engines and graphics code. flipdiff compares a directory of baseline images with a directory of fresh captures using [NVIDIA FLIP](https://github.com/NVlabs/flip), writes a JSON report, a self-contained HTML report (side by side, flicker, swipe, heatmap) and a Markdown summary, and exits non-zero on a regression. It ships as a CLI and a GitHub Action.

FLIP models how a human observer sees the difference between two images (colour, contrast sensitivity, spatial frequency, viewing distance). RMSE and pixelmatch count pixel differences: a one-pixel shift of fine detail or a 1-bit dither change scores the same as a visible colour error, and any threshold on them is either too noisy or too blind for rendered images with anti-aliasing, noise and sub-pixel variation. FLIP scores such changes by how visible they are.

What flipdiff does not do: it does not run your renderer or capture images, it handles 8-bit sRGB PNG and JPEG (16-bit PNGs are accepted and down-converted to 8 bits; no HDR or EXR), it has no masks or ignore-regions, it does not store baselines or host images, and it cannot tell you whether a visible change is a bug. Different GPUs and drivers produce different pixels, so keep baselines per hardware class or use a threshold that absorbs it.

## Quickstart: CLI

```sh
cargo install --git https://github.com/OWNER/flipdiff flipdiff --locked

flipdiff compare baseline/ capture/ --out report/ --threshold 0.01
open report/index.html

flipdiff approve capture/ baseline/ --all-failing report/flipdiff-report.v1.json
flipdiff summary report/flipdiff-report.v1.json --format markdown
```

Building from source compiles NVIDIA's C++ FLIP code through the `cc` crate, so a C++ compiler is required (g++, clang or MSVC).

```
flipdiff compare <BASELINE_DIR> <CAPTURE_DIR> [--out report] [--threshold F] [--metric mean|p95|max]
                 [--config flipdiff.toml] [--fail-on-new] [--json] [--ppd F]
flipdiff approve <CAPTURE_DIR> <BASELINE_DIR> [NAMES...] [--all-failing <REPORT_JSON> [--include-errors]]
flipdiff summary <REPORT_JSON> [--format markdown|text] [--artifact-url URL] [--comment-key KEY]
flipdiff view <DIR>... [--labels a,b] [--reference X] [--blind] [--seed N] [--out view]
flipdiff unblind <DECISIONS_JSON> <BLIND_KEY_JSON> [--out FILE]
```

- Images are paired by path relative to each directory (`png`, `jpg`, `jpeg`, any case, recursive).
- An image passes when its metric value is `<= threshold`. The metric is a statistic of the FLIP error map: `mean`, `p95` or `max`. Values run from 0 (identical) to 1.
- `approve` copies captures over baselines. Name the images, or take every failing and new entry from a report. `--include-errors` also takes `error` entries whose capture decodes (for example a deliberate size change). `approve` refuses to write through a symlink inside the baseline directory.
- `--ppd` sets pixels per degree of visual angle (default 67, a typical desktop viewing distance). It must be finite and greater than 0.
- **Alpha:** when either image has an alpha channel below 255, both are composited over black and over white, FLIP runs on each, and the per-pixel maximum is used. An alpha-only change is flagged, and RGB hidden under fully transparent pixels is ignored.
- **Symlinks** in the baseline or capture directory are not followed; each becomes an `error` entry. Unreadable files and directories are `error` entries too, not an aborted run.
- Each run first removes `flipdiff-report.v1.json`, `index.html` and `images/` from the report directory (nothing else), so a failed run never leaves a stale report.

### Blind comparison with `view`

`flipdiff view a/ b/ --blind --out view/` writes a self-contained page for pairwise judging. The page embeds only neutral labels (`P1`, `P2`, ...) and the seed, so view-source does not reveal which directory is which. The true labels are in `view/blind-key.json`, which the page does not reference: keep it away from the judge. After deciding every set the judge picks "Reveal" and selects that file, or hands the exported decisions back and you run `flipdiff unblind decisions.json view/blind-key.json` to get the true labels.

Try it on the generated examples (`python3 scripts/gen-examples.py` regenerates them):

```sh
flipdiff compare examples/baseline examples/capture --out /tmp/flipdiff-report
```

## Quickstart: GitHub Action

```yaml
on: pull_request
jobs:
  visual:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: ./render-tests.sh --out captures/   # your renderer
      - uses: dtolnay/rust-toolchain@stable      # needed only if no release binary matches
      - uses: OWNER/flipdiff@v0.1.0
        with:
          baseline-dir: tests/baseline
          capture-dir: captures
```

The action installs a prebuilt `flipdiff-<target>.tar.gz` (or `.zip`) from the release for the requested ref, verifies it against the `.sha256` file published next to it and runs `flipdiff --version`; on a missing asset, a missing or mismatched checksum, or a binary that does not run, it falls back to `cargo install --git`. It then runs `compare`, uploads the report directory as an artifact, writes the summary to the job summary, updates one sticky pull-request comment (found by the `<!-- flipdiff-summary -->` marker), and fails the job with the compare exit code.

| Input | Default | Meaning |
|---|---|---|
| `baseline-dir`, `capture-dir` | required | Image directories |
| `threshold`, `metric`, `config` | empty | Override `flipdiff.toml` defaults |
| `report-dir` | `flipdiff-report` | Output directory. Use a path relative to the workspace: an absolute path is compared and summarised but not uploaded as an artifact |
| `artifact-name` | `flipdiff-report` | Name of the uploaded artifact |
| `comment-key` | empty | Makes the sticky comment unique (marker `<!-- flipdiff-summary:<key> -->`) |
| `fail-on-new` | `false` | New images count as a regression |
| `comment` | `true` | Sticky PR comment (pull_request events only) |
| `github-token` | `github.token` | Used for the comment and release download |
| `version` | the action's own ref | Git ref of this repository to install |

**Matrix jobs.** `upload-artifact` v4 rejects two artifacts with one name, and all jobs would share one comment. Give each job its own `artifact-name` and `comment-key`:

```yaml
strategy:
  matrix:
    gpu: [nvidia, intel]
steps:
  - uses: OWNER/flipdiff@v0.1.0
    with:
      baseline-dir: tests/baseline/${{ matrix.gpu }}
      capture-dir: captures
      report-dir: flipdiff-report-${{ matrix.gpu }}
      artifact-name: flipdiff-report-${{ matrix.gpu }}
      comment-key: ${{ matrix.gpu }}
```

Outputs: `exit-code`, `failed` (count of failed images), `new`, `report-dir`.

The comment needs `pull-requests: write` on the token. On pull requests from forks the token is read-only, so the comment step prints a warning and the job result is unaffected. The comment links to the report artifact; the images themselves are not embedded in the comment.

`.github/workflows/example-usage.yml` runs the action on `examples/` by hand (`workflow_dispatch`). The examples contain a deliberate regression, so that run fails.

## `flipdiff.toml`

```toml
threshold = 0.01          # default threshold
metric = "mean"           # mean | p95 | max
fail_on_new = false
ppd = 67.0                # optional, pixels per degree
ignore = ["**/debug_*.png"]

[[override]]
glob = "terrain/**"
threshold = 0.03
metric = "p95"
```

`--config` defaults to `./flipdiff.toml` when that file exists. CLI flags override the top-level defaults; `[[override]]` tables always apply on top, and the first matching override wins, field by field. Globs match the `/`-separated image name, ignoring case; `*` does not cross `/`, `**` does. Unknown keys are errors.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | No regression |
| 1 | Regression: any `fail`, `error` or `missing` entry, or a `new` entry with `fail_on_new` |
| 2 | Usage, config or IO error |

## Report layout

```
report/
  index.html                     self-contained HTML report (no external requests)
  flipdiff-report.v1.json        machine-readable report
  images/<name>/baseline.<ext>   copies of both inputs
  images/<name>/capture.<ext>
  images/<name>/heatmap.png      FLIP error map (magma)
```

All paths in the JSON are relative to the report directory, so the directory can be zipped and opened anywhere. Statuses: `pass`, `fail`, `new` (capture without baseline), `missing` (baseline without capture), `error` (unreadable or undecodable image, symlink, differing dimensions). Each entry carries mean, p50, p95, p99 and max of the error map, and basic image properties (all-black and all-white captures are flagged).

## Roadmap / not yet

- HDR and EXR inputs (float FLIP).
- Masks and ignore-regions inside an image.
- Hosted baselines and storage.
- Images inline in PR comments (needs an image host).
- GPU capture helpers.

## Credits

flipdiff grew out of the visual test crate of the Moss engine (`moss_visual_test`). The FLIP algorithm is by NVIDIA (Andersson et al., "FLIP: A Difference Evaluator for Alternating Images", HPG 2020), used through the `nv-flip` bindings. See `THIRD_PARTY.md`.

## Licence

`MIT OR Apache-2.0`, at your option: `LICENSE-MIT`, `LICENSE-APACHE`. Bundled NVIDIA FLIP code is BSD-3-Clause; see `THIRD_PARTY.md`.
