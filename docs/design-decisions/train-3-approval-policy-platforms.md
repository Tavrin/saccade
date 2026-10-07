# Train 3: approval policy discovery across platforms

Scope: signed-approval platform compatibility in `integration/train-3`, starting
at `0978856395b1b040d97c16fd3a90caf4439ffda7`.

## Evidence and root cause

[CI run 37610672235](https://github.com/Tavrin/saccade/actions/runs/37610672235)
contradicts the brief's proposed common directory-check cause:

- Windows has widespread `approval_policy_invalid: approval policy is
  inaccessible` failures. The Windows path in `signed_approval.rs` contains a
  literal BEL byte between `saccade` and `pproval-policy.json`. A malformed path
  produces an error other than NotFound, so discovery correctly fails closed
  instead of selecting the absent-policy default.
- macOS has one failing test target, `signed_approvals`, and one failing test,
  `required_mcp_comparison_refuses_unsigned_and_changed_baselines`. Its error is
  `CLI cannot replace required policy signers`, not inaccessible system policy.
  The fixture uses a temporary-directory spelling through a system directory
  alias; `current_dir()` resolves that alias. Literal comparison rejects the
  otherwise equivalent policy and relative CLI signer paths.
- This revision has no parent-directory ownership checks in policy discovery.
  It checks opened files with O_NOFOLLOW, UID and mode validation. macOS `/etc`
  being a symlink does not explain the observed failure.

## Decisions

1. Linux retains `/etc/saccade/approval-policy.json`. Only NotFound means absent;
   inaccessible paths, malformed policies, unsafe files and dangling final
   symlinks remain refusals. System policy retains precedence over HOME.
2. macOS uses `/private/etc/saccade/approval-policy.json`, avoiding the `/etc`
   alias. UID 0 is the system-file owner requirement; root:wheel is accepted
   without weakening the group/world-write prohibition. Compare signer parent
   directories canonically to accept platform directory aliases, but never
   canonicalize the final signer file. A required policy retains its original
   signer path after accepting an equivalent CLI spelling.
3. Windows discovers `%ProgramData%\saccade\approval-policy.json` using path
   components, with `C:\ProgramData` as the unset-variable fallback. Relative
   ProgramData values fail closed. ACL-aware enforcement is deferred: any
   discovered system or user policy refuses, and CLI enablement/signers refuse
   at startup. An absent policy with no enablement flags remains OFF.
4. Do not classify PermissionDenied, InvalidInput or other discovery errors as
   absence. Do not follow final trust-file symlinks or accept different signer
   files to make platform tests pass. Do not skip or weaken existing signed or
   authority tests. Existing signed integration tests remain Unix-only because
   Windows has no implemented trust boundary.

## Validation scope

New tests exercise policy-absent comparison on every CI OS, Windows refusal for
both policy files and CLI enablement, directory-alias equivalence with rejection
of different signers, and Unix insecure/unreadable/malformed/dangling policies.
Existing signed and authority coverage remains intact. Local Linux gate receipts
and cross-target attempts are recorded with the implementation handoff; native
macOS and Windows proof belongs to the CI jobs for the pushed revision.

Local Linux validation completed with exit 0 for fmt, workspace/all-targets
clippy with warnings denied, workspace tests, signed-approval unit tests (4),
existing signed integration tests (8), platform regression tests (2), authority
harness offline tests (4), gen-docs --check, public hygiene and its self-tests.
The unreadable-policy regression executed as a non-root user (UID 1000).

Installed Rust targets were cross-checked with `cargo check --locked -p saccade
--target TARGET`. Both attempts exited 101 before checking Saccade: Windows GNU
lacks `x86_64-w64-mingw32-gcc`; macOS compilation of ring selected host GCC,
which rejects `-arch`, `-mmacosx-version-min` and `-gfull`. These are unavailable
native build prerequisites, not successful platform checks. Native CI remains
the macOS/Windows proof. No platform toolchain installation or Docker image
build was needed for this lane.
