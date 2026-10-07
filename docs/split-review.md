# Split-aware duplicate review

`saccade split-review MANIFEST --out REVIEW --json` reviews an explicit set of
images for candidate reuse across named partitions and groups bursts within each
partition. It needs no models for the stock hash and geometric routes. It never
deletes files or approves a dataset. A dataset can use train/validation/test,
holdout/site names, or any other declared partitions; a burst folder can declare
one partition. This command does not depend on `batch`.

Create a `saccade-split-manifest.v1` JSON file beside the dataset:

```json
{
  "schema": "saccade-split-manifest.v1",
  "entries": [
    {"path": "train/original.png", "split": "train"},
    {"path": "test/export.jpg", "split": "test"}
  ],
  "injected_pairs": [
    {"a": "train/original.png", "b": "test/export.jpg", "transform": "recompressed"}
  ]
}
```

Paths resolve relative to the manifest, must remain inside its directory and
must name distinct canonical files. Duplicate membership, traversal, symlink
escapes, unreadable files and invalid truth pairs fail with exit 2. Inputs are
bounded to 64 MiB encoded and 16 million decoded pixels per file; the manifest
is bounded to 1 MiB. The exhaustive pair review accepts 1–128 files. It holds
fingerprints, not the whole image set, and decodes each image once. Cost is
quadratic in file count; this is a bounded review, not a replacement for the
[incremental retrieval index](embeddings.md). Undeclared files are outside scope.

The JSON report is `saccade-split-review.v2` (the linked successor of the core
`saccade-split-review.v1` data contract), with report identity and source links.
The persisted filename is `saccade-split-review.v1.json`, following the existing
report writer convention. `index.html` presents the full evidence, and
`pairs.csv` exports cross-split pairs with both partition names and all route
evidence. `--json` emits the full report so routes and limitations remain visible.

Each cross-split candidate records one or more routes:

- **Hash:** encoded SHA-256 equality establishes exact file identity. Otherwise,
  pHash distance at or below `--hash-threshold` (default 6, range 0–64) supplies
  a candidate, not identity. Recompression and resize often preserve this hash;
  collisions and flat images can produce false matches.
- **Geometric:** existing FAST/oriented BRIEF and deterministic RANSAC consensus
  run on every nonidentical pair, including pairs rejected by the global hash.
  Matrix direction is `a_to_b`; the report retains the actual model, matches,
  inliers and RMS source-pixel residual. Crops can therefore match without a
  favorable global hash. Low texture, repeated patterns, severe crops and edits
  remain blind spots. Consensus is uncalibrated evidence.
- **Embedding:** opt in with `--embeddings` on an embeddings-enabled build, using
  the existing configured, hash-pinned export and local CPU runtime. Provision
  separately with `models pull embedding`; review never downloads or calls a
  provider. The full model/preprocessing identity and raw cosine threshold
  (`--cosine-threshold`, default 0.95) are retained. Cosine similarity does not
  establish duplicate identity. Missing configuration/runtime/models fail; the
  stock report explicitly marks this route `not_enabled`; that describes the
  current review, not whether a model is installed on the machine.

`cross_split_pairs` contains only pairs assigned to different partitions.
`within_split_pairs` retains the edges supporting `groups`, which are transitive
components confined to one partition. Two group members need not match each
other directly. Groups do not rank images or guess expressions; selection stays
with the reviewer. Exact/recompressed/resized/cropped are injected truth labels,
not transformations guessed from a candidate route.

Exit 1 means cross-split candidates need review; exit 0 means no cross-split
candidates were found by these routes (within-split groups may still exist).
**A clean list is never proof of no leakage.** Both text and JSON output state
that limit, and the report lists route use and known blind spots. Exit 2 means
the review could not complete. Failed inputs never disappear into a clean list.

When `injected_pairs` is supplied, the report counts every declared cross-split
truth pair, including misses, and reports overall and per-transform recall plus
the missed pairs. Reversed truth pairs are accepted but duplicate/reversed
entries are rejected. Absent truth produces `recall: null`, not perfect recall.
Measured injected recall does not estimate population recall or precision.

## Reproduce the generated proofs

```sh
python3 scripts/test-split-review.py --bin path/to/saccade --out proof-output
```

The script creates an ML-style dataset with two procedural source images using
different seeds and eight injected cross-split pairs (two each exact,
recompressed, resized, cropped), plus an independent negative control. It asserts
recall 8/8, per-transform recall 2/2, geometric evidence for both crops despite
hash distances above the configured threshold, and exclusion of the negative
from cross-split candidates. A separate photo burst contains
three related shots and an unrelated control; the proof asserts exactly one
three-member group, no cross-split candidates, null recall and the clean-list
caveat. Generated `provenance.json` records source seeds, licence
`MIT OR Apache-2.0` and exact input SHA-256 values. No imagery is downloaded.
These small proofs qualify those injected cases, not general photographic or
model retrieval quality. See the [tested guide](guides/split-review.md).

MCP mirroring is a follow-up; this command currently has CLI/core surfaces only.
