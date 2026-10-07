# Saccade for Python

Local image analysis and comparison with explicit evidence provenance, powered by Rust.
Supports Python 3.10 and newer on Linux, macOS and Windows.

```sh
pip install saccade-vision
```

The PyPI distribution name is **saccade-vision**; the Python import name is **saccade**.
Analyze an existing image without downloading models:

```python
import saccade
analyzer = saccade.Analyzer(profile="cpu-lite", allow_download=False)
record = analyzer.analyze_media("image.png")
print(record["identity"])
print(analyzer.hash("image.png"))
```

The standard wheels provide CPU-only analysis; model operations require a custom
model-enabled build and explicitly provisioned models.
See the [Python documentation](https://github.com/Tavrin/saccade/blob/main/docs/python.md)
for the API, errors and optional model support.

`batch(source, out, options_json=None, reference_dir=None)` returns resumable
per-input rows in-process through the same Rust core runner as the CLI. Installing
only the wheel is sufficient; no executable or `SACCADE_BIN` is needed. Deadlines
are cooperative: a running bounded operation finishes before a timed-out row returns.
Unsupported section flags produce explicit partial rows. See
[batch intake](../../docs/batch-and-assist.md) for resource limits and status semantics.
