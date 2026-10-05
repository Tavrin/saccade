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
