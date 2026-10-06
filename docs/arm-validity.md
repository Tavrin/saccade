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
outside the intended variables or explicit ignores. There are no automatic
timing, timestamp or run-ID exceptions. Missing, null or empty identity never
counts as equal, including when both sides omit it or it is varied/ignored.
It checks every selected image's inherited and per-image metadata and every
supplied ablation repeat before exclusions. Session differences require an
explicit intended-variable declaration. Default comparisons retain warning
behavior. Fingerprint differences appear by canonical field name in metadata
reports, including under a field mapping.

The standalone check accepts JSON capture records or sidecars, images with
sidecars, or capture directories. It evaluates validity only, without decoding
pixels. `--vary TOKEN` and `--ignore TOKEN` are repeatable. A token matches an
exact key, a dotted prefix (`TOKEN.*`) or suffix (`*.TOKEN`); explicit globs
also work. For example, `--vary mode` matches `run.mode`, and `--vary run.env`
matches all selected environment fields. Use `--arm-ignore TOKEN` on verdict
commands. Ignores are echoed even when they match no keys; ignored differences
remain visible. An ignore cannot supply missing identity, waive a session mismatch, or waive
an unreached or mismatched readiness predicate.

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
`reached: true`, and a non-null observed value. Records compare by name,
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
fingerprint destinations to `{path, file?}` sources. `path` is a dotted object
path, with exact flat keys taking precedence. An omitted `file` selects the
primary capture record/effective inherited sidecar. A `file` selects a sibling
JSON file relative to the arm's capture root (or the JSON/image parent for
standalone files). Mappings allow at most 128 fields, 32 predicates and 32 sibling files; each
JSON read is at most 1 MiB. Traversal and symlink files are refused.
Missing sibling files or source fields remain missing.

Destinations are `producer.binary`, `producer.build` or `producer.build.*`,
`inputs.identity`, `run.mode`, `run.env` or `run.env.*`, `run.readiness[]`, and
`run.session`. `run.readiness[]` selects a whole native readiness list. The
optional `[[readiness]]` form adapts independent producer flags to named
criteria with fixed parameters and sourced reached/observed values. Several
sibling files can contribute to one fingerprint. Mapped source keys become
canonical keys; other primary metadata and sibling metadata remain comparable
(the latter as `file:NAME.KEY`). Do not map volatile flags to content hashes.
The mapping itself binds readiness parameters; use the same mapping for both
arms and retain it with pipeline configuration.

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
`invalid_comparison`, `exit_code`, `offending`, `vary`, `ignore`, and `ignored`.
Each finding contains its canonical key, baseline and capture JSON values
(null for absence), and a reason. Directory findings prefix keys with their
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
`a`, `b`, optional `vary`, `ignore`, `meta_name`, and `fingerprint_map`.
Compare/identity/ablate and `reference_compare` accept `require_valid_arms`,
`fingerprint_map`, `intended_variables`, and `arm_ignore` under the existing
read-root policy. An invalid result is typed `invalid_comparison`, not a
regression or successful measurement.
