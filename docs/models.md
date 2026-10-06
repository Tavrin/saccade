# Models and runtime configuration

Saccade never bundles model files or an ONNX runtime. You provision them once, and
every surface (CLI, MCP server, Python, Rust library) then finds the same pinned
artifacts from one configuration.

## The setting

| Setting | Environment variable | `[models]` key | Default |
|---|---|---|---|
| Model cache | `SACCADE_MODELS_DIR` (`SACCADE_MODEL_CACHE` is read as an alias) | `dir` | `$XDG_CACHE_HOME/saccade/models`, else `~/.cache/saccade/models` |
| Pinned registry | `SACCADE_MODELS_REGISTRY` | `registry` | `~/.config/saccade/models.json` if present, else the built-in pins |
| ONNX runtime library | `SACCADE_MODELS_RUNTIME_LIBRARY` | `runtime_library` | `ORT_DYLIB_PATH`, else the runtime provisioned in the cache |
| Embedding contract | `SACCADE_MODELS_EMBEDDING_CONTRACT` | `embedding_contract` | none |

The config file is `$SACCADE_CONFIG`, else `$XDG_CONFIG_HOME/saccade/config.toml`, else
`~/.config/saccade/config.toml`. Relative paths in it are relative to the file.

```toml
[models]
dir = "/srv/saccade-models"
registry = "/srv/saccade-models/registry.json"
```

Precedence: deprecated per-command flag, then environment, then the config file, then
the default. Unknown `[models]` keys and wrongly typed values are errors, not ignored.
`saccade models config --json` (Python: `saccade.model_config()`) prints each resolved
value with its source.

## Provisioning: one verb

```sh
saccade models pull runtime               # ONNX Runtime, verified against its pin
saccade models pull yunet-2026may         # any model id in the registry
saccade models pull ocr                   # the pinned PP-OCRv5 contract (or --contract FILE)
saccade models pull embedding --contract export.json   # or the configured embedding contract
saccade models list --json                # what is present, missing or corrupt
```

`models pull` is the only thing that downloads, and only the artifacts you name, each
checked against a pinned SHA-256. Python's equivalent is `saccade.pull_models([...])`
and `saccade.pull_runtime()`. Commands that run inference (`locate`, `faces`, `text`,
`similar`, `analyze-media`, ...) never download.

## MCP authority

An MCP request cannot choose a model path, registry, runtime library or cache, and
cannot trigger a download. The server reads the operator configuration of its own
process. A request may restate the configured location (`models_list` accepts `cache` and
`registry` for older clients) but any other value is refused with
`model_location_not_request_controlled`. `models_pull` is always refused. Native
runtime loading additionally needs the operator-owned `onnx-runtime.json` described
in [the embeddings guide](embeddings.md).

## Deprecated spellings

These keep working through 0.3.x, print one notice on stderr per process (Python:
`DeprecationWarning`), and will not be removed before 0.4.0.

| Old | Use instead |
|---|---|
| `--cache`, `--model-cache`, `--model-dir`, `--api-model-dir`, Python `model_dir=` | `SACCADE_MODELS_DIR` / `[models].dir` |
| `--registry`, `--model-registry`, Python `registry=` | `SACCADE_MODELS_REGISTRY` / `[models].registry` |
| `--runtime-library`, `--library` | `SACCADE_MODELS_RUNTIME_LIBRARY` / `[models].runtime_library` |
| embedding `--model FILE` | `SACCADE_MODELS_EMBEDDING_CONTRACT` / `[models].embedding_contract` |
| `--ocr-contract REGISTRY` | a configured registry carrying one OCR contract |
| `--download-model`, `--allow-download`, Python `allow_download=` | `saccade models pull <id>` |

An explicit flag still wins over the configuration, so existing scripts behave as before.
