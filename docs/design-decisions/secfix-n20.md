# N20 security review fixes: findings 4–9

Scope: rooted CLI review preview/execution and MCP review, with the shared evidence
and generated-output helpers they consume. `origin/main` was merged first
(`d04faa8aaff06154965c64dcae74cacdde6326ed`, merge
`4f54ef4941cf5b2f3d4ea644e6a7305d4f965d77`). No changes to main were pushed.

| Finding | Root-cause fix | Focused regression |
| --- | --- | --- |
| 4 | Consume resolved report, intent and output routes; write the preview summary inside the original policy scope. | `rooted_relative_report_intent_and_output_use_authorized_paths` |
| 5 | Preflight all review output companions and use policy-bound atomic I/O for the marker, plan and index. | `mcp_preview_companion_marker_symlink_is_refused_without_truncation` |
| 6 | Acquire canonical roots with a no-follow directory walk; retain directory handles; read and create/replace outputs relative to those handles. Evidence hashes preserve the original alias authorization route. | `checked_read_and_write_replacement_races_are_confined`; additional startup and alias regressions |
| 7 | Run the guarded case-input check before MCP companion decoding or `expected_case_id` checking. | `mcp_expected_case_id_guards_companion_before_decode` |
| 8 | Exclusive same-directory temporary file, sync, atomic rename; never truncate an existing output inode. Index append uses a stable directory-relative coordination lock and atomic replacement. Manifest discovery excludes that internal lock. | `cli_preview_hardlinked_output_preserves_input_bytes`; additional output/index inode regressions |
| 9 | Require a successful MCP review preview with case/plan/marker and zero dispatch; add execution-mode path probes with real startup authorization and the case-ID companion probe. | `test_all_review_previews_refused_cannot_pass_positive_control` |

All fixtures are generated locally. The four transport tests fail against the
exact merged pre-fix source (0/4 pass). The harness control test fails against the
old harness because its real call group contains no successful review control.
For the race test, the legacy check-then-ambient-open implementation was
transplanted into the deterministic seam in a temporary source copy: the test
fails because the replaced path opens successfully. The production implementation
contains no environment-triggered race seam; tests supply callbacks directly to
the private functions also used by production with an empty callback.

Rejected alternatives: re-canonicalizing immediately before an ordinary open
still races; checking only the final symlink misses parent replacement; rejecting
symlinks before direct writes leaves hardlink truncation. Directory handles and
atomic replacement address those root causes without changing the read/output
root contract. Startup aliases remain supported after canonicalization, while
replacement symlinks during handle acquisition are refused.

The I/O scope is synchronous and thread-bound; it restores prior policy on error,
return or panic and cannot cross threads. Human configuration, credentials and
attempt-ledger authority remain separate from tool paths. New permissive
capability dependencies and their transitive additions are recorded in
`THIRD_PARTY.md`.

Validation receipts and final source/binary identity are retained outside the
repository. Final gate results follow below.

| Gate | Exit |
| --- | --- |
| fmt | 0 |
| clippy workspace/all targets, default | 0 |
| clippy workspace/all targets, every feature except imgtune-avif | 0 |
| workspace tests, default (736 passed, 25 explicitly ignored) | 0 |
| workspace tests, every feature except imgtune-avif (850 passed, 60 explicitly ignored) | 0 |
| rebuilt focused capability tests (6 passed) | 0 |
| native authority regression harness | 0 |
| container authority regression harness, read-only baseline mount | 0 |
| strict authority acceptance harness | 1, existing credential XFAIL |
| harness verdict tests | 0 |
| gen-docs --check, static and feature binary (explicit AVIF exception) | 0 |
| public hygiene and its tests | 0 |
| guides, default (32 run, 2 feature skips) | 0 |
| guides, feature binary (28 run, 6 unavailable-feature branch skips) | 0 |
| Windows and macOS core compile checks, no default features | 0 |

Initial clippy caught a test-only cloned-reference style issue. The first full
workspace run caught inclusion of the new coordination lock in manifest counts;
production discovery was corrected without weakening the existing assertions.
Both gates passed on the final source. A focused stock-core run reused the
pre-fix seam executable from the shared target after the temporary-source proof.
The temporary source was removed, core artifacts were cleaned, and the six
current tests were rebuilt and passed. The complete default and feature suites
also execute those current tests successfully. Earlier receipts are retained,
not relabeled as passes.

Remaining boundaries: the separate human baseline credential is still absent;
native/container receipts preserve XFAIL and `acceptance: failed` even when
`regression_gate: passed`. The native receipt has 46 PASS cases plus one XFAIL;
the read-only-mount container has 48 PASS cases plus one XFAIL. Windows/macOS
checks establish compilation of the capability layer, not OS runtime behavior.
Concurrent stress races, live providers and full filesystem/syscall sandboxing
are not established by these offline tests. Those wider qualifications remain
separate from the deterministic rooted-review fixes.
