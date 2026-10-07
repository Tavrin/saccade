# Offline evidence replay

`replay pack` records a comparison as a self-contained directory. `replay verify`
checks its identities, runs the measurement again in a fresh temporary directory,
and compares the complete report's replay projection and measurement exit code.
A reproduced regression remains a regression; replay grants no approval or model
accuracy qualification.

Create an explicit `saccade-replay-recipe.v1` recipe. Paths resolve against the
recipe file's parent, including a bare filename in the current directory:

```json
{
  "schema": "saccade-replay-recipe.v1",
  "run": {
    "operation": "compare",
    "a": "baseline",
    "b": "candidate",
    "threshold": 0.01,
    "metric": "mean",
    "ppd": 67.0
  }
}
```

```sh
saccade replay pack recipe.json --out evidence-pack --json
saccade replay verify evidence-pack --json
```

To bind a report you already have, add `--report REPORT_JSON` when packing.
Packing re-executes the recipe and refuses a different report with
`replay_report_mismatch`. It retains the original report alongside the fresh one.
The recipe must express the original effective options; recipe v1 supports raster
file/directory compare with threshold, metric and observer setting. Other compare
flags, project policies, document inputs and providers require a successor recipe
rather than silently losing options. Directory inputs include all regular files,
including metadata sidecars. File inputs retain the named raster and its standard metadata, timing, clock
and object/material annotation sidecars.

For text comparison, replace `run` with:

```json
{
  "operation": "text",
  "a": "reference.png",
  "b": "candidate.png",
  "a_source": null,
  "b_source": null,
  "ocr": {
    "registry": "registry.json",
    "contract_id": "ocr",
    "library": "runtime/libonnxruntime.so.1.22.0"
  },
  "expect_text": ["Value -2.5"],
  "readable_confidence": 80.0,
  "moved_px": 3.0
}
```

The registry must contain the explicitly selected PP-OCRv5 contract and its
already provisioned, SHA-pinned detector, recognizer and dictionary. Recipe v1
requires the shipped ONNX Runtime 1.22.0 library pin. Packing embeds the selected
contract, immutable revisions, licences, provenance, model bytes and runtime;
it never downloads. For a model-independent observation replay, supply both
image-bound `a_source` and `b_source` files and set `ocr` to null. This reproduces
imported observations; it does not re-run OCR. All observed/expected text remains
data, including text that resembles instructions.

The pack includes `recipe.json`, `pack.json`, `pack.sha256`, inputs, exact CLI
binary and build identity, required schemas, full report, and the Lane C report
manifest/index. Model artifact filenames are content-addressed. Versioned pack,
recipe and receipt schemas ship with `saccade schema`; later formats will use
successor ids. The pack's seal detects manifest drift; it is a checksum, not a
signature or an authenticity claim. Distribute its recorded hash through a
trusted channel when authenticity matters.

Verification uses the **running CLI**, whose SHA-256 must match `tool/saccade`.
It never executes a binary named by pack data. On another compatible machine,
run the retained CLI explicitly:

```sh
./evidence-pack/tool/saccade replay verify evidence-pack --json
```

Use a compatible OS, architecture and native dependencies for that binary.
Verification works with a read-only pack and writes its temporary workspace to
`TMPDIR` (or the platform temporary directory). OCR needs a temporary filesystem
that permits loading its pinned native library; container tmpfs mounts need `exec`. The child receives an isolated
home/config, an explicit local runtime and no provider credentials. There is no
network operation in either supported recipe. There is no general sandbox for
arbitrary native code; only the two closed operations are accepted.

Limits: 256 MiB per file, 2 GiB per pack, 8,192 tree entries, 32 directory levels,
16 MiB JSON documents and 300 seconds per measurement. Symlinks, special files,
path traversal, unknown recipe options and uninventoried inputs are refused.
The destination must be new and outside the inputs. Failed creation leaves no
published pack. Verification never changes recorded evidence.

Both commands exit 0 with `saccade-replay-result.v1` only when reproduction
succeeds. `measurement_exit` separately records the original comparison's 0/1.
Refusals exit 2 with the existing typed JSON error envelope:

| Code | Meaning |
| --- | --- |
| `replay_input_changed` | Missing, added or changed input/sidecar/observation |
| `replay_model_changed` | Model bytes violate their recorded hash or registry pin |
| `replay_config_changed` | Recipe, frozen registry or command changed |
| `replay_tool_changed` | Retained or running CLI/build identity changed |
| `replay_runtime_changed` | Runtime bytes changed or violate the supported pin |
| `replay_schema_changed` | Schema bytes or version changed |
| `replay_report_changed` | Retained report bytes or identity changed |
| `replay_pack_changed` | Manifest seal or inventory structure changed |
| `replay_report_mismatch` | Fresh measurement/report differs from the recorded one |
| `not_reproducible_here` | Missing optional model/runtime, incompatible platform/feature/native dependencies, or execution timeout; never a pass |
| `unsafe_path` | Link, special file or escaping path |

The pack retains the existing `report_id` and records a separate `comparison_id`.
The replay projection omits report timestamps, relocatable artifact paths and
source backlinks, plus exactly `entries[].diagnostics.elapsed_ms` and
`entries[].diagnostics.shift.estimate_ms`: these time diagnostic computation,
so change on re-execution. It preserves every actual performance measurement,
observation, score, policy, warning, validity and missingness field. There is no
numerical tolerance or score-only comparison. The receipt's `report_id` names
the recorded report; `replayed_report_id` exposes the actual fresh id, which can
differ because diagnostic compute durations are bound by the existing report-id
contract. Exact recorded report bytes and input/config/schema bytes are checked
separately before re-execution.

For a clean-container proof, run `scripts/test-replay-pack.py` with an OCR-enabled
binary, provisioned cache, the pinned runtime, an output directory and an existing
compatible container image. The runner creates CC0 procedural screenshot and
scientific-text fixtures, runs both packs with network disabled and a read-only
filesystem, and checks changed input/config/model plus missing optional-model
refusals. It does not download an image or model, or qualify OCR accuracy.
MCP mirrors are a follow-up outside this command's scope.
