# Capture-record conformance kit

Check externally acquired evidence before comparing it:

```sh
saccade capture conform examples/capture-kit/renderer-valid.json --json
saccade capture conform examples/capture-kit/browser-valid.json --json
```

The valid records pass (exit 0). Conformance is an offline contract check, not a
pixel verdict, performance qualification, authenticity check or baseline approval.
It verifies encoded image bytes without decoding pixels. A producer's planned
and observed executable/input identities are declarations; the checker cannot
authenticate either. Retain the independently declared plan before acquisition.

## Versioned record contract

[`saccade-capture-record.v1`](../crates/saccade-core/schemas/saccade-capture-record.v1.schema.json)
binds a `producer` (`renderer` or `browser`), nonempty `run_id`, explicit
`completion` (`complete` or `partial`), an `expected` slot list and all
`acquisitions`. New incompatible contracts will use successor versions;
existing fingerprint, performance and browser receipt formats stay unchanged.
Retrieve the installed schema with `saccade schema get saccade-capture-record.v1`.

Each expected slot has a unique `id`, exact `settings`, full native
[`fingerprint`](arm-validity.md) and `clock_domain`. Fingerprint executable and
prepared-input hashes use `sha256:` plus 64 lowercase hex digits. The build,
environment, mode, session and named readiness predicates follow strict arm
validation. Expected sessions must match `run_id`. No vary/ignore policy is
applied within a slot: this checks that acquisition honored its own plan.
Different slots can declare different settings and identities.

Each acquisition retains `id`, `status` (`captured`, `failed`, `skipped`),
`error`, `image`, observed `settings`, `fingerprint`, `clock` and `details`.
Unavailable observations are explicit nulls. `details` holds adapter observations
(for example final URL and HTTP status); it grants no authority. Failed/skipped
slots must remain recorded, including when a stale image exists. A captured slot
with any non-null error fails. An omitted acquisition is a partial run.

`image` is `{path, sha256}`: a relative regular file beside the receipt, bound by
the SHA-256 of its exact encoded bytes. Absolute paths, traversal and symlink
components are refused. The record limit is 1 MiB, the encoded-image limit 64 MiB,
and the slot limit 256. Slot IDs are at most 128 bytes. Clocks declare `domain`,
`run_id`, `start_ns`, `end_ns`; both domain and run must match the plan and time
must not reverse. Equal timestamps support deliberately frozen browser clocks.
This establishes declared clock consistency, not physical clock accuracy or GPU
clock qualification; the existing [clock checks](gpu-clock-mapping.md) remain separate.

## Generic producer adapters

`scripts/capture-record.py` writes either a renderer sidecar or a browser-capture
receipt from externally supplied observations. It does not launch a renderer,
browser or provider. Supply the plan as a JSON array, and acquisitions as a JSON
array using the acquisition fields above, with `image` as a relative filename
or null. The adapter computes image hashes for successful attempts and retains
failed/skipped attempts without creating image evidence. It copies observed
settings/identity/clocks independently from the plan and refuses overwriting receipts.

```sh
python3 scripts/capture-record.py --producer renderer --run-id generated-renderer-001 \
  --completion complete --plan examples/capture-kit/renderer-plan.json \
  --acquisitions examples/capture-kit/renderer-acquisitions.json \
  --out examples/capture-kit/renderer-sidecar.json
```

For a Playwright-style producer, use `--producer browser`, browser plan/observed
inputs and a browser run ID. Record actual stabilization settings, fingerprint,
clock and screenshot status after the attempt; failed screenshots remain failed
acquisitions. Do not copy requested settings into the observed fields without
checking that the producer applied them. Existing Playwright/sweep receipts are
not silently treated as this new contract.

## Generated adversarial receipts

Regenerate with `python3 scripts/gen-capture-kit.py`; verify byte identity with
`python3 scripts/gen-capture-kit.py --check`. No imagery, model or provider is
downloaded. The 8×8 gradient and receipts are procedural, dedicated to CC0-1.0;
[`provenance.json`](../examples/capture-kit/provenance.json) records the generator,
image hash and synthetic identity/clock limitations. Both producer families carry
each case; [`expected.json`](../examples/capture-kit/expected.json) freezes exits
and codes, asserted by the CLI integration test.

| Case | Stable code | Exit |
| --- | --- | --- |
| valid | no findings; `conformant: true` | 0 |
| missing image binding/file | `missing_image` | 1 |
| stale byte hash | `stale_hash` | 1 |
| failed acquisition, even with an image | `acquisition_failed` | 1 |
| mismatched or absent settings | `settings_mismatch` | 1 |
| omitted/skipped acquisition or declared partial run | `partial_run` | 1 |
| clock domain/run mismatch, absent or reversed clock | `clock_mismatch` | 1 |
| executable/input/build/session mismatch | `identity_mismatch` | 1 |
| missing/malformed producer identity | `missing_identity` | 1 |
| unreached readiness | `readiness_not_reached` | 1 |
| traversal/symlink image path | `unsafe_image_path` | 1 |
| unreadable/oversized image | `image_unavailable` | 1 |
| invalid plan, malformed/oversized JSON, duplicate/unplanned slot | `invalid_record` | 2 |
| absent receipt | `record_unavailable` | 2 |
| unsupported contract version | `unsupported_schema` | 2 |

Output is [`saccade-capture-conformance.v1`](../crates/saccade-core/schemas/saccade-capture-conformance.v1.schema.json):
`conformant`, planned/observed counts and `findings[{code,id}]`. All slots are checked;
independent failures are retained in plan order. A failed or skipped acquisition
has its state code without cascading missing-observation codes. Structural
record failures stop validation. Every adversarial fixture has exactly its own
expected code; no failed record is dropped or converted to a pass.

MCP exposure is a recorded follow-up. No live renderer/browser execution or
provider call is claimed by this kit. See also [performance kit](perf-kit.md).
