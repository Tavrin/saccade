# Wave 2 dependency review

Published Cargo package manifests and included license files were inspected. All additions use permissive licenses. The runtime library and model weights remain separate runtime inputs. Native gates used Rust 1.98.1; a Rust 1.88 build was not run.

| Package | Version | License | Declared Rust minimum |
| --- | --- | --- | --- |
| [aligned-vec](https://crates.io/crates/aligned-vec/0.6.4) | 0.6.4 | MIT | unspecified |
| [av-data](https://crates.io/crates/av-data/0.4.4) | 0.4.4 | MIT | unspecified |
| [byte-slice-cast](https://crates.io/crates/byte-slice-cast/1.2.3) | 1.2.3 | MIT | 1.51.0 |
| [equator](https://crates.io/crates/equator/0.4.2) | 0.4.2 | MIT | unspecified |
| [equator-macro](https://crates.io/crates/equator-macro/0.4.2) | 0.4.2 | MIT | unspecified |
| [libloading](https://crates.io/crates/libloading/0.8.9) | 0.8.9 | ISC | 1.71.0 |
| [ndarray](https://crates.io/crates/ndarray/0.16.1) | 0.16.1 | MIT OR Apache-2.0 | 1.64 |
| [ort](https://crates.io/crates/ort/2.0.0-rc.10) | 2.0.0-rc.10 | MIT OR Apache-2.0 | 1.81 |
| [ort-sys](https://crates.io/crates/ort-sys/2.0.0-rc.10) | 2.0.0-rc.10 | MIT OR Apache-2.0 | 1.81 |
| [pkg-config](https://crates.io/crates/pkg-config/0.3.34) | 0.3.34 | MIT OR Apache-2.0 | 1.63 |
| [portable-atomic](https://crates.io/crates/portable-atomic/1.15.0) | 1.15.0 | Apache-2.0 OR MIT | 1.34 |
| [portable-atomic-util](https://crates.io/crates/portable-atomic-util/0.2.8) | 0.2.8 | Apache-2.0 OR MIT | 1.36 |
| [smallvec](https://crates.io/crates/smallvec/2.0.0-alpha.10) | 2.0.0-alpha.10 | MIT OR Apache-2.0 | 1.57 |
| [ssimulacra2](https://crates.io/crates/ssimulacra2/0.5.1) | 0.5.1 | BSD-2-Clause | 1.65.0 |
| [tracing](https://crates.io/crates/tracing/0.1.44) | 0.1.44 | MIT | 1.65.0 |
| [tracing-core](https://crates.io/crates/tracing-core/0.1.36) | 0.1.36 | MIT | 1.65.0 |
| [v_frame](https://crates.io/crates/v_frame/0.3.9) | 0.3.9 | BSD-2-Clause | 1.80.0 |
| [yuvxyb](https://crates.io/crates/yuvxyb/0.4.2) | 0.4.2 | MIT | 1.64.0 |
| [yuvxyb-math](https://crates.io/crates/yuvxyb-math/0.1.0) | 0.1.0 | MIT | 1.64.0 |

The release notice generator inventories every resolved dependency, including optional and platform-only crates, and includes available license texts. SSIMULACRA2 and its v_frame dependency require BSD notices; the copies below are retained in the repository as well. RenderDoc 1.34 and ONNX Runtime 1.22 are MIT-licensed external runtime tools. Grounding DINO Tiny and SAM 2.1 Hiera Tiny checkpoint model cards declare Apache-2.0; the pinned manifest records exact revisions and hashes. No SAM custom-license model is used.

## Review-fix compression additions

The following pinned/resolved dependencies were inspected from their downloaded
published manifests and license files. None declares a minimum above Rust 1.88.
No new minimum-toolchain run is inferred from manifest review.

| Package | Version | License | Declared Rust minimum |
| --- | --- | --- | --- |
| butteraugli | 0.4.0 | BSD-3-Clause | 1.85 |
| imgref | 1.12.3 | CC0-1.0 OR Apache-2.0 | 1.63 |
| multiversion | 0.8.0 | MIT OR Apache-2.0 | unspecified |
| multiversion-macros | 0.8.0 | MIT OR Apache-2.0 | unspecified |
| rgb | 0.8.53 | MIT | 1.64 |
| simd_aligned | 0.6.1 | MIT | 1.83 |
| target-features | 0.1.6 | MIT OR Apache-2.0 | 1.61 |

Butteraugli's BSD notice is retained in THIRD_PARTY.md and the complete archive
notice generator. Later Butteraugli releases were rejected because their
archmage/magetypes dependencies require Rust 1.89, even where the metric crate
itself declares Rust 1.85. libc is now a direct CLI dependency for portable Unix
nonblocking regular-file opens; it was already present in the lockfile and uses
MIT OR Apache-2.0.

The shared CLI reader also pins serde_ignored 0.1.14 (MIT OR Apache-2.0,
Rust 1.61). Its callback detects otherwise discarded fields in existing nested
report types, without changing historical schemas. Published metadata and both
license texts were inspected; no additional transitive packages were added.
