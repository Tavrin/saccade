# Box interchange

`saccade boxes` converts bounding-box annotations between a canonical document and
COCO or YOLO files, and re-expresses them for a cropped or resized image. It does
not detect anything and needs no model.

The canonical `saccade-boxes.v1` document describes **one image** and states its
convention instead of implying it: `coordinates` must be `unit: pixel`,
`origin: top_left`, `form: xywh`, in the pixels of the named image. Any other value
is refused. `classes` is ordered; that order defines COCO category numbers (from 1)
and YOLO class indices (from 0). A class may be declared and unused, which is how an
image with no annotations keeps its class list.

```sh
saccade boxes export doc.json --format coco --out out/       # annotations.coco.json
saccade boxes export doc.json --format yolo --out out/       # <image stem>.txt + classes.txt
saccade boxes import out/annotations.coco.json --format coco --out back/
saccade boxes import out/scene.txt --format yolo --width 640 --height 480 \
  --image-file scene.png --classes out/classes.txt --out back/
saccade boxes transform doc.json --crop 40,10,100,60 --out cropped/
saccade boxes transform doc.json --resize 320,240 --out resized/
```

## Rules

- A box outside the image is an error. `--clip` (export) intersects boxes with the
  image and drops boxes with no area left; both counts appear in `adjustments`.
- COCO import needs exactly one image and a four-number `bbox` on every annotation;
  segmentation-only annotations are refused. YOLO carries no image size, file name
  or class names, so import requires `--width`, `--height`, `--image-file` and
  `--classes`. YOLO values outside the normalised image are refused.
- `transform --crop X,Y,W,H` moves boxes into the crop's coordinates, clips boxes
  crossing its edge and drops boxes outside it (both counted). `--resize W,H`
  scales each axis independently. The result records the derivation and the source
  hash (`derived_from_sha256`) and carries no hash of its own, since the new image
  is not the hashed one.
- A score, when present, is kept in both formats (COCO `score`, YOLO sixth column).
- Every command writes `saccade-boxes-result.v1.json` with the file hashes.

Geospatial formats and mask annotations are not covered.
