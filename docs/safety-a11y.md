# Photosensitivity and accessibility pre-check design

These features are **PRE-CHECKS** to catch problems early. They are **not a
certification**, do not replace platform-holder required testing (e.g. Harding
FPA) or formal compliance processes, and make no compliance claims. PASS means
that the implemented pre-check found no candidate above its configured limits;
it does not establish that a person can safely view the content.

## Published criteria and the review table

The recovery review checked the published W3C and ITU definitions. Criteria
are pinned below; a human must confirm the required edition, delivery rules
and detector assumptions before using this as an acceptance gate. The **single source of numeric
criteria** is [THRESHOLDS](../crates/saccade-core/src/safety/thresholds.rs).
Both JSON reports embed that table, including a citation and `verify` flag for
each entry. Assumptions, uncertain broadcast wording and detector heuristics
are explicitly marked. No new network dependency is needed for analysis.

Sources for human verification:

- ITU-R BT.1702-3, *Guidance for the reduction of photosensitive epileptic
  seizures caused by television*, Annex 1 flashing guidance and Attachment 1 informative pattern criteria:
  <https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.1702-3-202311-I!!PDF-E.pdf>.
- WCAG 2.2 SC 2.3.1 (*Three Flashes or Below Threshold*) and its normative
  *general flash and red flash thresholds* definition:
  <https://www.w3.org/TR/WCAG22/#three-flashes-or-below-threshold> and
  <https://www.w3.org/TR/WCAG22/#dfn-general-flash-and-red-flash-thresholds>.
- Legacy WCAG 2.0/2.1 red-flash working definition, retained in the W3C
  authorized Catalan translation, <https://www.w3.org/Translations/WCAG21-ca/>.
  The implemented `max(0,R-G-B)*320 > 20` detector is explicitly this legacy
  approximation; WCAG 2.2 uses CIE 1976 UCS difference > 0.2 instead, which
  this pre-check does not implement.
- WCAG relative luminance, SC 1.4.3, SC 1.4.6, and SC 1.4.11:
  <https://www.w3.org/TR/WCAG22/#dfn-relative-luminance>,
  <https://www.w3.org/TR/WCAG22/#contrast-minimum>,
  <https://www.w3.org/TR/WCAG22/#contrast-enhanced>,
  <https://www.w3.org/TR/WCAG22/#non-text-contrast>.
- Machado, Oliveira and Fernandes, *A Physiologically-based Model for
  Simulation of Color Vision Deficiency*, IEEE TVCG 15(6), 2009,
  <https://doi.org/10.1109/TVCG.2009.113>; severity-1.0 supplementary matrices.
- Sharma, Wu and Dalal, *The CIEDE2000 Color-Difference Formula:
  Implementation Notes, Supplementary Test Data, and Mathematical Observations*,
  Color Research & Application 30(1), 2005,
  <https://doi.org/10.1002/col.20070>.

## Safety pipeline

`saccade experiment safety FRAMES_DIR|VIDEO --fps N --display WxH@inches,distance_m
--standard itu-bt1702|wcag --out DIR [--json] [--junit FILE.xml]`.

The default standard is `itu-bt1702`. Frame collection reuses sequence's numeric
sorting, duplicate-number rejection and portable `/` names. Missing numbers,
unreadable files, empty inputs, mismatched dimensions, HDR and transparency are
errors (exit 2), rather than partial PASS results. Composite transparent pixels
on their actual display background and export displayed HDR as SDR before use.
All pixels are treated as opaque sRGB stretched to fill the declared screen;
letterboxing, display brightness, adaptation and viewing changes are not inferred.

Frame directories use `--fps`, then directory `saccade-meta.json`'s numeric `fps`,
then a documented 60 fps assumption with a warning. Videos accept mp4/mov/mkv and
use **optional external ffmpeg on PATH** without a shell or vendored decoder.
Without `--fps`, optional ffprobe reads the first video stream's average FPS.
Differing nominal/average rates are rejected as variable rate; matching rates
are a heuristic, not proof of constant timestamps. Explicit FPS imposes a
constant timebase. Export known constant-rate frames for uncertain/VFR sources.
ffmpeg output must already represent the intended SDR colour transfer; codec
colour metadata is not a display calibration.

Per pixel, sRGB is linearized at 0.04045; relative luminance is
`0.2126 R + 0.7152 G + 0.0722 B`. A WCAG general transition has absolute change
at least 0.10 and a darker endpoint below 0.80. Broadcast mode uses 20 cd/m²
change and darker below 160 cd/m², normalized by the explicitly assumed
200 cd/m² SDR peak. PNG does not establish actual absolute luminance.

