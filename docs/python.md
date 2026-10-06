# Python

Install with `pip install saccade-vision`. The PyPI distribution is named
`saccade-vision`; the import remains `import saccade`. Python 3.10+ is supported
through abi3 wheels for Linux x86_64/aarch64, macOS arm64 and Windows x86_64.
Standard wheels use the CPU-only feature set and never download models implicitly.
An sdist is also available; building it requires the workspace's supported Rust toolchain.

CI runs generated-image tests against installed wheels on every target and retains
the artifacts. Only `v*` tag pushes can publish through the protected `pypi`
environment; see [releasing](releasing.md). Contributors can also run the targeted
`python_package` Cargo test with feature `python-tests`; that check does not qualify
a release wheel archive.

PyO3's `extension-module` feature is enabled only in `[tool.maturin]` in
`crates/saccade-py/pyproject.toml`, following the
[PyO3 0.25.1 FAQ](https://pyo3.rs/v0.25.1/faq.html).
Ordinary Cargo builds and workspace tests must link Python on macOS and Linux;
enabling this feature unconditionally suppresses that linkage and caused the
macOS test linker failure in CI run 37385348419. Maturin enables it for wheels,
including the existing `python-wheels.yml` manifest-path invocation, while
`abi3-py310` remains enabled for both build paths. A `build.rs` extension-linker
workaround is unnecessary: this crate needs separate Cargo test and extension
build modes, and maturin already supplies the extension linker arguments.

```python
import saccade
analyzer = saccade.Analyzer(profile="cpu-lite", allow_download=False)
record = analyzer.analyze_media("image.png", output_sizes=[[1200, 800]])
hashes = analyzer.hash("image.png")
comparison = analyzer.compare("image.png", "variant.png")
```

One Analyzer supports concurrent calls, releases the GIL during compute and loads model
sessions lazily once. Model operations require a wheel built with `models`, a supplied
registry with reviewed embedding/OCR contracts and pinned cache files. `pull_runtime()`
and `pull_models(["yunet-2026may"])` explicitly provision wave 7 artifacts; inference
uses `allow_download=False` by default. The default cache follows `XDG_CACHE_HOME`
or `HOME/.cache`, under `saccade/models`; callers can set `model_dir`. Models are
external to wheels.
`ModelError`, `InputError` and `AnalysisError` subclass `SaccadeError`; `.code` carries
the same stable code as the CLI. Per-section failures stay in the record unless strict.
`embed_image`, `Index.build`, `add`, `query(image=...)`, `save` and `load` share the
versioned wave 6 index. `embed_text` and text queries require an installed pinned SigLIP 2 joint registry.
Image-only models raise `text_embedding_unavailable`. Similarity bands are uncalibrated.

A generic media analysis worker for any web application:

```python
from concurrent.futures import ThreadPoolExecutor
import asyncio
from fastapi import FastAPI, UploadFile
import saccade

app = FastAPI()
worker = ThreadPoolExecutor(max_workers=2)
analyzer = saccade.Analyzer(profile="cpu-lite")

@app.post("/media/analyze")
async def analyze(file: UploadFile):
    payload = await file.read(64 * 1024 * 1024 + 1)
    if len(payload) > 64 * 1024 * 1024:
        from fastapi import HTTPException
        raise HTTPException(413, "media input too large")
    return await asyncio.get_running_loop().run_in_executor(
        worker, analyzer.analyze_media, payload
    )
```

The worker retains bytes and returns a complete record to the application's storage layer;
application-specific rights decisions, queue persistence and authentication stay with the app.
The package ships `.pyi` stubs and `py.typed`.

`Index.query` returns `saccade-media-index-query.v1` with compact `hits`;
standalone CLI query evidence retains `saccade-embedding-query.v1`.

## Version and native maps

`saccade.__version__` matches the distribution version. NumPy is a declared
runtime dependency for the array API; no external FLIP package is needed.

```python
import saccade
maps = saccade.compare_maps("reference.png", "candidate.png", tile_size=32)
print(saccade.__version__, maps["flip"].shape)
print(maps["tile_signed_shift"])
```

Maps are independent float32 NumPy arrays. FLIP is `[height,width]`; tile statistics
have the actual tile-grid shape. This SDR pair API accepts paths or encoded bytes,
imports NumPy before input work, and releases the GIL during native comparison.
`compare --export-maps` provides the same maps as `.npy`/float32 `.exr` with a
versioned JSON index. See [rendering evidence](render-evidence.md) for units and limits.
