# Public source-fidelity labels

Twenty manually selected known-truth comparisons from the procedural showcases:
five texture encodings, three UI regressions, three cover changes, three ML
checkpoint changes, one LOD pop and five upscaler losses. The bounded criterion
is to preserve all source content, placement, text, colour and fine detail.
The source is the expected answer in every stable pair; presentation is reversed
during benchmarking. This measures source fidelity under that rubric, not
aesthetic quality. It is a smoke-test fixture, not collected production human
calibration or a universal model ranking.

Every item includes both encoded presentation states, the two public image
paths/hashes, intent, crop rectangles, an evidence identity and provenance.
Two low-error texture pairs have no automatic hotspot. Their explicit central
detail crops are marked manual, with unmeasured hotspot statistics set to zero.
No credentials, private captures or temporary report copies are included.

Regenerate after running the showcases:

```sh
SACCADE_SHOWCASE_REPORTS=.work/saccade-review-showcases scripts/run-showcases.sh
cargo run -p saccade-core --example build_review_labels -- .work/saccade-review-showcases examples/labels/showcase-truths.json
```

The benchmark verifies all image hashes before any provider call. Live checks
have separate actual HTTP caps; tests never call live providers.
