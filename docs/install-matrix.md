# Install matrix

Pick the smallest bundle that covers your task. All bundles are the same `saccade`
binary built with different Cargo features; none contains model files or an ONNX
runtime (see [models](models.md)).

| You want to | Bundle | Install |
|---|---|---|
| Compare captures, prove identity or performance, review, run the MCP server | `default` | download `saccade-<target>` (legacy name) or `saccade-default-<target>` |
| Also read C2PA credentials, compare PDF/SVG, use imgtune/product tools, print measurements or Rust OCR | `media` | download `saccade-media-<target>` |
| Also use embeddings, geometry, geo TIFF measurements, dense motion, local models and providers, assist, media HTTP | `full` | download `saccade-full-<target>` |
| Use AVIF in imgtune | build from source | `cargo install saccade --features imgtune-avif` (needs libdav1d and pkg-config) |
| Use it from Python | wheel | `pip install saccade-vision` (see [python](python.md)); wheels carry no models |
| Use it from Rust | crate | `cargo install saccade` (default features) or add `--features ...` |

Targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
`aarch64-apple-darwin` (`.tar.gz`) and `x86_64-pc-windows-msvc` (`.zip`).

## What each bundle can do

| Capability | default | media | full |
|---|:-:|:-:|:-:|
| `compare`, `prove`, `perf`, `arms`, `review`, `mcp`, workbench | yes | yes | yes |
| Prechecks and text-quality pixel evidence | yes | yes | yes |
| ICC-managed CMYK print measurements | no | yes | yes |
| Geo TIFF measurements and grid checks | no | no | yes |
| C2PA credentials (`inspect-image`) | no | yes | yes |
| PDF and SVG documents | no | yes | yes |
| imgtune and product tools (AVIF excluded) | no | yes | yes |
| Rust OCR (`text`, needs `models pull runtime` and `models pull ocr`) | no | yes | yes |
| Embeddings, `similar`, `index` (needs your pinned export) | no | no | yes |
| Geometry, dense motion, semantic regions | no | no | yes |
| Local vision models (enabled by OCR; needs provisioned models/runtime) | no | yes | yes |
| VLM and provider adapters, `assist`, media HTTP | no | no | yes |

A command not compiled into your bundle fails with `feature_unavailable` and names
what to install; it never falls back to a different measurement.

## Verify a download

Each release lists, per asset, `<asset>.inventory.json` (features and compiled-feature
list), `<asset>.smoke.json` (the smoke test result for that exact binary) and one
`SHA256SUMS-bundles`:

```sh
sha256sum -c SHA256SUMS-bundles --ignore-missing
saccade doctor --json | python3 -c "import json,sys; print(json.load(sys.stdin)['features'])"
```

## Platform and licence notes

- Models and the ONNX runtime are downloaded at your request, from pinned sources,
  into your cache; their licences are recorded in the registry and in
  `THIRD_PARTY_NOTICES.md` inside each archive covers the compiled-in dependencies.
- Native runtimes are loaded dynamically and CPU-only; a bundle that contains `ocr` or
  `embeddings` still needs `saccade models pull runtime` (or `ORT_DYLIB_PATH`).
- C2PA, PDF and OCR code adds dependencies and binary size; that is why they are not in `default`.
