# Validating comparison arms

Use strict arm validation in automated pipelines. A pixel verdict is meaningful
only when the captures differ along the declared experiment variables.

```sh
saccade arms check baseline.json candidate.json --vary run.mode --json
saccade compare BASE CANDIDATE --require-valid-arms --intended-variable run.mode --out REPORT --json
saccade experiment ablate BASE CANDIDATE --require-valid-arms --intended-variable run.mode --out REPORT --json
saccade experiment reference candidate.png baseline.png --require-valid-arms --json
```

`compare`, `prove identity`, experiment `ablate`, `sequence`, `rank`, `temporal`
and `reference`, and `localized-check` accept `--require-valid-arms`.
The configuration equivalents are `require_valid_arms = true`,
`intended_variables = ["run.mode"]`, `fingerprint_map = "fingerprint-map.toml"`
and `arm_ignore = ["capture.timestamp"]`. Config mapping paths resolve relative
to the config file; command-line mapping paths resolve relative to the command.

Strict mode refuses measurement if any fingerprint or metadata field differs
outside the intended variables, explicit derivations or ignores. There are no automatic
timing, timestamp or run-ID exceptions. Explicit JSON null is a value: null equals null, and null versus a value is
a difference (exit 3). Only an absent key is missing (exit 4). Required typed
identity fields still reject malformed values. `--ignore` can explicitly waive
null and missing fields; waived findings retain both states in the receipt.
It checks every selected image's inherited and per-image metadata and every
supplied ablation repeat before exclusions. Session differences require an
intended-variable or derivation declaration. Default comparisons retain warning
behavior. Fingerprint differences appear by canonical field name in metadata
reports, including under a field mapping.

The standalone check accepts JSON capture records or sidecars, images with
sidecars, or capture directories. It evaluates validity only, without decoding
pixels. `--vary TOKEN` and `--ignore TOKEN` are repeatable. A token matches an
exact key, a dotted prefix (`TOKEN.*`) or suffix (`*.TOKEN`); explicit globs
also work. For example, `--vary mode` matches `run.mode`, and `--vary run.env`
matches all selected environment fields. Use `--arm-ignore TOKEN` on verdict
commands. Ignores are echoed even when they match no keys; ignored differences
remain visible. Ignores waive explicitly selected null and missing fields and ordinary differences.
They cannot waive a session mismatch or an unreached/mismatched readiness predicate;
use the specific readiness policy below for intentionally unconverged captures.

## Producer fingerprint schema

Emit a `fingerprint` object in `saccade-meta.json`, a per-image
`frame.saccade-meta.json`, or the JSON record passed to `arms check`.
A standalone fingerprint object with its own schema is accepted too.
The version is `saccade-arm-fingerprint.v1`; the shipped
[fingerprint schema](../crates/saccade-core/schemas/saccade-arm-fingerprint.v1.schema.json)
describes the optional declarations. Strict validation requires every group:

```json
{
  "fingerprint": {
    "schema": "saccade-arm-fingerprint.v1",
    "producer": {
      "binary": "sha256:0123456789abcdef",
      "build": {"profile": "optimized", "features": ["feature-a"]}
    },
    "inputs": {"identity": "sha256:abcdef0123456789"},
    "run": {
      "mode": "fixed",
      "env": {"FEATURE_A": false},
      "readiness": [
        {
          "criterion": {"name": "settled", "parameters": {"epsilon": 0.01}},
          "reached": true,
          "observed": 0.001
        },
        {
          "criterion": {"name": "receiver_ready", "parameters": {}},
          "reached": true,
          "observed": true
        }
      ],
      "session": "comparison-001"
    }
  }
}
```

Example hash strings are illustrative; producers should emit full content
hashes. `producer.build` must have at least one known field; `run.env = {}`
means explicitly no selected flags. Readiness is a nonempty list of unique
criterion names. Every record requires exact parameters (possibly `{}`),
`reached: true`, and an observed value (including explicit null). Records compare by name,
independent of order. Different names or parameters refuse even if both arms
claim convergence or declare the readiness fields as varying. Observation
changes are ordinary differences and can be declared explicitly.

