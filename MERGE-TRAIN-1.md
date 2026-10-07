# Merge train 1

## Scope and decisions

Base: `origin/main` at `7801bce`. Git operations and integration edits belong to
this train worktree. No other worktree was modified. Commit identity is Tavrin;
no AI trailers, force pushes, main pushes or GitHub PR merges.

Merge order: #23 `feat/lane-C`, #22 `feat/lane-E`, #24 `feat/text-quality`,
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

The quality-report schema feature drift remains owned by
`fix/quality-schema-features`. No train correction modifies that schema or
weakens its snapshot assertions.

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

## Validation

Pending the final gate batch. Exact command receipts and failure logs are retained
outside the Cargo target, which will be deleted after validation.
