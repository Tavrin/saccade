# Guide: delivery tuning (how much compression is too much)

Use this when images are re-encoded for delivery (a photo CDN, a thumbnail
pipeline, a document export, a game texture) and you need evidence that the
shipped version still looks like the source. The cross-domain example is a
product photo delivered at lower quality.

Two routes: `compare` against the source works on every build and answers "does
this shipped file stay within a limit?"; `imgtune` searches encodings against a
perceptual target and needs the `products` feature (`saccade doctor` shows
whether you have it). Run the tested blocks with `scripts/test-guides.py`.

## Known good: the shipped image stays within the limit

```sh case=known-good exit=0
saccade compare samples/delivery/source.png samples/delivery/current.png --out ship-ok --metric p95 --threshold 0.05
```

`--threshold` is a FLIP score in 0 to 1 (0 = identical); the limit is your
policy, not a universal constant. State the viewing condition too: `--ppd` is
pixels per degree of visual angle (default 67).

## Known bad: the limit is exceeded

```sh case=known-bad exit=1 says="1 fail"
saccade compare samples/delivery/source.png samples/product/shots/side.png --out ship-bad --metric p95 --threshold 0.05
```

(That second file is a different picture on purpose: the same command shape
fails whenever the shipped file is further from the source than the limit.)

## Known good, search: the cheapest encoding that meets the target

```sh case=known-good requires=products exit=0
cat > tuning.json <<'JSON'
{
  "schema": "saccade-imgtune.v1",
  "images": [{"id": "photo-1", "source": "samples/delivery/source.png", "current": "samples/delivery/current.png"}],
  "widths": [64, 128], "formats": ["jpeg"], "qualities": [60, 80, 95],
  "target_score": 60, "butteraugli_ceiling": null,
  "adapter": {"kind": "local"}
}
JSON
saccade imgtune search tuning.json --out tuning-report.json --json
```

`target_score` is an SSIMULACRA2 score (higher is closer to the source; 100 is
identical). `qualities` are JPEG quality values 1 to 100. The report lists every
candidate with its bytes; the lowest-bytes qualifying setting is selected among
the declared grid only.

## Missing input: the shipped file is not there

```sh case=missing-input exit=2
saccade compare samples/delivery/source.png samples/delivery/not-shipped.png --out ship-missing
```

## Unavailable dependency: the encoder search

```sh case=unavailable-dependency unavailable=products exit=2 says="products"
saccade imgtune search tuning.json --out tuning-report.json --json
```

On a `products` build, `imgtune audit` records what an image URL actually serves
(`Accept` header, `Content-Type`, sniffed format, bytes) and `imgtune search`
measures a grid of widths, formats and qualities against a target score. A grid
with missing cells cannot select a winner. See [image delivery tuning](../imgtune.md);
network access happens only for URLs you list.