To add `inputs.identity`, hash the prepared content deterministically, or hash
a preparation report that binds all content hashes, preparation settings and
the preparer's executable/version identity. Record the full algorithm and hash.
A cache location or hit/miss flag alone supplies no content identity. If the
producer has no such hash yet, strict mode reports `inputs.identity` missing
and exits 4. Saccade checks producer declarations; it does not authenticate
hash claims or infer prepared-input identity from pixel similarity.

## Mapping existing producer records

`--fingerprint-map FILE` accepts TOML or JSON. `fields` maps canonical
fingerprint destinations to `{path, file?, derives?}` sources. `path` is a dotted object
path, with exact flat keys taking precedence. An omitted `file` selects the
primary capture record/effective inherited sidecar. A `file` selects a sibling
JSON file relative to the arm's capture root (or the JSON/image parent for
standalone files). Mappings allow at most 128 fields, 32 predicates and 32 sibling files; each
JSON read is at most 1 MiB. Traversal and symlink files are refused.
Missing sibling files or source fields remain missing.

Destinations are `producer.binary`, `producer.build` or `producer.build.*`,
`inputs.identity` or `inputs.*`, `run.mode`, `run.env` or `run.env.*`, `run.readiness[]`, and
`run.session`. `run.readiness[]` selects a whole native readiness list. The
optional `[[readiness]]` form adapts independent producer flags to named
criteria with fixed parameters and sourced reached/observed values. Several
sibling files can contribute to one fingerprint. Mapped source keys become
canonical keys. The map-level `compare` mode selects the remaining fields:

- `compare = "all"` (default): compare every effective field, including unmapped
  primary metadata and sibling metadata (the latter as `file:NAME.KEY`).
- `compare = "mapped_only"`: compare mapped destinations, their `derives`, and
  mapped readiness criteria. Other fields do not affect validity. This is the
  recommended mode when producer records mix setup with outcomes.

`mapped_only` requires all mapped fields on both arms, including explicitly
mapped fields absent on both sides. Null remains a value; missing mapped fields give
exit 4. In `mapped_only`, only named identity groups are required; choosing a
smaller map makes a narrower validity claim. An unlisted native identity field
is unmapped. In `all`, the complete native identity requirements remain.

Optional map-level `outcomes = ["timing.*", "results.*"]` globs exclude matching
effective keys in either mode, including mapped keys. Matching happens after
source canonicalization; use canonical destinations for mapped outcomes and
`file:NAME.KEY` for sibling keys. Outcomes take precedence over map selection.

`arms check --compare mapped-only|all` overrides the map and echoes the effective
mode as JSON `compare` (`mapped_only` or `all`). `mapped-only` requires a map.
The `unmapped` and `outcomes` objects each contain a distinct-key `count`, sorted
`keys` capped at 64, and `truncated`. Lists union both arms across selected
images. Unmapped keys are compared in `all` and excluded in `mapped_only`;
explicit outcomes are always excluded. Human output summarizes the same counts.

Source lookup uses dotted object paths and exact flat keys; numeric components
are object keys, not array indices. Select a whole array as a field or expose
object-shaped setup fields in the producer record. Do not map volatile flags to content hashes.
The mapping itself binds readiness parameters; use the same mapping for both
arms and retain it with pipeline configuration.

A field's optional `derives` list names exact effective metadata keys computed
from that field. For example, a cache key that includes the executable hash:

```toml
[fields."producer.binary"]
path = "exe.hash"
derives = ["cache_key"]
```

When `--vary binary` (or a declared intended variable) covers `producer.binary`,
a changed `cache_key` is reported separately as **covered by derivation** in
`covered_by_derivation`, with both baseline and capture values. It is not an
extra difference and does not cause exit 3. Without `derives`, that same cache
change remains undeclared and exits 3. Targets use canonical keys after mapping,
or unmapped primary keys / `file:NAME.KEY` sibling keys; no target globs are
accepted. Each field allows at most 128 derived keys. Explicit chains propagate
coverage; cycles alone grant none. Ignores do not activate derivations.
Missing identity refuses unless explicitly ignored; malformed identity and readiness
failures still refuse comparison. These are producer declarations,
not verification that the values were actually computed from one another.

See the runnable [generic example](../examples/arm-validity/fingerprint-map.toml).
Its generated text fixtures bind a binary, source revision and dirty/diff
state, prepared input identity, environment, mode and session. Try:

