# Reproducible showcases

9 cases discovered from `*/commands.json`.

Generate with `python3 scripts/gen-showcases.py` and `python3 scripts/gen-photosensitivity.py`.
Python 3, Pillow and numpy are required. Images are procedural; no imagery is downloaded.

Validation requires `cargo build --release -p saccade --features prechecks`, with the binary on PATH.
`scripts/run-showcases.sh` checks exits and reproduces each measured `EXPECTED.txt` byte for byte.
Set `SACCADE_SHOWCASE_REPORTS` to choose the output directory. The default is `target/showcase-reports`.
Changed expectations fail; the runner never accepts them automatically.

| Case | Commands | Expected exits | Asset scope |
| --- | --- | --- | --- |
| [cover-art](cover-art/README.md) | `compare` | 1 | Procedural |
| [lod-transition](lod-transition/README.md) | `sequence` | 1 | Procedural |
| [ml-image-model](ml-image-model/README.md) | `compare`, `explain`, `export` | 1, 0, 0 | Procedural |
| [perf-identity](perf-identity/README.md) | `identity` | 1 | Procedural; illustrative timings |
| [photosensitivity](photosensitivity/README.md) | `safety` | 1 | Procedural; experimental static previews |
| [render-gbuffer](render-gbuffer/README.md) | `compare` | 1 | Procedural |
| [texture-compression](texture-compression/README.md) | `rank` | 0 | Procedural |
| [upscaler](upscaler/README.md) | `rank`, `sequence` | 0, 1 | Procedural |
| [webapp-ui](webapp-ui/README.md) | `compare` | 1 | Procedural |

Bytes are stable for fixed Pillow/numpy versions; seeds and simulated timestamps are fixed.
The runner does not qualify native platform installation, model quality or renderer timing.
