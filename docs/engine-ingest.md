# Engine screenshot and render-test inputs

Each `saccade ingest` adapter reads existing files only, copies PNGs into
paired directories under a new `--out`, writes a mapping artifact and flat
sidecars, then runs `compare` in `OUT/report`. It never runs Blender, Bevy,
Godot, Unreal or Unity. The normal comparison exit code applies.

| Format | Command | Documented layout |
| --- | --- | --- |
| Blender | `saccade ingest blender BUILD_TEST_OUTPUT --out OUT` | [Blender's render test reporter](https://raw.githubusercontent.com/blender/blender/main/tests/python/modules/render_report.py) writes `CATEGORY/ref/TEST.png`, `CATEGORY/TEST.png` and optional `CATEGORY/diff/TEST.diff_color.png`. The [testing handbook](https://developer.blender.org/docs/handbook/testing/render/) describes reference images and the generated report. |
| Bevy | `saccade ingest bevy REFERENCE_RUN CAPTURE_RUN --out OUT` | Bevy's [screenshot example](https://bevy.org/examples/window/screenshot/) writes `screenshot-N.png` in the working directory. The adapter pairs the same relative numbered filename across two run directories; the example does not define a baseline store. |
| Unity | `saccade ingest unity PROJECT/Assets --out OUT` | The [Graphics Test Framework guide](https://github.com/Unity-Technologies/com.unity.testframework.graphics/blob/master/Documentation~/com.unity.testframework.graphics.md) defines `ReferenceImages/` and `ActualImages/` under Assets, with `ColorSpace/Platform/GraphicsAPI` and optional reference fallback at `ColorSpace/Platform`. This is the archived package's documented format, not a claim about every modern Unity graphics-test package. |
| Unreal | `saccade ingest unreal COMPARISON_RESULT.json --out OUT` | Unreal's [FImageComparisonResult](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Developer/ScreenShotComparisonTools/FImageComparisonResult) exposes `ReportApprovedFilePath`, `ReportIncomingFilePath`, `ReportComparisonFilePath`, `ScreenshotPath`, platform and RHI. The adapter accepts one such JSON object or an array of them, resolving paths relative to the JSON file. The [Screenshot Comparison Tool](https://dev.epicgames.com/documentation/unreal-engine/screenshot-comparison-tool-in-unreal-engine) stores reports under `Saved/Automation/Comparisons`. |

For Unreal, export the comparison result objects as JSON if your automation
pipeline does not already retain them. Missing images or a missing reference
remain visible as incomplete pairs. Source diff images are copied for audit;
saccade computes its own FLIP heatmap and does not treat a producer diff as a
measurement. All tests use small synthetic PNGs.

Godot documents [Viewport image capture](https://docs.godotengine.org/en/stable/tutorials/rendering/viewports.html),
but the Godot project does not specify a standard screenshot-test directory
or comparison result layout. No `ingest godot` path is provided on this
evidence. A project can use ordinary `compare BASELINE_DIR CAPTURE_DIR` with
its own capture convention.
