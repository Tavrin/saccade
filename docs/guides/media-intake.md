# Guide: media intake and near-duplicates

Use this when a folder of images arrives (product photos, scans, user uploads,
camera frames) and you need to know which are near-duplicates and which cannot
be read, without deleting or changing anything. The cross-domain example is a
product photo shoot: a front shot, a re-exported copy of it, and a side shot.

Run the tested blocks with `scripts/test-guides.py`.

## Known good: find the near-duplicate

```sh case=known-good exit=0 says="\"duplicate_clusters\":1"
saccade dedupe samples/product/shots --algorithm phash --threshold 6 --out dedupe-ok --json
```

`--threshold` is a perceptual-hash distance in bits, 0 to 64: 0 means identical
hashes, larger is looser. The receipt counts clusters; the report lists their members. Clusters are transitive (members need not all be within
the threshold of each other), singletons stay visible, and originals are never
deleted or modified. Treat a cluster as a candidate to look at; flat or similar
images can collide.

```sh case=known-good exit=0
saccade hash samples/product/shots --out hashes-ok --json
saccade assess samples/product/shots/front.png --out assess-front
```

`assess` reports content-dependent indicators (blur, noise, blockiness,
banding, clipping); there is no universal pass level.

## Known bad: an unreadable file

```sh case=known-bad exit=1 says="\"errors\":1"
saccade dedupe samples/product/corrupt --out dedupe-bad --json
```

A file that cannot be decoded is recorded as an error (`"errors":1` in the
receipt, the file named in `dedupe-bad/saccade-dedupe.v1.json`) and the command
exits 1. It is never silently dropped from the count.

## Missing input: the folder is not there

```sh case=missing-input exit=2
saccade dedupe samples/product/not-delivered --out dedupe-missing --json
```

## Unavailable dependency: finding the product by a phrase

```sh case=unavailable-dependency unavailable=local-models exit=2 says="local-models"
saccade locate samples/product/shots/front.png "red product" --overlay overlay.png
```

Locating a phrase needs a `local-models` build, a pulled detector model and the
runtime. Without them the command exits 2 and names the missing piece;
`saccade doctor` lists the models and their fix commands.

## Next

Re-rank or review the clusters with a person before acting:
`saccade review plan` prepares the closed question without calling a provider.
See [hashing](../hashing.md) and [media](../media.md).
