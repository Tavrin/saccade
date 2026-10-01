# Third-party notices

flipdiff itself is licensed `MIT OR Apache-2.0` (see `LICENSE-MIT`, `LICENSE-APACHE`).

## NVIDIA FLIP (BSD-3-Clause)

The FLIP algorithm implementation is NVIDIA's C++ code, compiled into flipdiff
through the `nv-flip-sys` crate (version 0.1.1, vendored under `extern/cpp`).
The `nv-flip` and `nv-flip-sys` Rust wrappers are licensed
`(MIT OR Apache-2.0 OR Zlib)`; the bundled NVIDIA code is `BSD-3-Clause`.

The notice below is a verbatim quotation from the header of
`nv-flip-sys-0.1.1/extern/cpp/CPP/image.h`:

> Copyright (c) 2020-2022, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
>
> Redistribution and use in source and binary forms, with or without
> modification, are permitted provided that the following conditions are met:
>
> 1. Redistributions of source code must retain the above copyright notice, this
> list of conditions and the following disclaimer.
>
> 2. Redistributions in binary form must reproduce the above copyright notice,
> this list of conditions and the following disclaimer in the documentation
> and/or other materials provided with the distribution.
>
> 3. Neither the name of the copyright holder nor the names of its
> contributors may be used to endorse or promote products derived from
> this software without specific prior written permission.
>
> THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
> AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
> IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
> DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
> FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
> DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
> SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
> CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
> OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
> OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

Reference: Andersson et al., "FLIP: A Difference Evaluator for Alternating
Images", High Performance Graphics 2020.

### HDR-FLIP exposure procedure (derived work)

`crates/flipdiff-core/src/hdr.rs` re-implements, in Rust, the logic of NVIDIA's
HDR-FLIP exposure-range selection from `CPP/image.h` (`computeExposures`),
the tone-mapping coefficient table in `CPP/tensor.h` and `solveSecondDegree`
in `common/sharedflip.h`, all under the BSD-3-Clause notice above (copyright
2020-2022 NVIDIA CORPORATION & AFFILIATES). No source text was copied; the
algorithm and the published curve coefficients were ported. NVIDIA's name is
not used to endorse flipdiff. Per-exposure images are quantised to 8 bits in
flipdiff, whereas the reference stays in floating point.

## HDR image decoding

The `exr` and `hdr` features of the `image` crate add `exr` 1.x (BSD-3-Clause),
`lebe` (BSD-3-Clause), `half` (MIT OR Apache-2.0), `bit_field`
(Apache-2.0/MIT), `crunchy` (MIT) and `zune-inflate` (MIT OR Apache-2.0 OR
Zlib), all compatible with flipdiff's `MIT OR Apache-2.0` licence.

## Other dependencies

Rust dependencies (`image`, `serde`, `serde_json`, `thiserror`, `clap`, `rayon`
(MIT OR Apache-2.0), `sha2` (MIT OR Apache-2.0), and their transitive crates) are under MIT, Apache-2.0, BSD, Zlib or ISC terms.
Run `cargo tree` for the full list.
