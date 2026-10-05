# Hashing and near-duplicates

```sh
saccade hash images/ --out hashes --json
saccade hash a.png b.jpg --out hashes --json
saccade dedupe images/ --algorithm phash --threshold 6 --out duplicates --json
```

Hashing decodes each unique input once and computes aHash (8x8 mean), dHash
(9x8 horizontal gradient signs) and pHash (32x32 DCT, low 8x8 coefficients).
pHash excludes DC from the median and leaves the DC bit zero: 63 informative
bits in a 64-bit word. Triangle resizing and sRGB luma operate on pixels
composited over white. All hashes are hexadecimal strings to preserve 64-bit
values in JavaScript. Hashes are not cryptographic identities; the separate
SHA-256 binds encoded source bytes. Blockhash is omitted because these three
algorithms already cover mean, gradient and low-frequency structure.

Dedupe uses a Hamming BK-tree with exact-hash collapse and deterministic
connected-component clustering. Radius is inclusive in 0..64. Each cluster
contains indexes into `entries`; singleton clusters remain visible. Connectivity
is transitive: endpoints can be farther apart than the radius. Originals are
never deleted. Different flat images and hash collisions can cluster together;
inspect candidates before asserting they depict the same content.

Reports are `saccade-hash.v1.json` or `saccade-dedupe.v1.json`, plus HTML.
`--json` returns a bounded `saccade-general-result.v1` artifact receipt. Empty
inputs and decode failures yield exit 1 with errors recorded; invalid options,
traversal failure and unsafe outputs yield exit 2. Supported candidates are PNG,
JPEG, SVG and PDF; documents produce decode errors until their renderer is
available. New/empty report directories are required.

Capacity is 100000 images, 64 MiB of input path storage, four times the capacity
in visited directory entries, and the shared 64 MiB/16M-pixel raster bounds.
The BK-tree contains unique hashes rather than decoded pixels. Exact duplicates
have linear storage/work. Search can degrade for adversarial hash distributions;
this is an exact bounded-memory implementation, not a constant-time guarantee.

MCP `saccade_general` operations `hash` and `dedupe` accept `files` (a list of
files/directories) and `out`, with `algorithm`/`threshold` only for dedupe. Root
containment applies to every input and output.
