# Third-party notices

saccade itself is licensed `MIT OR Apache-2.0` (see `LICENSE-MIT`, `LICENSE-APACHE`).

## flip-rs / NVIDIA FLIP (BSD-3-Clause)

FLIP and HDR-FLIP are implemented by the pure-Rust
[flip-rs](https://github.com/Tavrin/flip-rs) port of NVIDIA FLIP v1.7, pinned to
revision `5f4d5c29a0dc40fed2bda8e1d6f6ccc1772d412e`. No FLIP C++ code or FFI
wrapper is built or bundled. The display-only HDR tone-mapping coefficients
in `crates/saccade-core/src/hdr.rs` are also derived from NVIDIA's reference.

The following notice is reproduced from flip-rs's `LICENSE`:

```text
BSD 3-Clause License

Copyright (c) 2020-2025, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
Copyright (c) 2026 flip-rs contributors

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
this list of conditions and the following disclaimer in the documentation
and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

SPDX-FileCopyrightText: Copyright (c) 2020-2025 NVIDIA CORPORATION & AFFILIATES
SPDX-License-Identifier: BSD-3-Clause
```

Reference: Andersson et al., "FLIP: A Difference Evaluator for Alternating
Images", High Performance Graphics 2020, and "Visualizing Errors in Rendered
High Dynamic Range Images", Eurographics 2021 Short Papers.

## HDR image decoding

The `exr` and `hdr` features of the `image` crate add `exr` 1.x (BSD-3-Clause),
`lebe` (BSD-3-Clause), `half` (MIT OR Apache-2.0), `bit_field`
(Apache-2.0/MIT), `crunchy` (MIT) and `zune-inflate` (MIT OR Apache-2.0 OR
Zlib), all compatible with saccade's `MIT OR Apache-2.0` licence.

## Other dependencies

Rust dependencies (`image`, `serde`, `serde_json`, `thiserror`, `clap`, `rayon`
(MIT OR Apache-2.0), `sha2` (MIT OR Apache-2.0), and their transitive crates) are under MIT, Apache-2.0, BSD, Zlib or ISC terms.
Run `cargo tree` for the full list.
