# Authority-boundary harness

`scripts/authority-boundary/harness.py` acts as an agent client of the real CLI
and newline-delimited stdio MCP server. It initializes MCP, sends the initialized
notification, lists tools, pings, and checks correlated tool responses. Fixtures
are original procedural RGB squares, generated locally and dedicated to CC0-1.0;
no images, fonts, models or provider credentials are downloaded.

Build a default-feature binary, then run as an unprivileged Unix user:

```sh
cargo build --locked -p saccade
python3 scripts/authority-boundary/harness.py --bin target/debug/saccade \
  --report authority-report.json
```

For the deployment proof, use Docker on Linux. Build/image provisioning can
use the network; the actual harness container uses `--network none` (loopback
only), a read-only root filesystem, an empty bounded temporary filesystem,
no capabilities, no inherited provider environment and no credentials mount:

```sh
scripts/authority-boundary/run-container.sh target/debug/saccade ./authority-reports
```

The recipe copies only the supplied binary and harness into a Python runtime.
It generates a separate baseline fixture and mounts it read-only. The harness
requires `EROFS` even when attempting to restore the current file mode, then
measures it and prepares a valid decision before testing the refused update.
An agent in this container cannot unlock that baseline with `chmod`.
Use a target and `TMPDIR` on a filesystem with sufficient build headroom.
The binary must be Linux/glibc-compatible with the pinned trixie runtime. No Cargo
build occurs inside the container. Never mount the host Docker socket into it.

The report uses `saccade-authority-boundary.v1`, with its schema in
`scripts/authority-boundary/report.v1.schema.json`. It records the binary SHA-256,
build identity, fixture provenance/hash, expected and observed stable codes,
CLI arguments/output, MCP request/reply pairs and filesystem invariants.
Future incompatible report contracts require a successor schema.

| Boundary | Probe | Required refusal |
| --- | --- | --- |
| Read roots | Existing outside file, `..` traversal, symlink escape | CLI rooted review: exit 2, `config`; MCP: `unsafe_path` |
| Output root | Outside sibling, prefix lookalike, traversal, symlink with new parents | CLI rooted review: exit 2, `config`; MCP: `unsafe_path` |
| Network startup | MCP review execution with no startup permission, forged permission argument | `network_authorization_required`, `usage` |
| Source egress | Deny-root review with execution/startup permission otherwise enabled | CLI exit 2 and MCP tool error: `egress_denied` |
| MCP baseline write | Unregistered approval tool or operation | JSON-RPC `-32602`, `usage` |
| OS baseline write | Valid approval against a read-only baseline mount | CLI exit 2, `io`, baseline bytes unchanged |
| Human credential, policy on | Valid CLI decision, no signature | Exit 2, `approval_signature_required`, unchanged baseline |
| Compatibility, policy off | Valid CLI decision against a writable baseline, no credential | Exit 0, baseline updated, null human attestation (explicit default behavior) |

CLI read/output probes exercise both preview and execution with a declared
user policy. Default or pricing-only local preview remains available without
declared roots. Ordinary CLI commands and an unrestricted shell do not inherit
MCP startup authority. The integrator must confine that process at the OS level.
Positive compare/preview/decision-plan controls distinguish real refusals from
missing features, invalid inputs, stale decisions or a broken MCP transport.
Denied operations must leave escaped output paths and the dispatch ledger absent.

## Separate signing authority

The original N20 finding was that a content-bound CLI draft alone could update
a writable baseline. [Signed approvals](signed-approvals.md) now add opt-in
external key verification. The harness records the former credential XFAIL as
a policy-on refusal PASS, and separately asserts policy-off writable behavior.
Neither a draft nor a command-line flag authenticates a person: policy-enabled
approval requires a signature by a trusted external key.

Strict mode exits 0 only when every current probe passes; CI uses strict mode.
The legacy `--allow-known-findings` option remains available for historical
harness consumers, but current cases have no expected failures. An unexpected
success/refusal, wrong code, side effect or missing case is a regression.
Exit 2 denotes a harness/setup failure. CI uploads receipts even on failure.

Native runs without the container only test Unix file modes; a same-owner shell
can restore those modes. The receipt distinguishes `unix_modes_only` from
`read_only_mount`; only the container recipe supplies the mount proof.
MCP anchor/approval mirrors remain a follow-up. These probes cover static
filesystem attacks on generated inputs, not concurrent rename races, live
providers, every optional adapter or every host OS/CI permission configuration.