The legacy WCAG 2.0/2.1 red transition requires a saturated endpoint (`R/(R+G+B) >= 0.8`) and an
excursion **greater than 20** in `max(0,R-G-B)*320`. Linear RGB follows WCAG's
relative-luminance definition. Both transitions must qualify. Broadcast red
detection uses this numeric WCAG surrogate; verify its relation to the
BT.1702 saturated-red wording with the human-reviewed edition.

One flash is a non-overlapping pair of opposing qualifying transitions at the
**same pixel**. A transition is never counted twice. Constant states between
transitions retain the pending direction; same-direction transitions update it.
The detector checks counts at every frame in the half-open window `(t-1,t]`.
More than three completed flashes in a window are the frequency risk. A mask
of pixels exceeding that rate must exceed the area criterion to FAIL. The
approach also combines asynchronously phased pixels in a common risk window;
that can be conservative relative to the standards' concurrent-area wording.
Transitions are measured between neighbouring frames; slow/complex multiframe
ramps can escape this approximation and still require formal testing.

Broadcast flashing uses greater than one quarter of screen area. Patterns use
the separate stationary/changing area limits described below.
WCAG flashing uses greater than 0.006 sr within any 10-degree field. Pixel solid
angles are computed from exact rectangular corner integrals; an integral image
searches every placement of a conservative circumscribing square. The square
is enlarged one pixel for arbitrary placement, so cone-edge findings can be
false positives. This is not an exact circular-cone solver.

The documented default display is a 55-inch 1920x1080 TV at 4 m, a living-room
setup; users must specify their actual screen and distance. Physical width and
height follow the diagonal/aspect ratio. Screen solid angle is
`4 atan(a*b / (d*sqrt(d²+a²+b²)))` for half-width `a`, half-height `b`, distance
`d`. WCAG reports the nominal whole-screen fraction `0.006 / screen_sr`, capped
at 100%; the actual verdict still applies the local field search. Different
geometry can change the small-area result.

Regular stripes/grids are tested on horizontal and vertical luminance
scanlines in 8-pixel bands, using FFT power followed by inverse FFT
(Wiener–Khinchin autocorrelation). A candidate needs more than five light/dark pairs, the broadcast general-flash
luminance difference (at least 20 cd/m² with the darker below 160 cd/m²),
and a nonzero periodic correlation peak. BT.1702-3 Annex 1 Attachment 1 uses
area greater than 40% for stationary patterns and greater than 25% for changing
patterns. Its smooth one-direction flow exemption is not inferred here;
all changing patterns conservatively use the changing criterion. Band bounds approximate pattern area and can overstate
localized stripes. Oblique patterns, fine aliasing, tile-local repetition and
profiles with too few visible pairs can be missed. This detector is useful
evidence, not a complete spatial-pattern test. WCAG SC 2.3.1 contains no spatial
pattern criterion: WCAG mode's pattern findings are supplementary BT.1702
guidance, explicitly named in the report warning.

WARN is a pre-check policy: three flashes/s over the area limit, or more than
three flashes/s at at least 80% of the area limit; patterns near the area limit
warn too. **Exactly two flashes/s passes**, satisfying the explicit validation
truth in the brief rather than adopting its optional two-Hz WARN example.
Risk intervals merge overlapping windows of each kind; heatmaps show their
union while the reported area is the peak instantaneous risk-mask area.

## Accessibility pipeline

`saccade experiment a11y IMAGES_DIR|IMAGE [--config saccade.toml] --out DIR [--json]
[--junit FILE.xml] [--suggest-regions --keys-dir DIR]`.

Machado severity-1.0 matrices act on linear RGB, clip to display gamut and
re-encode sRGB. Protanopia/deuteranopia/tritanopia and anomalous variants are
exported. At severity 1.0 anomalous trichromacy reaches the dichromacy endpoint;
the anomalous labels deliberately share those artifacts. They are not separate
physiological predictions at that endpoint. Existing `view` and compare reports
gain additive **Simulate: protan/deutan/tritan** display choices, using the same
matrices in SVG `linearRGB`. Only display filters change; underlying FLIP data
and approvals keep their original pixels.

Information-loss candidates compare horizontal/vertical neighbouring block
means at `[a11y] scale = 4` pixels (configurable 1–128). Linear means are
converted to D65 CIELAB, then CIEDE2000 with kL=kC=kH=1. Original ΔE00 >= 5
and simulated ΔE00 < 2 marks both blocks. These perceptual thresholds are
heuristics, not universal JNDs. Connected components become fractional boxes,
WARN findings and per-mode heatmaps. A finding can indicate red/green status
markers becoming indistinguishable; the detector does not infer that the
colours convey information. Tiny markers, averaging and remote non-adjacent
colours require additional human inspection. WARN does not fail CI.

