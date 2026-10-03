# CI containment and Showcase Pages repair

Binding brief: `/home/etienne/dev/specs/saccade/lanes/SPEC-ci-fix.md`.
Checkout: `~/dev/saccade-wt/ci-fix`, branch `lane/ci-fix`, starting at
`2c0c248c4d44e87c751ae5a10d69826a50b38fa4`.

## Cause and decisions

- CI run `37144282777` fails the MCP `agent_contract` and `v1_contract`
  containment tests on Windows and macOS. R5 canonicalizes registered roots
  using `paths::canonicalize`, but compares raw lexical input/output routes
  against those canonical roots. Windows verbatim/non-verbatim prefixes and
  short/long names can differ; macOS temp paths use `/var` aliases for
  `/private/var`. These spellings can name the same filesystem location.
- Resolve native Windows path spelling before deciding whether a request is
  absolute. Normalize each route through its first registered root or output
  boundary using `run::normalise_path` and its shared `paths::canonicalize`
  helper. This also handles an output directory that does not exist yet.
  Preserve the remaining lexical route, then check the fully canonical
  destination separately. Canonicalizing the whole route for root selection
  would erase descendant symlinks and bypass cross-root `follow` permissions.
- Retain the separation between read-only roots, storage targets authorized
  only through root aliases, and generated outputs. Outputs reject `..`,
  capture/target destinations, and symlink escapes. `mcp.rs` needs no edit.
- One Unix regression simulates macOS temp aliasing with a symlinked directory.
  Before the product change it exited 101 with `input escapes registered roots`.
  It covers absolute/relative input routes, absent/generated output routes,
  authorized storage aliases, forbidden direct target access, cross-root links,
  symlink-plus-parent traversal, and output escapes/capture writes.
- Pages run `37144283200` fails `photosensitivity/safety` with
  `feature_unavailable: requires feature prechecks`. Its release build now
  enables `--features prechecks`. The other showcase entry points,
  `scripts/run-showcases.sh` and `docs/showcase/build.py`, consume an existing
  binary and now state that requirement. No other workflow runs showcases.

## Verification

| Gate | Exit | Evidence |
| --- | ---: | --- |
| Regression before product fix: `cargo test --locked -p saccade-core --test root_policy` | 101 | Reproduced canonical-root mismatch on Linux |
| Same focused regression after fix | 0 | Aliases accepted; unauthorized reads/writes rejected |
| `cargo fmt --check` | 0 | Clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 | Clean |
| `cargo test --workspace --locked --no-fail-fast` | 0 | 244 passed, zero failed |
| `.tools/actionlint` | 0 | All workflows validated |
| `cargo build --locked -p saccade --features prechecks` | 0 | Pages gallery probe binary |
| Required `verify-lane.sh ~/dev/saccade-wt/ci-fix` | 0 | PASS; 9/9 showcases reproduced; Moss ablation exit 0 |

Native Windows/macOS acceptance belongs to the coordinator's GitHub CI run
after pushing. Only files are edited; no Git mutations or subagents are used.
Cargo used `/mnt/linux-extra/moss-cargo-targets/codex-saccade-ci-fix`, two
build jobs, no incremental compilation, and no dev/test debug information.
The full tests and required verifier ran offline. The mandated lane target
was deleted after verification. The supplied verifier selects its own
`verify-ci-fix` target and removes its target, reports, temporary home, and
logs on PASS. The ignored `.tools/actionlint` entry links the existing local
binary at `~/dev/saccade/.tools/actionlint`; no tool download was needed.

## Additional failure reported, outside this brief

Running `docs/showcase/build.py` against the local `prechecks` binary gets
through all showcase commands, so the reported feature failure is resolved.
The gallery then exits 1 at `docs/showcase/build.py:535`: it still invokes the
removed `saccade snapshot` command, which exits 2 with `interface_removed` and
points to `saccade inspect export` as its replacement. This is a separate
R5 CLI migration in the gallery, outside the brief's two failures; it is
reported without changing gallery behavior. The subsequent, unexecuted block
also expects legacy JSON fields `verdict`, `totals`, and `failing` at lines
541–542. The complete Pages gallery is therefore not claimed to pass.
