# Proposal: metadata sidecars and refusing mismatched comparisons

Status: proposed (2026-10-01), not built. Waiting on a prioritisation decision.

## Problem

Two captures can differ because the configuration differed, not because the
code did. One example is a capture preset that silently injects a renderer
mode the caller never set. An image comparator that ignores configuration
then reports a "regression" or "identity" verdict about the wrong question.

## Contract (engine-neutral)

- **Directory sidecar:** `<image_dir>/cost-card.json`. The file name is
  configurable with `--meta-name`, and this is its default. It applies to
  every image in that directory.
- **Per-image sidecar:** `<image_stem>.cost-card.json` overrides it for one
  image. Keys merge, and the per-image value wins.
- **Format:** a flat JSON object. Values are strings, numbers or booleans,
  and keys are dot-namespaced. For example: `receiver.mode`,
  `env.SOME_VAR`, `env_source.SOME_VAR` = `caller|preset|default`,
  `resolution.internal`, `binary.sha`, `gpu.adapter`, `warmup_frames`.
- **Report:** each entry gains `meta_diff`, a list of
  `{key, baseline, capture}` for differing keys, plus the run-level union.
  The HTML report shows them, and the Markdown summary adds a "config
  differs" line.
- **`--require-matching-meta [--declare k1,k2,…]`:** when any differing key
  is not declared, the run exits 2 and lists those keys, and no verdict is
  given.
- **Ignored keys:** a default ignore-list (timestamps, run ids, timings:
  `*.time*`, `*timestamp*`, `run.id`), which `--meta-ignore` extends.
- **Scope:** `compare`, `identity` and `view` all read sidecars. `view` shows
  the diff per set.

## First consumer

The Moss capture tooling will write `cost-card.json` beside each capture
directory. Its perf/quality lanes use `flipdiff identity` for optimisation
identity proofs and `compare`/`view` for image A/B.
