# Checking historical capture records

`capture conform --legacy-map MAP DIR` reads a user-owned
`saccade-capture-legacy-map.v1` mapping and checks retained historical evidence.
It writes no receipts or images. Producers keep their own formats and mappings;
there are no built-in producer-specific importers. See the fictional
[Atlas TOML map](../examples/capture-legacy/atlas/map.toml) and
[Beacon JSON map](../examples/capture-legacy/beacon/map.json).

```sh
saccade capture conform --legacy-map examples/capture-legacy/atlas/map.toml \
  examples/capture-legacy/atlas --json
saccade capture conform --legacy-map examples/capture-legacy/beacon/map.json \
  examples/capture-legacy/beacon --json
```

Both examples return **3**, with `conformant: true` and
`provenance: adapted_legacy`. Every mapped result, including invalid maps and
missing records, carries that provenance. Checking a native receipt without a
map still reports `provenance: native` and returns 0 on a pass. Adaptation proves
only the retained declarations and byte relationships; it does not prove that
plans were recorded before acquisition, authenticate producer declarations,
qualify performance, approve a baseline, or assess pixels.

## Mapping contract

The required keys are `schema`, `record`, `hash_policy`, `absent`, `fields`,
`expected`, and `acquisitions`; `retry_records` is optional. Unknown mapping keys
are rejected. `record` names a relative primary JSON file beneath DIR. TOML
and JSON mappings have identical semantics. Retrieve the schema with
`saccade schema get saccade-capture-legacy-map.v1`.

`fields` maps `producer`, `run_id`, and `completion`. `expected.source` selects
an independently retained plan array; `acquisitions.source` selects an observed
array. Each section's `fields` maps canonical contract destinations to sources
relative to its row. An entire object can be mapped as a subtree, for example
`fingerprint`, or individual nested fields can be assembled with destinations
such as `fingerprint.producer.binary`. Overlapping destinations are rejected.
No expected plan, identity, clock, settings or status is inferred from outputs.
Missing arrays stay unavailable. Empty arrays and contradictory or malformed
retained values fail the normal contract checks.

```toml
schema = "saccade-capture-legacy-map.v1"
record = "ledger.json"
hash_policy = "compute_now"
absent = "unavailable"

[fields.run_id]
path = "session.id"

[expected.source]
path = "requested_slots"

[expected.fields.id]
path = "name"

[acquisitions.source]
path = "attempts"

[acquisitions.fields."image.path"]
path = "output.filename"
```

This fragment illustrates the syntax; the linked examples supply complete maps.
Each source has `path`, optional `file`, optional `default`, and optional `values`.
`values` explicitly translates scalar producer/status/completion tokens (at most
32 entries), for example `values = { saved = "captured", error = "failed" }`.
Translation requires an actual source value; an unmapped token is invalid.
It never defaults a missing outcome. Dotted paths
support numeric array indices, with exact flat keys taking precedence; wildcards
are rejected. A `file` selects a sibling JSON document relative to DIR, even in
a row mapping. An absent source or null is unavailable. `absent = "unavailable"`
is the only supported absent policy. Defaults are permitted only for `producer`,
`error`, and `details`; evidentiary defaults are rejected. An assembled fingerprint object lacking `schema` receives the fingerprint
contract discriminator as a labelled formatting default; no identity values
are synthesized. A supplied unsupported discriminator still fails. Omitted optional
observations become null; omitted `details` becomes an empty non-authoritative
object. These decisions are included in `fields[{field,state,source}]`.

`retry_records` selects an optional retained array and echoes it in the report.
It grants no conformance authority. Map the producer's independently retained
terminal outcomes to `acquisitions`; do not select an earlier successful retry
in place of a failed terminal attempt. Duplicate terminal slot IDs are invalid.
Failed/skipped terminal attempts remain failed/partial even if an image exists.

## Hashes and unavailable requirements

`hash_policy = "recorded"` checks a mapped legacy SHA-256 declaration against
encoded bytes. Use this only when the producer retained that acquisition binding;
producer hash claims remain declarations, not authenticated timestamps.
`compute_now` hashes readable images and reports `state: hashed_at_check_time`.
It always reports `acquisition_hash_unavailable`: today's hash cannot establish
an acquisition-time binding. `unavailable` performs no historical hash claim.
There is no writeback. Preserve the report externally if check-time hashes are
needed for subsequent byte identity checks.

`unavailable[{code,field}]` distinguishes missing evidence from conflicting
evidence in `findings`. Stable missing-requirement codes are:

| Requirement | Code |
| --- | --- |
| Producer, run ID, completion | `producer_unavailable`, `run_id_unavailable`, `completion_unavailable` |
| Retained plan/acquisition array | `expected_unavailable`, `acquisitions_unavailable` |
| Planned slot, settings, fingerprint, clock domain | `planned_slot_unavailable`, `planned_settings_unavailable`, `planned_identity_unavailable`, `planned_clock_unavailable` |
| Observed slot/status | `observed_slot_unavailable`, `acquisition_status_unavailable` |
| Observed settings/fingerprint/clock | `observed_settings_unavailable`, `observed_identity_unavailable`, `observed_clock_unavailable` |
| Image path or acquisition-time binding | `image_path_unavailable`, `acquisition_hash_unavailable` |
| Requested retry history | `retry_records_unavailable` |

A record with unavailable required evidence has `conformant: false`. Contradictory
retained evidence uses the [normal conform codes](capture-kit.md), including
`stale_hash`, `settings_mismatch`, `acquisition_failed`, and `partial_run`.
If the plan cannot be established, checks that require it are unavailable;
failed/skipped states and readable image bindings are still inspected.

| Exit | Meaning |
| --- | --- |
| 0 | Native conformant receipt (without a mapping) |
| 1 | Contradictory/rejected evidence, including failed attempts |
| 2 | Invalid map/record, unsafe source, unreadable record, or archive bound refusal |
| 3 | Adapted pass of retained declarations; always `adapted_legacy` |
| 4 | Adapted result with unavailable required evidence, without stronger failure |

Invalid contracts take precedence over failures, failures over unavailable
requirements, and unavailable requirements over adapted passes.

## Archive trees and bounds

```sh
saccade capture conform --legacy-map producer-map.toml archive-root --archive --json
```

Archive output is JSON Lines with `{directory,report}` rows, ordered by directory
name. Directories containing the mapping's primary `record` are checked;
if none exist, a missing-record row is emitted for the root. Aggregate exit uses
the severity order above. Traversal errors or limits return one labelled refusal row for the root,
with `invalid_record`; partial rows are discarded. Symlink entries are refused; directories and files are
opened through handles that refuse symlink components, including source files,
images, map files and directory ancestors. Relative file/image names cannot
contain traversal or absolute components. Inputs stay read-only.

Limits: map and each JSON source 1 MiB; 32 cached source documents per directory;
128 mappings per section; 1 MiB cumulative selected/projected JSON and mapping metadata; 256 planned/acquired/retry rows per directory;
64 MiB per encoded image; archive depth 16, 4096 visited entries, and 256 record
directories. The limits stop processing rather than silently truncating evidence.
A mapping stays beneath each capture directory; records stored elsewhere must
be checked with their own declared root, never through escaping file mappings.

The two formats and all image bytes are procedural fixtures, dedicated to
CC0-1.0. Regenerate with `python3 scripts/gen-capture-legacy-kit.py` and check
with `--check`. Acceptance covers offline adapters and filesystem refusals,
not live producer execution or private archive qualification.

Archive JSON rows serialize `directory` with forward-slash separators on every platform.
