# Guide: derivative sheets

Use generated source images and declare boxes in source pixels. See
[derivative sheets](../derivative-sheets.md) for the contract and generated proofs.
These cases distinguish successful review completion from safe/legible findings.

```sh case=known-good exit=4 says="not_established"
mkdir data
cp samples/product/shots/front.png data/source.png
printf '%s\n' '{"schema":"saccade-derivatives.v1","derivatives":[{"id":"thumbnail","display_size":[64,48]}]}' > data/sizes.json
saccade derivative-sheet data/source.png data/sizes.json --out review --json
```

An unannotated source completes review but never certifies subject or text safety.

```sh case=known-bad exit=1 says="unsafe"
printf '%s\n' '{"schema":"saccade-derivatives.v1","subjects":[{"label":"protected","rect_px":[0,0,4,4]}],"derivatives":[{"id":"cuts subject","display_size":[16,16],"crop_px":[2,0,16,16]}]}' > data/cut.json
saccade derivative-sheet data/source.png data/cut.json --out cut-review --json
```

```sh case=missing-input exit=2
saccade derivative-sheet data/not-delivered.png data/sizes.json --out missing-review --json
```

```sh case=unavailable-dependency exit=4 says="unavailable"
saccade derivative-sheet data/source.png data/sizes.json --detector unprovisioned-detector --out no-model-review --json
```

Missing face models remain visible in the sheet. Declare subject boxes for a
scoped geometry check; inspect original text beside the final display sizes.
