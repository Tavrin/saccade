# Local compatibility patch

Source: crates.io butteraugli 0.4.0, BSD-3-Clause. Only compiler-version target
selection, build metadata and omitted external example/test declarations differ.
Rust >=1.89 retains the upstream AVX-512/AVX2/SSE target list. Rust 1.88 and
unrecognized compilers use AVX2/SSE. Runtime CPU detection remains multiversion's.
A registry package does not inherit this workspace patch; upstream publication
of the fix is required before claiming packaged MSRV qualification.