```sh
saccade arms check examples/arm-validity/baseline/capture.json examples/arm-validity/candidate/capture.json --fingerprint-map examples/arm-validity/fingerprint-map.toml --vary mode --json
```

The two fixtures differ only in mode and produce exit 0 with `--vary mode`;
without that declaration they produce exit 3. Their abbreviated content hashes
represent synthetic identities, not external assets.

## Result and exit codes

The [result schema](../crates/saccade-core/schemas/saccade-arms-check.v1.schema.json)
uses `schema: saccade-arms-check.v1`, `result: valid_comparison` or
`invalid_comparison`, `exit_code`, `offending`, `vary`, `ignore`, `ignored`, and
`covered_by_derivation`, `allowed_unreached`, `diagnostics`, `compare`,
`unmapped`, and `outcomes`.
Each finding contains its canonical key, baseline and capture JSON values
(null for absence), `baseline_state`/`capture_state` (`missing`, `null`, or
`value`), and a reason. Directory findings prefix keys with their
image name. A refusal has no pass/fail verdict and produces no measurement
report. Existing output artifacts are left untouched.

| Exit | Meaning |
| --- | --- |
| 0 | Valid arm pair (standalone); verdict commands then follow their measurement result |
| 1 | Measured regression or evidence threshold failure |
| 2 | Usage, config, input parsing or execution error |
| 3 | Undeclared difference, mismatched/unreached readiness, or malformed identity |
| 4 | Missing identity or metadata; takes precedence when other violations coexist |

Successful strict compare results echo declarations and ignores in
`data.arm_validation`; full reports record `config.meta.require_valid_arms`
and `config.meta.arm_ignore`. Reference reports include `arm_validation`.

MCP `saccade_measure` mirrors the standalone command with `operation: arms_check`,
`a`, `b`, optional `vary`, `ignore`, `meta_name`, `fingerprint_map`, and
`compare` (`mapped_only` or `all`).
Compare/identity/ablate and `reference_compare` accept `require_valid_arms`,
`fingerprint_map`, `intended_variables`, and `arm_ignore` under the existing
read-root policy. An invalid result is typed `invalid_comparison`, not a
regression or successful measurement.


## Intentionally unconverged pairs

`arms check A B --allow-unreached warmup --json` permits the `warmup` criterion
only when both flags are exactly `false`, the named criterion and parameters
match, and both observed values are present and exactly equal. Different
observations, one reached arm, missing observations, or different criteria
still refuse, including with `--vary run.*`. The visible `allowed_unreached`
list contains the criterion and both observations; it does not claim convergence.
Verdict commands accept the same flag; config uses `allow_unreached = ["warmup"]`.

A fingerprint map may declare this for one criterion:

```toml
[[readiness]]
name = "warmup"
unreached_policy = "matched"
[readiness.reached]
path = "warmup.reached"
[readiness.observed]
path = "warmup.frame_index"
```

## Run-record discovery in directories

A map's `record_files = ["capture.json", "cost-card.json"]` reads those ordered
JSON files from each arm root and merges them into the primary record (later
files win). Inherited/per-image sidecars then override the run record. Ordinary
source paths resolve against that merged object; explicit `file` mappings keep
their existing sibling semantics. Missing files do not become null values.
When no fingerprint identity is found, the check's `diagnostics` names the files
searched and explains how to select the record with `record_files` or `--meta-name`.
Each record is bounded to 1 MiB; traversal and symlinks are refused.

A generic preparation identity map uses this producer shape:

```json
{"binary":{"hash":"sha256:binary"},"content":{"hash":"sha256:input","preparation_report_sha256":"sha256:report","preparer":null},"cache_key":"sha256:cache"}
```

The `preparer` may instead be `{"commit":"revision","dirty":false}`. Retain
explicit null when preparation predates the identity stamp; omit a key only
when its value is actually missing. Add these entries to a complete map:

```toml
record_files = ["capture.json"]
[fields."producer.binary"]
path = "binary.hash"
derives = ["cache_key"]
[fields."inputs.identity"]
path = "content.hash"
[fields."inputs.preparation_report_sha256"]
path = "content.preparation_report_sha256"
[fields."inputs.preparer"]
path = "content.preparer"
```

MCP `arms_check`, strict comparisons and `reference_compare` accept
`allow_unreached` as a list of exact names. Strict full reports retain the
validation receipt, including ignored states and allowed observations.
