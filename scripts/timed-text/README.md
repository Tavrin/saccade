# Generated timed-text acceptance

Run `fixtures.py --out DIR --binary PATH` with a `text-quality` build. Pillow,
installed DejaVu Sans and FFmpeg are local prerequisites; nothing downloads.
The bounded cases assert a correct cue, +0.5-second shift, missing cue,
misspelling, extra text, cross-domain passing presentation/illustrated sequences
and a low-contrast failure. Lossless FFV1 video roundtrips for captioned footage, a two-slide presentation
recording and an illustrated cinematic sequence must preserve pixels.

`provenance.json` in each case records the generated-media licence, font hash
and licence reference, and generator/Pillow identities. `receipts.json` records
imported generated-truth acceptance separately from cached OCR. Missing OCR
features/models/runtime must explicitly skip, exit 4, and make no missing/late
finding. Actual OCR execution is recorded without promoting it to accuracy
qualification. No fixture assets are checked into the repository.
