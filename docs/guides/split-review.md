# Guide: split leakage and burst review

Declare membership explicitly; review cross-partition candidate pairs with their
route evidence. Keep reports outside the dataset. See [split review](../split-review.md)
for the manifest contract, stock and optional routes, generated proof and limits.
These blocks use generated product images from the guides runner.

```sh case=known-good exit=0 says="not proof of no leakage"
mkdir data
cp samples/product/shots/front.png data/front.png
printf '%s\n' '{"schema":"saccade-split-manifest.v1","entries":[{"path":"front.png","split":"session"}]}' > data/one.json
saccade split-review data/one.json --out one-review --json
```

One file has no pairs to compare. The output still marks hash and geometric
routes used, embedding unused, and explicitly limits the clean result.

```sh case=known-bad exit=1 says="review_required"
cp data/front.png data/copy.png
printf '%s\n' '{"schema":"saccade-split-manifest.v1","entries":[{"path":"front.png","split":"train"},{"path":"copy.png","split":"test"}]}' > data/splits.json
saccade split-review data/splits.json --out leakage-review --json
```

The exact cross-split copy requires review. Read the complete pair evidence and
`pairs.csv`; a candidate is not permission to delete or approve anything.

```sh case=missing-input exit=2
saccade split-review data/not-delivered.json --out missing-review --json
```

```sh case=unavailable-dependency unavailable=embeddings exit=2 says="feature_unavailable"
saccade split-review data/one.json --embeddings --out unavailable-review --json
```

The optional route requires an embeddings build and a provisioned pinned export.
It never silently falls back or downloads during review. Injected truth produces
recall with missed pairs; no truth produces null recall. Within-split burst
groups remain separate from the cross-split candidate list.
