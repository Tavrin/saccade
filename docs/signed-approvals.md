# Signed approvals

Signed approval is opt-in. By default CLI decisions remain content-bound,
unattested workflow records: anyone able to run the CLI and write a baseline
can apply one. This default is **unauthenticated**. Enable
`--require-signed-approval`, or provision `/etc/saccade/approval-policy.json`:

```json
{
  "require_signed_approval": true,
  "allowed_signers": "/etc/saccade/allowed_signers",
  "verifier": "/usr/bin/ssh-keygen",
  "max_approval_age_seconds": 604800,
  "ledger": "/var/lib/saccade/approvals.json"
}
```

The fixed system policy is checked before HOME and takes precedence over the
entire user policy. Changing HOME cannot hide an installed system policy.
The system policy must be root-owned. When absent, the fallback remains
`~/.config/saccade/approval-policy.json`; that fallback alone is a launch
convention, because a caller can select another HOME. Malformed, unreadable or
unsafe policy files fail closed. The CLI can strengthen a selected policy and
cannot replace signers when that policy requires signatures.

Policy signer, verifier and ledger paths must be absolute. Without a selected
required policy, `--approval-allowed-signers FILE` may select an external trust
file (relative CLI paths resolve against the working directory). The verifier
comes from the selected policy, with a fixed `/usr/bin/ssh-keygen` default;
PATH is never searched. Unix policy, signers, ledger and verifier checks apply
to opened regular files: no final symlink, effective-user or root ownership,
and no group/world write bits. System policy additionally requires root
ownership. Refusal uses `approval_trust_unsafe`. Provision trust files with
mode 0600 or 0644 and protect their parent directories against replacement.
Signers and policy are reread for each process; signers are rechecked on each
verification. The verifier executable and its loader/libraries must remain
protected throughout execution; an open-file check is not executable isolation.

Linux and macOS use these Unix checks and the same fixed system-policy path.
On macOS provision the policy under `/etc/saccade` (normally `/private/etc`)
and an absolute OpenSSH verifier. Native macOS execution is not qualified by
the Linux tests. Windows uses the fixed discovery path `C:\ProgramData\saccade\approval-policy.json`
and an absolute `C:\Windows\System32\OpenSSH\ssh-keygen.exe` default verifier path.
Windows lacks the implemented Unix ownership/mode boundary:
signed verification and trust-policy loading refuse with `approval_trust_unsafe`;
policy-off operation without a policy file remains unauthenticated. An ACL-aware
Windows implementation is deferred; there is no permissive fallback.

Protect the policy, trust files, executable and consumer account from agent
writes. An unrestricted shell can run another binary or edit baselines directly;
flags and user-owned configuration are not an OS security boundary.

## Review, sign externally, apply

Keep human private keys outside the repository and Saccade's config directory.
Use a separately held SSH key, hardware-backed SSH key or a human-only signing
agent. Saccade never reads private keys and never executes a signing command.
OpenSSH performs detached SSHSIG verification, using namespace
`saccade-approval`. This reuses the established OpenSSH signature implementation
and existing SSH key custody, rather than introducing a second key format or
cryptographic implementation. No new crypto crate is needed.

An allowed-signers file contains a principal, a namespace restriction and its
public key, for example `reviewer namespaces="saccade-approval" ssh-ed25519 ...`.
Use the actual public key in place of the ellipsis. Restrict signer lifetime or
remove a key from this file to revoke future verification. Verification uses
the verifier's current time; the signed timestamp is an audit claim, not a
trusted timestamp service.

```sh
saccade approve --report report/saccade-report.v1.json --entry sample.png \
  --approver reviewer --dry-run --out plan
# Human: inspect the report, decision.json, manifest.json and approval.json.
ssh-keygen -Y sign -f /secure/human/approval-key -n saccade-approval plan/approval.json
saccade approve --report report/saccade-report.v1.json --decisions plan/decision.json \
  --approval-record plan/approval.json --approval-signature plan/approval.json.sig \
  --require-signed-approval --approval-allowed-signers /etc/saccade/allowed_signers \
  --out receipt
saccade compare baseline candidate --approved \
  --approval-allowed-signers /etc/saccade/allowed_signers --out checked
```

`ssh-keygen -Y sign -f` can select the public key corresponding to a key held
by an external SSH agent. Do not expose that agent socket to untrusted agents;
an accessible signing agent grants signing power. Saccade's verifier does not
use an SSH agent. Arbitrary external signing workflows are supported by supplying
the resulting SSHSIG file; no signer subprocess runs inside Saccade.

The new `saccade-approval-record.v1` binds the semantic `report_id`, exact report
hash, canonical decision ID, approver principal, decision timestamp, absolute
canonical baseline directory, selected before/after hashes (including deletions)
and the **complete resulting baseline file inventory**, including metadata.
Signing approves that entire resulting inventory, including untouched files.
Sign the exact `approval.json` bytes; do not reformat it afterward. A partial
application of a larger signed scope is refused. Moving a baseline requires a
new approval. Optional `max_approval_age_seconds` refuses expired and future
signed timestamps using the consumer wall clock (`approval_expired`). This bounds
age; it does not establish which revision is current.

