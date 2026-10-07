# Merge train 1

## Scope and decisions

Base: `origin/main` at `7801bce`. Git operations and integration edits belong to
this train worktree. No other worktree was modified. Commit identity is Tavrin;
no AI trailers, force pushes, main pushes or GitHub PR merges.

Merge order: #30 `fix/quality-schema-features` (`85a3dc0`), #23 `feat/lane-C`, #22 `feat/lane-E`, #24 `feat/text-quality`,
#25 `feat/g23-incremental-index`, #27 `feat/g26-animated-lod`,
#29 `feat/g25-trustmark-decode`, #28 `feat/saccade-print`,
#26 `feat/g12-openrouter-live`. All merges use `--no-ff` and the prescribed
messages. Lane E's three terminal JSON output sites now call Lane C's manifest
writer with default anchors; no approval or last-good claim is inferred.

Changelog conflicts retain every addition under Unreleased. Feature and command
registrations are additive. Generated artifacts are regenerated together at the
end; the CLI header declares the complete all-features build, including AVIF,
print and text-quality, even though local dav1d is unavailable.

G26 keeps its explicit timestamp-list plan. Adapting Lane E's seconds-based
frame map to G26's paired millisecond captures, scope masks and ID buffers would
change the input contract; this is beyond a trivial additive integration.

PR #30 is merged first, as required by the amended brief. Its quality-report and
geometry schemas are retained byte for byte. All final gates must pass; there is
no schema-drift exception in this train. The earlier unpublished history was
archived before reconstructing the merge order; no remote history was rewritten.

## CI diagnosis and bounded fixes

- #22, run `37547144920`: Windows bare-role Playwright captures were classified
  unusable because Number file IDs can lose precision. The reporter uses BigInt
  device/inode identity, with canonical-path fallback for unavailable inode IDs.
  The regression simulates distinct IDs which alias as Numbers on every OS.
  CLI documentation drift is repaired by final compiled-help generation.
- #23, run `37551651146`: Windows R11 retry accounting hit a five-second wall
  deadline under concurrent filesystem work. Retry timing is injectable for
  recorded replies; production retains the monotonic wall clock and sleeps.
  The offline test retains its five-second limit and checks all three consumed
  requests, full backoff, and insufficient-deadline deferral without a retry.
- #26, run `37556072019`: clippy found a feature-dependent unused timeout.
  Compute the OpenRouter ceiling timeout at the call site; dispatch still
  recomputes remaining time immediately before sending.
- #28, run `37556430387`: core schema catalogue lacked the print discriminator,
  and packaging lacked the intended README inventory. The canonical print schema
  is shipped by core and shared by the print library. Register the print README,
  specify it in Cargo, and document the library sufficiently for the existing
  package guard. Preserve the catalogue and package assertions.
- #29 and #28: add only the eight explicitly named procedural TrustMark PNGs
  and the original constant CMYK JPEG to the package image allowlist. Arbitrary
  images, weights and extra payload/schema combinations remain rejected.
- Combined agent guide retains animation, TrustMark and print guidance within
  the existing generated-pack byte budget.
- CI originally triggers pushes only on main. Add `integration/train-1` to its
  push filter so the prescribed single train push triggers CI without a PR merge
  or a manual dispatch.

## Final fix locations

