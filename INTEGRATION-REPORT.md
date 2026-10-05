# Waves 4–6 integration

Scope: `/home/etienne/dev/saccade-wt/integ`, branch `integ/waves-4-6`.
Base `cccd6e35d52f1472d7b51297cd20d75ea8127fe6`. Only this worktree's Git state
is owned; no push or changes to main/other worktrees. No subagents or live providers.

## Integration decisions

- Merge commits preserve Wave 4 (`2579457`), Wave 5 (`f8afd9e`) and Wave 6
  (`ae86d87`) in that order. Documentation/notices preserve all additions;
  registrations and feature manifests combine all waves. Lock resolution starts
  with Wave 6's pins and adds Wave 5's dependencies offline. Temporary markers
  are replaced by ordinary ordering and behavioral documentation.
- Discoverability lists assist and product commands, availability, prerequisites
  and authority. Assess/inspect-image reports include related commands. The
  generated agent guide is condensed to retain the existing 4800-byte pack limit;
  detailed contracts remain linked. README is preserved through --skip-readme.
- Matcher/sweep registration is explicit. It retains geometric exclusions and
  refuses ordinary config/masks, which the separate registration contract cannot
  honor. Matcher stability compares raw captures. Raw existing defaults remain.
- Butteraugli 0.4.0 is vendored with unchanged arithmetic. Its 12 multiversion
  attributes retain AVX2/SSE on Rust 1.88 and add AVX-512 for Rust >=1.89. The
  build script uses the actual compiler, refusing unrecognized versions safely.
  Rejected raising MSRV or removing newer fast paths. Reversal: remove workspace
  patch after a compatible upstream release. Cargo registry archives do not
  inherit patches: packaged Rust 1.88 needs the upstream fix or separately
  published compatible dependency; this branch does not publish either.
- dav1d development files (Ubuntu 1.4.1-1build1) are extracted under
  `/mnt/linux-extra/saccade-models/toolchain/dav1d`; system runtime 1.4.1 is reused.
  No system package installation. Browser/tool prerequisites use standard caches.
- Live qualification, provider/model conformance and actual unavailable model or
  credential fixtures are not fabricated. Failing prerequisite gates remain FAIL.

## Gate receipts

Pending one coordinated batch, followed by bounded fixes and necessary reruns.
Logs and exact commands will be recorded below. The prescribed target is removed
only after all execution has finished; evidence remains outside it.