Optional `ledger` supplies rollback protection. Provision an existing trusted
regular JSON file containing `{}` at the configured absolute path, owned by the
consumer user or root, without group/world write access. Only the human-controlled
approval job should have write access; consumers need read access. The ledger
binds each canonical destination to its latest signed sequence and exact record
hash. Drafts include the next `sequence`; application requires exactly the next
revision and consumers require the current revision (`approval_replayed`).
Repeated consumption of the current approval is allowed; an old snapshot and
its old envelope are refused after a newer approval. Without a ledger matching
old records remain replayable. Enabling a ledger for an existing baseline
requires a new signed approval with a sequence; unregistered records fail closed.

Ledger access uses a file lock and writes/syncs the new revision before publishing
the baseline envelope. Concurrent applications of the same sequence cannot both
advance it. A crash or partial write may invalidate the ledger or leave baseline
and ledger inconsistent: consumers refuse until a trusted operator repairs state
and obtains a new approval. Ledger errors use `approval_ledger_invalid`. Do not
replace the ledger inode while jobs are running; protect its parent directory.
Rollback protection requires preserving this trusted state and a trustworthy
clock for age limits. Restoring both baseline and ledger defeats rollback detection.

After applying, `signed-approval.json` and baseline `.saccade-approval.json`
contain a `saccade-signed-approval.v1` envelope preserving exact signed bytes.
Existing evidence receipt schemas and their unattested CLI semantics remain
unchanged; this new signature domain adds independently verified authority.
A stored signature is not itself a validity assertion: consumers reverify it.
If an update fails partway, the old signature cannot validate changed content;
no successful signed approval is reported. Existing multi-file updates are not
transactional.

With the policy enabled, CLI and MCP stock measurements share a process-wide
core guard, including identity, rank, ablation and other helpers that use the
stock measurement engine. `compare` requires signed baseline contents, including
when given a last-good snapshot: passing history alone cannot supply approval.
`compare --approved` also requires verification with policy off. Ordinary
policy-off comparison retains its existing behavior. A signed baseline refuses
changed, added, removed or symlinked files. Signature records are excluded from
their own inventory. Traversal is limited to 64 levels, 10,000 files and 1 GiB
per file; records and trust files are limited to 4 MiB. At measurement, every
copied baseline byte is hashed into a private snapshot and checked against the
verified complete inventory before the engine reads that snapshot. Metadata and
ignored files are included. Replacement between the CLI precheck and measurement
is refused with `approval_content_mismatch`. Later changes to the original
baseline cannot supply measurement bytes. Report references retain the real
baseline destination. Sequence and subprocess batch/bisect workflows, plus routed
document/registration/question comparisons without
this snapshot contract refuse with `approval_consumer_unsupported` in signed mode.

Use `manifest build DIR --approved-anchor baseline/.saccade-approval.json`.
With policy on, building, verifying, linking and classifying manifests verifies
the declared signed anchor and its baseline inventory (manifest v1 and v2);
coverage views and link verification also
checks its manifest. Manifests without an approval anchor remain unapproved.
Per-case manifest v2 approved file references must belong to a signed baseline
and their declared hash must match its inventory. Last-good stays a separate role. A legacy anchor file cannot establish signed
authority. MCP comparison enforces the policy; MCP approval/anchor tools and
workbench approval integration remain follow-ups. The CLI policy does not
upgrade workbench authority.

Stable refusal codes (exit 2): `approval_signature_required` for missing proof,
`approval_signature_invalid` for malformed/untrusted/tampered signatures,
`approval_content_mismatch` for changed scope/content/destination,
`approval_policy_invalid` for missing signer configuration or invalid policy,
`approval_trust_unsafe` for unsafe/inaccessible trust files (including the
verifier path) or an unsupported platform, `approval_replayed` for a
noncurrent ledger revision, `approval_expired` for timestamp limits,
`approval_ledger_invalid` for invalid/full trusted state, and
`approval_verifier_unavailable` for failed execution or a timed-out OpenSSH verifier.
Existing path, stale-decision and IO checks still apply, and may refuse first.

## Threat model and deployment

An agent with writable baselines but without the human signing capability cannot
produce an approval accepted by a trusted policy-enabled consumer. It can still
corrupt or delete baselines (denial of service). Protect baseline directories
with a separate owner and mount them read-only for the agent account; permit
updates only through a separate human-controlled account/job. Run verification
in a trusted CI job with read-only policy/signers, an absolute trusted verifier and trusted binary, not
an agent-controlled wrapper. Keep reports outside baseline directories.

A compromised human key, a malicious authorized human, a compromised verifier
or policy, an exposed SSH-agent socket, and direct shell access to the trusted
verification account defeat this boundary. A signature proves possession of an
authorized key over particular bytes, not that a person inspected the images,
that the images are correct, or that the timestamp is true. It does not prevent rollback without the optional trusted ledger, or mutation
of the consumer's own private snapshot by someone controlling its account.
It does not isolate verifier execution or protect replaceable trust directories.
OS ownership,
read-only snapshots and isolation remain required for hostile concurrent agents.

The offline authority harness explicitly proves both policy-on credential
refusal, forged-signature refusal, policy-on MCP comparison refusal and
policy-off writable CLI behavior. Rust integration tests generate
original RGB images and ephemeral Ed25519 keys outside the repository; fixtures
are original CC0-1.0, have no downloaded content and keys are discarded.
