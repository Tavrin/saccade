# Local media API

`saccade serve media/ --api --port 7878` starts the versioned API on 127.0.0.1.
Without roots it registers the current directory. Requests cannot change startup roots,
model registry or download authority. The viewer's `serve ROOT...` behavior is retained
when `--api` is absent. `--api-bind` accepts an explicitly chosen address; loopback is
the default. `--api-registry` and `--api-model-dir` supply installed model configuration.

| Endpoint | Request | Response |
| --- | --- | --- |
| GET `/v1/health` | None | `saccade-api-health.v1` |
| POST `/v1/analyze-media` | `source`, optional `options` | `saccade-media-record.v1` |
| POST `/v1/compare` | `a`, `b`, optional `ppd` | `saccade-media-compare.v1` |
| POST `/v1/search` | `index`, exactly one `image`/`text`, optional `top` | `saccade-embedding-query.v1` |

Image sources are `{ "path": "image.png" }` inside registered roots or
`{ "bytes_base64": "..." }` with encoded image bytes. Media path sources also accept
videos or pre-extracted frame directories. URLs are not accepted through HTTP/MCP;
URL fetch authority belongs to direct library/CLI callers.
Text search requires a pinned SigLIP 2 joint registry; image-only models return
`text_embedding_unavailable`. Both towers and tokenizer must match the index.
Image search uses the shared wave 6 index and exact model identity.

[OpenAPI](api-openapi.json) links the published request/result schemas. Failures use
`saccade-media-error.v1` with stable CLI/Python codes. Unsupported optional analysis
sections remain visible in a successful record unless strict.

Requests require JSON Content-Type and Content-Length (chunked bodies unsupported).
The default body limit is 16 MiB, configurable with `--api-max-bytes` up to 64 MiB;
headers are bounded to 32 KiB. Read deadline is 10 seconds total and write timeout 5 seconds.
The small local HTTP/1.1 server closes each connection and processes requests sequentially,
reusing one lazy analyzer. There is no CORS access or hosted-service/publishing behavior.

If `~/.config/saccade/api.env` exists, it must contain `SACCADE_API_TOKEN=...`; all
endpoints then require `Authorization: Bearer ...`. `--api-token-file` selects a startup
file explicitly. No ambient credential environment fallback, keys in requests or key logs.

```sh
curl -H 'Content-Type: application/json' \
  --data '{"source":{"path":"image.png"},"options":{"output_sizes":[[1200,800]]}}' \
  http://127.0.0.1:7878/v1/analyze-media
```

`Dockerfile.wave8` builds a minimal unprivileged binary image; models and optional runtime
remain in `/models`, media in `/data`. No ffmpeg is bundled; frame directories work.
The heavy gate builds and smoke-tests it; no Docker Hub publishing is configured.
For container port forwarding, declare `--api-bind 0.0.0.0` inside the container and bind
the published host port to 127.0.0.1. Supply a mounted token file when authentication is wanted.
