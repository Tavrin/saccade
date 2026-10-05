# Compression reference qualification

The four RGB8 64x64 PNGs are synthetic project-authored fixtures, distributed
under this project's MIT OR Apache-2.0 license. They contain an opaque sRGB
ramp, a +3 brightness shift, +/-3 red-channel 8x8 blocks, and a 16x16 changed
patch. There is one identity pair and three nonidentical pairs. No private,
third-party or model-generated images are included.

`values.json` retains encoded-image hashes, reference source-file hashes,
reference executable hashes and the actual reference outputs. Reference values
were run locally, not derived from the Rust implementations:

- [libjxl v0.12.0](https://github.com/libjxl/libjxl/tree/v0.12.0): official
  `tools/ssimulacra2` and `tools/butteraugli_main`, Release build, GCC 13.3,
  upstream-pinned Highway 1.2.0 and skcms; system PNG/JPEG/Brotli libraries.
- [Cloudinary SSIMULACRA2 v2.1](https://github.com/cloudinary/ssimulacra2/tree/v2.1):
  the unchanged standalone source and bundled libjxl support, Release build,
  GCC 13.3, Highway 1.0.7 and Little CMS 2.14. Its results exactly matched
  the libjxl SSIMULACRA2 tool on all four pairs.

Run each executable as `TOOL reference.png distorted.png`. Butteraugli uses
its default intensity target of **80 cd/m2**; retain its first-line global
maximum distance, not its separately printed 3-norm. Inputs are opaque RGB8
sRGB/BT.709. No profiles, alpha, HDR, resizing or JPEG decoder differences
are involved.

The fixed absolute tolerances are **0.05 SSIMULACRA2 points** and **0.02
Butteraugli distance units**. They bound the observed differences between the
pinned Rust ports and independently built C++ implementations without claiming
bit parity. On these fixtures the maximum SSIMULACRA2 discrepancy is 0.026834
points and the maximum Butteraugli discrepancy is 0.010102 (the patch distance
is about 54.25). These tolerances do not qualify arbitrary images, display
conditions, invisibility, or human perception. They are deliberately much
smaller than the differences between the nonidentical reference scores and a
constant identity scorer. Changing tolerances or fixtures requires fresh
reference evidence and review.

`compression_metrics_match_official_nonidentical_references` checks fixture
hashes, both official SSIMULACRA2 references, and the official Butteraugli
reference. It also exercises incremental/cumulative sweep evidence. Sweep
selection remains governed by the declared SSIMULACRA2 and byte budgets;
Butteraugli is supplementary evidence, not an undeclared extra policy gate.

To reproduce reference collection, build libjxl with `BUILD_TESTING=OFF`,
`JPEGXL_ENABLE_DEVTOOLS=ON`, and build targets `ssimulacra2 butteraugli_main`.
Build Cloudinary using its documented standalone CMake workflow. Then run
`scripts/qualify-compression.py` with explicit tool and source directory paths.