| PR / scope | Files and reason |
| --- | --- |
| #30 | Quality-report and geometry schemas retained exactly from `85a3dc0`; merge is first. |
| #23 | `crates/saccade-core/src/judge_provider/transport.rs:873`: injectable retry clock, with production deadlines unchanged. |
| #22 | `crates/saccade/src/measure_cmd.rs:89`, `:206`, `:422`: terminal output manifests; `integrations/playwright/reporter.cjs:32`: exact file IDs; `docs/cli.md:1`: combined generated help. |
| #24, #25 | No corrective implementation changes required. Additive registrations and changelog entries retained. |
| #27 | No corrective implementation changes; explicit paired capture timestamps retained for the contract reasons above. |
| #29 | `scripts/check-packages.py:23`: exact eight-image TrustMark fixture inventory, with all eight hashes checked against provenance. |
| #28 | `crates/saccade-core/src/schema_catalog.rs:5`: shared embedded print schema; `scripts/check-packages.py:14`: print package inventory; `crates/saccade-print/Cargo.toml:4` and `README.md:19`: explicit README contract; `crates/saccade/tests/agent_contract.rs:90`: additive command count, including print. |
| #26 / merge | `crates/saccade-core/src/judge_provider/transport.rs:738`: remaining ceiling timeout at use; `crates/saccade/tests/agent_contract.rs:484`: one explicit isolated user configuration per review invocation. Auto-merged duplicate options had caused usage errors before the unchanged denial assertions. |
| Global OCR | `crates/saccade-core/src/general/ocr.rs:147`: verify all pinned artifact bytes before runtime discovery. The existing pin-before-runtime assertion is unchanged. CLI preflight continues to check runtime availability before input work. |
| Train CI | `.github/workflows/ci.yml:6`: one train push triggers CI; `scripts/validate-plugin-manifests.py`: executable bit fixed for the prescribed direct gate. |

## Validation

Final Rust validation source: `82d147ba6481b2a73fa95795adfcb55f88403b2e`.
The handoff commit changes this report only. The local feature list is every
workspace/CLI feature except `default` and `imgtune-avif`; normal defaults remain
enabled. `default` is a Cargo feature selector, not an omitted runtime capability.
The machine has no system dav1d, so AVIF compilation is left to CI as prescribed.

| Required gate | Final exit |
| --- | --- |
| `cargo fmt --check` | 0 |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 |
| Same clippy command with every feature except `imgtune-avif` | 0 |
| `cargo test --locked --workspace --no-fail-fast` | 0 |
| Same workspace tests with every feature except `imgtune-avif` | 0 |
| `python3 -m unittest discover -s scripts/assist` | 0 |
| `python3 scripts/gen-docs.py --check` | 0 |
| `scripts/validate-plugin-manifests.py` | 0 |
| `bash scripts/check-public-hygiene.sh` | 0 |
| `scripts/test-guides.py` | 0 |

The initial default test exit was 101 for the duplicate review-fixture options;
the initial feature-rich test exit was 101 for OCR runtime-before-pin ordering.
Both failures, their source identities, and the corrective full-suite receipts
are retained. Corrections preserve assertions, thresholds, feature availability
and production retry limits. The validator's initial launch was rejected by its
non-executable mode; after the mode fix its direct invocation exited 0.

Additional checks passed: package inventory/README guard, its nine regression
tests, four generator regression tests, compiled CLI-help comparison with the
prescribed header normalization, exact TrustMark fixture hashes, all nine PR-head
ancestry checks, and commit identity/trailer checks. The reporter regression
fails against the original Lane E reporter and passes against the train.

The assist Python suite ran 14 tests. Guides ran 21 blocks, skipped five
unavailable-feature cases because this build has those features, and failed none.
Existing ignored tests requiring supplied models/runtimes remain ignored;
these gates do not establish live-provider, model-accuracy, press or GPU acceptance.

Generated files were rewritten together once. Subsequent generator checks were
read-only. All-features CLI header wording comes from the fetched main revision;
its feature inventory includes `imgtune-avif`, `print` and `text-quality` in
alphabetical order.

No PR is left out. There is no schema-drift waiver: the feature-rich schema type
snapshot and optional-link compatibility tests passed after #30's first merge.

Evidence is retained in the operator-owned `saccade-train-evidence` directory: initial
`rust-receipts.json`, final `correction-receipts.json`, `light-receipts.json`,
per-command logs, PR failing-job logs, ancestry/fixture/CLI receipts, the original
unpublished-history bundle, and `target-cleanup.json`. The dedicated Cargo target
has been deleted under its Cargo lock. Remote CI is the next acceptance step;
only one train push is intended, with no PR merge or main push.


Measured Cargo target peak: 9,547,067,225 bytes (9.547 GB), below 12 GB.