Only confirmed `[[region]]` entries with `kind = "text"` or `"ui"` participate
in contrast checks. Region tables without kind are left to compare's format.
The a11y parser tolerates other config tables; use the a11y-specific declarations
with `experiment a11y --config` (compare's region parser does not accept these extra keys).
Region/glob/name/level errors and declared regions matching no input are errors.
No automatic config discovery or OCR changes the user's intended coverage.

```toml
[a11y]
scale = 4

[[region]]
name = "score text"
kind = "text"
glob = "hud*.png"
rect = [0.05, 0.05, 0.25, 0.10]
large = false
level = "AA"

[[region]]
name = "button boundary"
kind = "ui"
rect = [0.4, 0.7, 0.2, 0.1]
```

Pixels in each region are robustly clustered into two linear-RGB groups. A
5-bit/channel histogram mode seeds background, its farthest real colour seeds
foreground, then deterministic assignments/component medians refine them for
at most 12 iterations. The smaller cluster is estimated foreground. Less than
1% minority support, no second cluster, or residual linear-RGB RMS > 0.08 gives
WARN/unmeasurable, never a false PASS. Gradients, multiple UI elements and
antialias fringe can distort the estimate; define tight two-colour rectangles
and confirm the reported swatches. Transparency/HDR are rejected.

The ratio is `(Llighter+0.05)/(Ldarker+0.05)`. Normal text requires 4.5:1 AA or
7:1 AAA; declared large text requires 3:1 AA or 4.5:1 AAA. Users confirm large
means at least 18pt, or 14pt bold; image geometry cannot determine that. Essential
UI/graphical boundaries use 3:1 SC 1.4.11 regardless of text level. Exemptions,
applicability, adjacent boundaries and text semantics are human decisions. No
declared regions means contrast was not checked, explicitly reported.

AI region assist is off by default. **Only `--suggest-regions`** sends 16 coarse
4x4 crops/image through judge's existing Gemini vision chain from
`crates/saccade-core/examples/panel.toml`, with the existing provider/retry/key policy. Only
`gemini.env`'s `SACCADE_GEMINI_API_KEY` in `~/.config/saccade` or `--keys-dir`
is read. No ambient key, unrelated project .env, shell or automatic network is
used. Closed `text/ui/none/abstain` answers become coarse boxes tagged
`confirmed=false`, excluded from contrast verdicts. Users refine and explicitly
copy proposals into config; the command never edits config. Provider errors
fail the explicit request; live network quality/cost was not qualified offline.
MCP exposes deterministic checks only and deliberately has no AI opt-in.

## Outputs and verification

Both commands always write versioned JSON, `report.txt`, `index.html` and
portable relative PNG artifacts into a guarded output directory. Matching
previous pre-check artifacts are replaced; unrelated nonempty directories are
refused. An incomplete sentinel prevents a failed analysis from presenting stale
complete evidence. Reports embed the design tokens. Clicking a safety segment
shows its static frames and heatmap; a11y has simulation selection, finding
boxes, heatmaps, confirmed contrast checks and unconfirmed proposals. Reports
never automatically animate synthetic flashing sequences.

Schemas are `crates/saccade-core/schemas/saccade-safety.v1.schema.json` and
`crates/saccade-core/schemas/saccade-a11y.v1.schema.json`. `--json` prints the full report including
the disclaimer and threshold verification notes. `--junit` emits failures only
for FAIL findings; WARN stays a passing testcase with explanatory system-out.
Exit 0 = PASS/WARN, 1 = pre-check FAIL, 2 = usage/config/IO/tool error. MCP tools
`saccade_measure` operations `safety` and `a11y` preserve server root confinement for every
input, output, config and JUnit path, return full structured content and the
same disclaimer, and represent a pre-check FAIL as a successful tool result.

Seven new tests cover the six synthetic truths, sliding-window/area/red limits,
Sharma reference CIEDE2000, deutan-collapse markers, AA/AAA/large/UI contrast,
error/source-protection paths and actual CLI/schema/JUnit/MCP/video boundaries.
`showcases/photosensitivity` adds deterministic generated 4 Hz content and
measured EXPECTED stdout. Existing upscaler and lod-transition sequences are
checked separately as requested; results and verification exit codes are in
the implementation handoff.
