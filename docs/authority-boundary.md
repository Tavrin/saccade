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

The recipe copies the supplied binary, harness and host OpenSSH verifier with
its loader/shared libraries into a Python runtime. This lets the forged-proof
probe run real signature verification without installing packages over the network.
The verifier bundle is test infrastructure, not a distributed Saccade dependency.
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
| Forged credential, policy on | Complete reviewed record and forged SSHSIG | Exit 2, `approval_signature_invalid`, unchanged baseline and absent receipt |
| MCP signed consumer | Unsigned comparison with policy enabled at startup | Tool error `approval_signature_required` |
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
MCP anchor/approval mirrors remain a follow-up. The transport probes cover static filesystem attacks; separate deterministic
Rust tests cover review check/open replacement races. Live providers, optional
adapters and all host OS/CI permissions remain outside that evidence.

## N20 security fixes (review findings 4–9)

Rooted review consumes policy-resolved report, intent and output paths. Relative
inputs resolve under the first read root and relative outputs under `out_root`,
even when the process working directory contains conflicting files. MCP guards
adjacent evidence before checking `expected_case_id`. All output companions,
including `.saccade-run`, review plans and report indexes, receive output policy
checks as well.

Review uses pinned `cap-std` directory handles for evidence reads, content
hashes, sidecars, intent and visual source bytes, and for output directory creation
and replacement. The synchronous, thread-bound I/O scope restores the previous
policy on return or panic. Human startup configuration, credentials and the
attempt ledger retain their separate human-owned authority; they are not tool
input paths. Path-returning policy methods authorize display routes; consumers
must use the bound I/O methods for filesystem access. This scope is not an async
context and must not cross spawned threads.

Artifact writes create an exclusive temporary file in the destination directory,
flush it, and atomically rename it over the output entry. Existing output inodes
are never truncated, so output hardlinks cannot overwrite input bytes. Index rows
use a directory-relative lock and atomic replacement too. Capability operations
are portable to Windows/macOS; unsupported filesystem operations fail closed.
This is application path confinement, not a full filesystem/syscall sandbox.

Focused Rust tests inject replacement of an input file, an input parent, an output
parent, an output entry and root pathnames between authorization and open. These
are deterministic confinement tests, not timing-dependent concurrent stress tests.
The transport harness additionally requires a successful MCP review preview with
its case, plan, marker and zero dispatch count, exercises execution path attacks
with actual startup authorization, and probes companion access with
`expected_case_id`. Native mode and container mount evidence remain distinct;
the human-credential refusal passes under signed policy.
