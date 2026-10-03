# Photosensitivity pre-check fixture

PRE-CHECK only. Not a certification; does not replace platform-holder required
testing (e.g. Harding FPA) or formal compliance processes. No compliance claims.

`python3 scripts/gen-photosensitivity.py` generates 64 numbered 64x32 PNG frames
with a fixed 32 fps timebase, alternating full-screen black and white at 4 Hz.
The generator uses only the Python standard library. The committed sequence is
small and deterministic; `saccade-meta.json` supplies FPS without a CLI override.

From this directory:

```sh
saccade experiment safety frames --fps 32 --standard itu-bt1702 --out .work/photosensitivity-report
```

Expected pre-check verdict: **FAIL**, exit **1**, general-flash segment at 4
flashes/s over 100% of the screen. EXPECTED.txt is the measured stdout. The HTML
timeline uses static frame previews; it does not play the flashing content.
The existing showcase runner discovers this case through commands.json.

Validation requires a `--features prechecks` build. All frames are procedural.
