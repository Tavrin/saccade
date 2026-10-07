# saccade-core

`saccade-core` provides image comparison and the report model used by the
[saccade CLI](https://crates.io/crates/saccade). It can be used without invoking
a subprocess or writing a report directory. Rust 1.89 or newer is required.

## Image comparison

`compare::compare` measures two same-sized RGB images with NVIDIA FLIP through
`flip-rs`. The result contains mean, p95, p99 and maximum error, along with a
per-pixel error map. Viewing conditions are explicit; the default is 67 pixels
per degree. `compare_rgba` handles alpha, and `hdr` provides HDR comparison.
A score below a chosen threshold does not prove that the images are identical.

Add these dependencies to an application:

```toml
[dependencies]
saccade-core = "0.2.9"
image = { version = "0.25", default-features = false }
```

This example compares an in-memory image with itself and checks that it has
no error hotspots. The README is included in the crate documentation, so
`cargo test -p saccade-core --doc` compiles and runs this code.

```rust
use image::{Rgb, RgbImage};
use saccade_core::compare::{compare, CompareOptions};
use saccade_core::hotspots::{find_hotspots, HotspotOptions};

let baseline = RgbImage::from_pixel(32, 32, Rgb([80, 120, 160]));
let capture = baseline.clone();
let result = compare(&capture, &baseline, &CompareOptions::default())?;
assert_eq!(result.metrics.mean, 0.0);
assert_eq!(result.error_map.len(), 32 * 32);
let hotspots = find_hotspots(
    &result.error_map, None, 32, 32, &HotspotOptions::default(),
);
assert!(hotspots.is_empty());
# Ok::<(), saccade_core::Error>(())
```

`hotspots::find_hotspots` groups connected pixels above a threshold, merges
nearby components and ranks them by summed error. Its options control the
number of returned regions and the minimum error share. A mask can exclude
pixels from the analysis.

## Identity and performance

`compare::native_samples_identical` checks decoded files at their native bit
depth, including dimensions, channel representation and alpha. Encoded file
bytes may differ while decoded samples match. Decode failures return false.
The `run` module handles directory pairing and report generation; a complete
identity claim also requires every selected pair to exist and decode.

With `graphics` enabled, `perf` evaluates timing evidence and produces
performance verdicts. It checks measurement comparability, repeat noise and
attribution before accepting a performance claim. Missing noise is treated
as unknown rather than zero. Equal images alone do not establish a speedup. The
[identity and performance guide](https://github.com/Tavrin/saccade/blob/main/docs/identity-and-performance.md)
describes the evidence requirements.

## Reports and schemas

`Report`, `Entry`, `Metrics` and the other report types support Serde
serialization. Persisted report and decision types remain available when
the optional computations are disabled. `render` produces the HTML report;
`evidence` holds typed artifacts and their schema accessors.

The crate packages JSON Schemas in `schemas/`. Enable `schema` to derive
`schemars::JsonSchema` for report, decision and explain types. See the
[contract index](https://github.com/Tavrin/saccade/blob/main/docs/contracts.md)
for artifact versions and compatibility rules. The library version and an
artifact's schema version are separate.

## Cargo features

Only `parallel` is enabled by default. Use `default-features = false` for
the sequential FLIP backend. The other features are opt-in:

| Feature | Effect |
| --- | --- |
| `parallel` | Enables parallel filtering in `flip-rs`. |
| `graphics` | Enables graphics computations, including performance evaluation. |
| `compression` | Adds pinned SSIMULACRA2 scoring for explicit encoded candidate sweeps. |
| `semantic-regions` | Adds explicit model caching and dynamically loaded ONNX runtime plumbing; text-to-mask inference remains unavailable. |
| `ai` | Adds provider adapters, review orchestration and HTTP dependencies. |
| `workbench` | Adds the local HTTP workbench independently of AI. |
| `evaluation` | Adds replay, calibration and provider conformance; enables `ai`. |
| `prechecks` | Enables experimental safety and accessibility checks. |
| `schema` | Enables JSON Schema derives through `schemars`. |

The `saccade` crate supplies command-line parsing, MCP transport and command
workflows over this library. Its default feature set is broader. Applications
that only need comparisons can depend on `saccade-core` directly. Neither
basic image comparison nor report rendering needs a GPU or provider account.

`models/semantic-regions.json` packages checkpoint hashes, licenses and qualification
limits. Model weights are never packaged; explicit runtime caching verifies downloads.
Frozen mask import and localized measurement do not require the model feature.

## License

Licensed under either [MIT](https://github.com/Tavrin/saccade/blob/main/LICENSE-MIT)
or [Apache-2.0](https://github.com/Tavrin/saccade/blob/main/LICENSE-APACHE), at your
option. The `flip-rs` dependency is BSD-3-Clause; see the repository's
[third-party notices](https://github.com/Tavrin/saccade/blob/main/THIRD_PARTY.md).

Command-level artifact writing and explicit approval policies are available through
`workflows`. See the [Rust library guide](../../docs/library.md) for compiled examples
and a complete CLI-to-library audit.
