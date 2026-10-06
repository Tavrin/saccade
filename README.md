[![CI](https://img.shields.io/github/actions/workflow/status/Tavrin/saccade/ci.yml?branch=main&label=CI)](https://github.com/Tavrin/saccade/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/saccade.svg)](https://crates.io/crates/saccade)
[![docs.rs](https://img.shields.io/docsrs/saccade)](https://docs.rs/saccade)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![MSRV](https://img.shields.io/badge/MSRV-1.89-blue)](Cargo.toml)
[MCP Registry: io.github.Tavrin/saccade](https://registry.modelcontextprotocol.io/?q=io.github.Tavrin%2Fsaccade)

# saccade

saccade is a visual and performance evidence tool for humans, CI and AI agents.
It measures perceptual differences, locates them in measured regions (with
optional advisory observations), and checks them against the thresholds and
policies you declare. It writes offline HTML reports and versioned JSON.
Performance conclusions require comparable captures, timing provenance and
repeat evidence. Only a human can approve a baseline.

![Report with image differences and numbered hotspots](docs/images/report.png)

## Quickstart

```sh
saccade demo --out saccade-demo
saccade view saccade-demo
```

The demo exits 1 on purpose: it contains changed and missing captures.
Open `saccade-demo/report/index.html` to see the result. The
[quickstart walkthrough](docs/quickstart.md) has copyable examples for
comparison, exact identity, configuration, evidence export and local review.

## Use cases

### Visual verification for coding agents

The local MCP server reads registered capture roots and writes reports under a
separate output root. Its tools cannot approve baselines. Agents can inspect
bounded results and follow recorded next actions.

```sh
saccade mcp --root captures --out-root agent-reports
saccade compare captures/before captures/after --out agent-reports/change --json
```

See the [agent guide](docs/agents.md) and [MCP and plugin setup](docs/plugins.md).

### CI visual regression

The local Playwright package provides `toMatchSaccade` and capture stabilization.
A sweep groups page pairs and records capture failures as failures.

```sh
node integrations/playwright/sweep.cjs sweep.json captures capture-options.json
saccade sweep compare sweep.json --captures captures/captures.json --out sweep-report --json
```

See the [Playwright matcher](docs/playwright-matcher.md), [sweep](docs/sweep.md)
and [CI integration](docs/ci.md). Sweep needs the optional `products` feature.

### Render and engine evidence

Declared spatial policies distinguish `texture_noise_only` from
`systematic_shift`. Required-effect checks can fail on an empty footprint;
capture layers restrict measurement scope and fixed-camera sequences measure
temporal stability. Supply the policies and capture provenance with your inputs.

```sh
saccade compare before after --config render-evidence.toml --out render-report --json
saccade experiment sequence before-frames after-frames --fixed-camera --out temporal-report --json
```

See [rendering evidence](docs/render-evidence.md) and
[engine capture ingestion](docs/engine-ingest.md),
[temporal settling](docs/experiments-wave11.md#event-relative-visual-settling)
and [one-flag masks](docs/experiments-wave11.md#one-flag-native-scopes-and-occupancy).

### Image delivery tuning

Audit served formats and search declared encodings for a perceptual target.
Local and URL-template adapters record bytes and content types and do not modify originals.

```sh
saccade imgtune audit --urls images.txt --accept 'image/avif,image/webp,image/*' --out audit.json --json
saccade imgtune search tuning.json --out tuning-report.json --json
```

See [image tuning](docs/imgtune.md). Enable `products`, plus `imgtune-avif` for AVIF.

### Media analysis records

A media record keeps status and provenance for metadata, quality, fingerprints
and optional model sections. Disabled or failed sections are recorded as such.
The Python package and local HTTP API use the same analysis path.

```sh
saccade analyze-media image.jpg --profile cpu-lite --output-size 1200x800 --json
saccade keyframes video.mp4 --out frames --json
```

See [media analysis](docs/media.md), [Python](docs/python.md) and [HTTP API](docs/api.md).
Install with `pip install saccade-vision` (Python 3.10+); the import stays `import saccade`.

### Single-image provenance and integrity

Inspect C2PA credentials, metadata and compression-history indicators without a
baseline. Heuristics have stated limits and do not establish a real/fake verdict.
GPS disclosure is opt-in; C2PA validation needs `credentials`.

```sh
saccade inspect-image received.jpg --output-size 1600x900 --out inspection --json
saccade inspect-image received.png --hash-index hashes/saccade-hash.v1.json --out indexed-inspection --json
```

See [single-image inspection](docs/inspect-image.md).

### General comparison

Pick registration, hashes, embedding similarity/search, OCR text differences,
no-reference quality or document rasterization to fit the question.
Model workflows require supplied pinned artifacts and the relevant features.

```sh
saccade compare before.png after.png --align similarity --out aligned-report --json
saccade text before.png after.png --out text-report --json
saccade assess image.jpg --out quality-report --json
```

See [choosing a comparison](docs/choosing-a-comparison.md),
[registration](docs/registration.md), [hashing](docs/hashing.md),
[embeddings and search](docs/embeddings.md), [OCR](docs/text.md),
[quality](docs/assessment.md) and [documents](docs/documents.md).

### Performance evidence

Compare supplied timing sidecars using paired statistics and uncertainty
intervals, or locate changes in a sequence. Saccade does not run the benchmark;
missing or rejected provenance cannot qualify a speedup.

```sh
saccade compare before after --out perf-report --json
saccade history onset --store history --json
```

See [paired statistics](docs/paired-performance.md),
[change points](docs/wave1.md), [identity/performance](docs/identity-and-performance.md),
[timing verdicts](docs/experiments-wave11.md#timing-ab),
[ablation tables](docs/experiments-wave11.md#multi-arm-repeat-tables)
and [report linking](docs/experiments-wave11.md#external-report-links-and-indexes).
The [performance sidecar kit](docs/perf-kit.md) has fixtures and recipes for producers.
Label images can be scored with [mask metrics](docs/mask-metrics.md), boxes exchanged
as COCO or YOLO ([box interchange](docs/box-interchange.md)), and extracted video
frames described by a [frame map](docs/frame-map.md).

## Install

Install with Rust 1.89 or newer:

```sh
cargo install saccade --version 0.2.6 --locked
saccade doctor --json
```

To build this checkout, use `cargo install --locked --path crates/saccade`.
[Release archives](https://github.com/Tavrin/saccade/releases) include checksums,
licences and third-party notices. See [release instructions](docs/releasing.md).

| Cargo features | Default? | Purpose and requirements |
| --- | --- | --- |
| `compression`, `parallel`, `graphics` | Yes | FLIP, compression scores, graphics and temporal measurements; CPU processing. |
| `ai`, `evaluation` | Yes | Provider review adapters and evaluation; calls require explicit authorization and budgets. |
| `workbench`, `mcp` | Yes | Local report browsing and bounded agent tools. |
| `assist` | No | Experimental explain, mask audit, visible-condition checks and batch advice. |
| `products` | No | Sweep, image tuning, design-source and notifier adapters. Browser captures separately need Node.js and Playwright. |
| `imgtune-avif` | No | AVIF encoding/decoding; system dav1d >=1.3.0 development library and pkg-config. |
| `semantic-regions`, `embeddings`, `local-models` | No | Pinned CPU model artifacts and dynamically loaded ONNX Runtime 1.22 (API 22). Runtime/model pulls are explicit provisioning operations; analysis never downloads them. |
| `ocr`, `ocr-provider` | No | Local PP-OCRv5 Latin with pinned models/runtime; separately opt-in hosted document OCR with spend/egress controls. |
| `documents`, `credentials` | No | SVG/PDF rasterization and offline C2PA validation. |
| `media-http` | No | Bounded URL inputs for media analysis; network fetches require an explicit URL input. |
| `local-vlm`, `vision-providers` | No | Configured local/hosted vision adapters; observations remain advice. |
| `geometry`, `dense-motion`, `prechecks`, `schema` | No | Mesh measurements, dense motion, experimental safety/accessibility checks and schema generation. |

Video extraction invokes external `ffmpeg` and `ffprobe`; neither is bundled.
See [model/runtime provisioning](docs/wave7.md) and [AVIF prerequisites](docs/imgtune.md#system-prerequisites).
Default comparisons need no provider account or GPU. Add optional features with
`cargo install ... --features products,ocr`, for example.

## Metrics and algorithms

Each qualification covers only the recorded contract and evidence. A passing
score does not prove correctness or that a difference is invisible.

| Metric or algorithm | Qualification status and scope |
| --- | --- |
| Native decoded-sample identity | Exact equality contract; covers only supplied, complete pairs. |
| NVIDIA FLIP / HDR-FLIP | Reference-backed implementation; viewing conditions and HDR mapping must be declared. |
| SSIMULACRA2 / Butteraugli 0.9.3 | Reference checks recorded; perceptual targets are user policy. See [implementation evidence](docs/design-decisions/backlog-2026-10.md). |
| ColorVideoVDP, numerical buffers, motion and mesh distances | Declared display/unit/frame/camera contracts; fixture checks do not qualify a consumer renderer. |
| Registration; aHash/dHash/pHash; FAST/oriented-BRIEF matching | Generated-fixture validation; match confidence is uncalibrated. |
| Embedding similarity and image/text retrieval | Pinned model/export provenance; retrieval calibration remains unqualified. |
| PP-OCRv5 Latin and positional text diff | Generated strict and typographic contracts reviewed; [font-specific failures remain](docs/ocr-contract-results-2026-10-06.md). |
| Blur, noise, blockiness, banding, clipping and forensic indicators | Descriptive, content-dependent heuristics; no authenticity qualification. |
| Spatial classes, effect occupancy, layers and temporal tiles | Constructed/fixture contracts; no general renderer or physical-effect qualification. |
| Paired Hodges–Lehmann estimates, bootstrap and change points | Constructed statistical checks; actual timing requires independent qualified runs. |
| Safety/accessibility prechecks | Experimental checks; no certification or formal compliance. |
| AI assist, Jev support and routing ([Jev](https://typesafe.ai) is a decision model from TypeSafe), blind-order handling | Unqualified in this release; experimental. |
| LPIPS, DISTS, MUSIQ; TrustMark payload decoding | Deferred / unavailable; neural-only TrustMark inference does not decode payloads. |

## AI layer

AI observations are advisory. They cannot approve baselines, create exclusions,
override deterministic failures, establish equality or qualify timing. Providers
are opt-in, with source-root egress authorization, dedicated credential files and
call/spend caps; image text and provider output are data, never instructions.

The release qualification attempt with a $15 cap stopped at the prerequisite
check, before any provider request or spend. No frozen corpus with an observed
immutable Gemini revision or exact-source heavy receipt was supplied. Explain,
mask audit, visible-condition checks, blind orders, Jev support, deterministic
cascade routing and optional Jev evidence routing all remain **unqualified**.
Assist commands still require `--experimental`; Jev routing stays off by default.
See [assist workflows](docs/assist.md), [qualification policy](docs/assist-qualification.md)
and the [release guide](docs/releasing.md).

## Known limitations

- OCR can misread œ in large serif text and omit dashes with some sans fonts;
  omitted characters and missing spaces before € remain errors.
- Retrieval scores lack qualified calibration. Model/runtime smokes establish
  execution, not production accuracy or export parity.
- GI occupancy uses supplied masks/layers as a proxy; it does not prove physical
  illumination, causality or correct rendering.
- LPIPS, DISTS and MUSIQ remain deferred; TrustMark payload decoding is unavailable.
- Captures define the evidence scope. Equal images do not establish application
  correctness, and perceptual passes do not establish native sample identity.
- Performance conclusions need matched workload, clocks, warmup and repeat noise.
  Missing checks remain unknown; human approval records do not authenticate an operator.

## Documentation and integrations

- [Documentation](docs/choosing-a-comparison.md), [CLI reference](docs/cli.md)
  and [JSON contracts/schemas](docs/contracts.md).
- [MCP server and plugins](docs/plugins.md), [agent guide](docs/agents.md),
  [Python package](docs/python.md) (PyPI: `saccade-vision`),
  [HTTP API](docs/api.md) and [CI](docs/ci.md).
- [CHANGELOG](CHANGELOG.md), [release guide](docs/releasing.md)
  and [contributing](CONTRIBUTING.md).

<!-- showcase-count:start -->
[9 reproducible cases](showcases/README.md) with commands, expected exits and measured output.
[Pages gallery](https://tavrin.github.io/saccade/showcase/).
<!-- showcase-count:end -->

## License

[MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE), at your option.
FLIP uses the BSD-3-Clause `flip-rs` port. See [third-party notices](THIRD_PARTY.md);
release archives also include generated dependency notices.

MCP Registry ownership: `mcp-name: io.github.Tavrin/saccade`

Saccade originated in the Moss engine’s visual testing tools.
