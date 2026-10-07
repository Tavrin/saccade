# Signed approvals

Signed approval is opt-in. By default CLI decisions remain content-bound,
unattested workflow records: anyone able to run the CLI and write a baseline
can apply one. Enable `--require-signed-approval`, or install this human-owned
`~/.config/saccade/approval-policy.json`:

```json
{"require_signed_approval":true,"allowed_signers":"/etc/saccade/allowed_signers"}
```

User-policy `allowed_signers` must be an absolute path. Unreadable or malformed
policy fails closed. The CLI flag can strengthen this policy, never disable it or replace its trusted
signers. Without a required user policy, `--approval-allowed-signers FILE` selects
an external trust file. Protect the policy, trust file, verifier executable and
launch environment from agent writes. An unrestricted shell can select another
HOME, run another binary or edit files directly: flags and a writable user
configuration are not an OS security boundary.

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
new approval. Old valid records may be replayed if all bound content matches;
there is no monotonic revision service or freshness guarantee.

After applying, `signed-approval.json` and baseline `.saccade-approval.json`
contain a `saccade-signed-approval.v1` envelope preserving exact signed bytes.
Existing evidence receipt schemas and their unattested CLI semantics remain
unchanged; this new signature domain adds independently verified authority.
A stored signature is not itself a validity assertion: consumers reverify it.
If an update fails partway, the old signature cannot validate changed content;
no successful signed approval is reported. Existing multi-file updates are not
transactional.

With the policy enabled, `compare` requires signed baseline contents, including
when given a last-good snapshot: passing history alone cannot supply approval.
`compare --approved` also requires verification with policy off. Ordinary
policy-off comparison retains its existing behavior. A signed baseline refuses
changed, added, removed or symlinked files. Signature records are excluded from
their own inventory. Traversal is limited to 64 levels, 10,000 files and 1 GiB
per file; records and trust files are limited to 4 MiB.

Use `manifest build DIR --approved-anchor baseline/.saccade-approval.json`.
With policy on, building, verifying, linking and classifying manifests verifies
the declared signed anchor and its baseline inventory; link verification also
checks its manifest. Manifests without an approval anchor remain unapproved.
Last-good stays a separate role. A legacy anchor file cannot establish signed
authority. MCP mirrors and workbench approval integration remain follow-ups;
the CLI policy does not upgrade those authority domains.

Stable refusal codes (exit 2): `approval_signature_required` for missing proof,
`approval_signature_invalid` for malformed/untrusted/tampered signatures,
`approval_content_mismatch` for changed scope/content/destination,
`approval_policy_invalid` for missing signer configuration or invalid user policy and
`approval_verifier_unavailable` for a missing/timed-out OpenSSH verifier.
Existing path, stale-decision and IO checks still apply, and may refuse first.

## Threat model and deployment

An agent with writable baselines but without the human signing capability cannot
produce an approval accepted by a trusted policy-enabled consumer. It can still
corrupt or delete baselines (denial of service). Protect baseline directories
with a separate owner and mount them read-only for the agent account; permit
updates only through a separate human-controlled account/job. Run verification
in a trusted CI job with read-only policy/signers and trusted PATH/binary, not
an agent-controlled wrapper. Keep reports outside baseline directories.

A compromised human key, a malicious authorized human, a compromised verifier
or policy, an exposed SSH-agent socket, and direct shell access to the trusted
verification account defeat this boundary. A signature proves possession of an
authorized key over particular bytes, not that a person inspected the images,
that the images are correct, or that the timestamp is true. It does not prevent
rollback, concurrent filesystem replacement or all TOCTOU attacks. OS ownership,
read-only snapshots and isolation remain required for hostile concurrent agents.

The offline authority harness explicitly proves both policy-on credential
refusal and policy-off writable CLI behavior. Rust integration tests generate
original RGB images and ephemeral Ed25519 keys outside the repository; fixtures
are original CC0-1.0, have no downloaded content and keys are discarded.
