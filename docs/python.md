# Python

The `saccade` package supports Python 3.10+ through PyO3 abi3 wheels. Install a locally
built wheel: `python -m pip install path/to/saccade-*.whl`. Release wheel construction
and aarch64 qualification belong to `scripts/gates-wave8.sh`. CI uploads wheels as
artifacts only. The development lane uses the targeted `python_package` Cargo test
with feature `python-tests` to import the compiled abi3 extension and run light pytest;
that check does not qualify a release wheel archive.

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
