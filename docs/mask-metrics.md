# Mask metrics

`saccade mask-metrics PREDICTED REFERENCE` scores two integer label images of the
same size. It is model-independent and uses no domain vocabulary: a class raster,
an object-ID buffer, a thresholded map and a hand-drawn region are scored the same
way. Images of different sizes are refused; nothing is resampled.

```sh
saccade mask-metrics pred.png ref.png --each-label --void ignore=id=255 --boundary-px 2 --json
```

## Labels

Single-channel 8/16-bit images use the sample value. RGB(A) images pack
`R<<16 | G<<8 | B` (alpha ignored), the same convention as object-ID sidecars.
EXR images use the red channel, which must hold non-negative integers.

## Classes

A class is a predicate over labels in the shared mask-spec grammar
(`NAME=id=1,2`, `NAME=range=1,5`, `NAME=above=0`, `NAME=mask`). Label-glob
predicates need a dictionary and are refused here. With no flag, one class
`foreground` is scored (label is not zero). `--class NAME=PREDICATE` (repeatable)
declares classes; `--each-label` makes every distinct non-void label a class
named `id=N` (at most 256).

## Conventions

| Case | Result |
| --- | --- |
| Class empty in both images | `status: absent_in_both`; IoU, Dice and boundary are `null`; excluded from means and listed in `absent_classes` |
| All classes empty in both | `summary.state: empty_empty`; every mean is `null` |
| Class only in the reference | `status: missed`; IoU 0; stays in the macro mean; listed in `missed_classes` |
| Class only in the prediction | `status: spurious`; IoU 0; stays in the macro mean; listed in `spurious_classes` |
| Void (`--void NAME=PREDICATE`) | pixels whose **reference** label matches are removed from every count in both images; a mask pixel beside a void pixel is not a boundary pixel on that side |
| Size mismatch | error |

`iou = |P∩R| / |P∪R|`, `dice = 2|P∩R| / (|P|+|R|)`. `macro_*` average over
classes present in either image, so a missing rare class lowers the mean and is
also named. `micro_iou` pools all counts. `label_agreement` is the share of
evaluated pixels whose labels are equal.

## Boundary score

A mask's boundary is its pixels with a 4-neighbour inside the image, not void and
outside the mask; the image edge is not a boundary. Boundary precision is the share
of predicted boundary pixels within `--boundary-px` (Euclidean, inclusive, 0–64) of a
reference boundary pixel; recall is the converse; the F-score is their harmonic mean.
No boundary on either side gives `null`; a boundary on one side only gives `0`.
The tolerance is a declared unit, not a tuned constant: a one-pixel-thin region
shifted by one pixel has IoU 0 but boundary F 0 at 0 px and 1 at 1 px.

Output schema: `saccade-mask-metrics.v1`. `--out DIR` also writes it to
`DIR/saccade-mask-metrics.v1.json`.
