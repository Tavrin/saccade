# Agent trial fixes

- **Provenance:** Accept sidecar `binary.sha` and `build.commit` beside existing keys. Missing evidence names `--meta-name` and the needed fields. Reject guessing source identity from image bytes: pixels cannot establish a build.
- **Verdicts:** Preserve the documented image exit code. Keep `verdict` for the gate, but set it to `performance_rejected` when image thresholds pass and performance comparability is rejected; add `performance` and `overall`. Reject an image-only `pass` because it misstates the run-wide claim.
- **Inspection:** Return deciding fields and three hotspots, plus a paginated `--validity-reasons` view. Reject dumping full reports into JSON because the agent result remains bounded. `failing` uses value/threshold ratio; entries without measured values follow measured failures.
- **Actions:** Attach the absolute invocation `cwd` to relative `cli_argv` paths. Reject making every report path absolute; other output remains portable.
- **Review preview:** Write `requests.json` and `preview.json` when `--out DIR` is supplied. Display source roots relative to the invoking directory unless absolute recording is requested. Estimate input tokens from payload bytes and reserve 512 output tokens per question; compute USD only from user-configured model rates. Reject a default price because it can become stale. No provider is contacted for estimation.
- **Report assets:** Keep the copied image evidence and refer to it from the viewer payload. Reject duplicate base64 embeds, which made the trial report roughly twice as large.
