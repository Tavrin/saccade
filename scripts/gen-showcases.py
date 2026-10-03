#!/usr/bin/env python3
"""Generate all publishable showcases with Pillow, numpy and fixed seeds.

Run from any directory. No source images, system fonts, clock, network, or GPU.
Only writes showcases/ next to this script; EXPECTED.txt is never overwritten.
PNG/JPEG bytes are reproducible with the same Pillow/numpy versions.
"""

import io
import json
import shlex
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parents[1]
SHOWCASES = ROOT / "showcases"
FONT = ImageFont.load_default()
RESAMPLE = Image.Resampling


def save(image, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(image, np.ndarray):
        image = Image.fromarray(image)
    image.save(path, optimize=True)


def rgb(values):
    return Image.fromarray(np.clip(np.rint(values), 0, 255).astype(np.uint8))


def jpeg(image, quality):
    stream = io.BytesIO()
    image.save(stream, format="JPEG", quality=quality, subsampling=2)
    stream.seek(0)
    return Image.open(stream).convert("RGB")


def tone(image, gains):
    a = np.asarray(image, dtype=np.float64) / 255
    linear = np.where(a <= .04045, a / 12.92, ((a + .055) / 1.055) ** 2.4)
    linear = np.clip(linear * gains, 0, 1)
    return rgb(255 * np.where(linear <= .0031308, linear * 12.92,
                              1.055 * linear ** (1 / 2.4) - .055))


def compare(config=True, mode="compare", **extra):
    args = [mode, "baseline", "capture"]
    if config:
        args += ["--config", "saccade.toml"]
    return dict(name=mode, args=args, out=mode, exit=1, **extra)


def rank(labels):
    return dict(name="rank", args=["experiment", "rank", "baseline"] +
                ["candidates/" + label for label in labels] +
                ["--labels", ",".join(labels), "--metric", "mean", "--threshold", "0.001"],
                out="rank", exit=0)


def case(name, description, expected, commands, config=None):
    path = SHOWCASES / name
    path.mkdir(parents=True, exist_ok=True)
    if config:
        (path / "saccade.toml").write_text(config.strip() + "\n")
    (path / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
    lines = ["# " + name, "", description, "", "Generate from the repository root:",
             "", "```sh", "python3 scripts/gen-showcases.py", "```", "",
             "Run from the repository root with `saccade` on PATH. Reports go to a",
             "sibling directory outside the repository; use a fresh directory or an",
             "existing saccade report directory.", "", "```sh",
             "REPORTS=../saccade-showcase-reports", "(", "  cd showcases/" + name]
    for command in commands:
        args = [a.replace("@REPORTS@", "$REPORTS/" + name) for a in command["args"]]
        # REPORTS was relative to the repository root, two levels above this cwd.
        text = "saccade " + " ".join('"../../' + a + '"' if "$REPORTS" in a
                                      else shlex.quote(a) for a in args)
        if command.get("out"):
            text += ' --out "../../$REPORTS/' + name + '/' + command["out"] + '"'
        lines += ["  " + text]
    lines += [")", "```", "", "Expected: " + expected, "",
              "Exit 1 is the intentional regression verdict; exit 0 is expected for",
              "inspect evidence and inspect. `EXPECTED.txt` contains the actual CLI",
              "stdout captured by `scripts/run-showcases.sh`, including diagnostics.", "",
              "All images are procedural, use fixed seeds and Pillow's bundled default",
              "font, and contain no third-party source imagery. The timestamp variation",
              "is simulated deterministically, rather than read from the system clock.", ""]
    (path / "README.md").write_text("\n".join(lines))
    return path


def dashboard(shift=0, label="Export CSV", token=(58, 110, 218), stamp="08:00:00"):
    image = Image.new("RGB", (480, 300), (239, 242, 248))
    d = ImageDraw.Draw(image)
    d.rectangle((0, 0, 479, 53), fill=token)
    d.text((18, 18), "PULSE / Analytics", font=FONT, fill="white")
    d.text((340, 21), "Updated " + stamp, font=FONT, fill="white")
    for x, title, value in [(18, "Visitors", "12,840"), (173, "Revenue", "$24,680"),
                            (328, "Conversion", "4.8%")]:
        d.rounded_rectangle((x, 70, x + 133, 125), radius=5, fill="white")
        d.text((x + 10, 80), title, font=FONT, fill=(72, 82, 102))
        d.text((x + 10, 103), value, font=FONT, fill=(20, 35, 64))
    d.rounded_rectangle((18, 141, 461, 244), radius=5, fill="white")
    for y in (164, 189, 214, 237):
        d.line((32, y, 444, y), fill=(224, 230, 239))
    points = [(32 + i * 34, 214 - int(28 * np.sin(i * .55) + i * 2)) for i in range(13)]
    d.line(points, fill=token, width=2)
    d.rounded_rectangle((335 + shift, 259, 459 + shift, 285), radius=4, fill=token)
    d.text((347 + shift, 266), label, font=FONT, fill="white")
    d.text((20, 266), "Last 12 months", font=FONT, fill=(72, 82, 102))
    return image


def webapp():
    p = case("webapp-ui", "A drawn dashboard: three regressions, a pixel-identical page, "
             "and a timestamp-only change. The header region gates its p95; the timestamp "
             "mask has a generous margin because FLIP filters spread error beyond glyphs.",
             "3 fail, 2 pass; button-shift, label and token fail, identical and timestamp-only pass.",
             [compare()], '''threshold = 0.02
metric = "max"
[[region]]
name = "header"
rect = [0.0, 0.0, 1.0, 0.18]
metric = "p95"
threshold = 0.02
[[mask]]
rect = [0.67, 0.015, 0.33, 0.15]
''')
    variants = {"button-shift": dashboard(shift=1, stamp="08:00:01"),
                "label": dashboard(label="Delete data", stamp="08:00:01"),
                "token": dashboard(token=(134, 66, 183), stamp="08:00:01"),
                "identical": dashboard(), "timestamp-only": dashboard(stamp="08:00:01")}
    for name, image in variants.items():
        save(dashboard(), p / "baseline" / (name + ".png"))
        save(image, p / "capture" / (name + ".png"))


def cover():
    p = case("cover-art", "Generated gradient, geometric cover and title. Warm tone, "
             "an 8% crop zoom and decoded JPEG quality 40 demonstrate diagnostic classes. "
             "Recompression is stored as PNG after JPEG decoding so names pair exactly.",
             "3 fail: warmer is global_tone; crop and jpeg-q40 are local_structure "
             "with structural descriptions. These are measured diagnostics, not hardcoded labels.",
             [compare()], 'threshold = 0.001')
    y, x = np.mgrid[:288, :224]
    image = rgb(np.stack((35 + .22 * x + .10 * y, 45 + .18 * y,
                          110 + .20 * x - .1 * y), axis=-1))
    d = ImageDraw.Draw(image)
    d.ellipse((31, 54, 194, 217), fill=(216, 174, 96))
    d.polygon([(0, 232), (105, 120), (224, 252), (224, 288), (0, 288)], fill=(46, 84, 104))
    for i in range(9):
        d.line((0, 190 + i * 9, 223, 110 + i * 11), fill=(112, 132, 145))
    d.text((18, 19), "ORBIT / Volume 03", font=FONT, fill=(238, 231, 212))
    d.text((18, 261), "A procedural anthology", font=FONT, fill=(238, 231, 212))
    variants = {"warmer": tone(image, (1.24, 1.01, .76)),
                "crop": image.crop((9, 12, 215, 276)).resize(image.size, RESAMPLE.LANCZOS),
                "jpeg-q40": jpeg(image, 40)}
    for name, variant in variants.items():
        save(image, p / "baseline" / (name + ".png"))
        save(variant, p / "capture" / (name + ".png"))


def blocks(image, high):
    """4x4 RGB line palette: 4 entries with 565 endpoints, or 8 with 888."""
    a = np.asarray(image, dtype=np.float64)
    out = np.empty_like(a)
    levels = np.array([31, 63, 31]) if not high else np.array([255] * 3)
    count = 8 if high else 4
    for y in range(0, a.shape[0], 4):
        for x in range(0, a.shape[1], 4):
            b = a[y:y + 4, x:x + 4]
            lo = np.rint(b.min(axis=(0, 1)) / 255 * levels) / levels * 255
            hi = np.rint(b.max(axis=(0, 1)) / 255 * levels) / levels * 255
            palette = lo + np.linspace(0, 1, count)[:, None] * (hi - lo)
            indices = ((b[:, :, None] - palette) ** 2).sum(axis=-1).argmin(axis=-1)
            out[y:y + 4, x:x + 4] = palette[indices]
    return rgb(out)


def texture():
    labels = ["block-low", "block-high", "jpeg-q40", "jpeg-q85", "palette-256"]
    p = case("texture-compression", "Seeded brick/noise texture with normal-map-like blue "
             "detail. Block candidates use 4x4 line palettes (4 colours/RGB565 or 8/RGB888): "
             "these approximate two block-compression qualities and are simulations. "
             "Real BC/ASTC encoders, format bitrates and encoder speed are out of scope.",
             "rank exits 0 because all lossy candidates exceed mean 0.001; lower FLIP ranks first.",
             [rank(labels)])
    y, x = np.mgrid[:192, :192]
    rng = np.random.default_rng(1701)
    mortar = (y % 24 < 2) | ((x + (y // 24 % 2) * 20) % 40 < 2)
    noise = rng.normal(0, 7, (192, 192))
    a = np.stack((135 + 24 * np.sin(x * .7) + noise,
                  80 + 20 * np.cos(y * .8) + noise, 120 + 30 * np.sin((x + y) * .4) + noise), -1)
    a[mortar] = (45, 47, 58)
    image = rgb(a)
    save(image, p / "baseline/texture.png")
    images = [blocks(image, False), blocks(image, True), jpeg(image, 40), jpeg(image, 85),
              image.quantize(colors=256, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)]
    for label, variant in zip(labels, images):
        save(variant, p / "candidates" / label / "texture.png")


def scene(pan=0):
    y, x = np.mgrid[:192, :256]
    checker = ((x + pan) // 9 + (y - 95) // 7) % 2
    a = np.where((y >= 96)[..., None], (65 + 80 * checker)[..., None],
                 np.broadcast_to(np.array([64, 85, 113]), (192, 256, 3))).astype(np.uint8)
    image = Image.fromarray(np.broadcast_to(a, (192, 256, 3)).copy())
    d = ImageDraw.Draw(image)
    for i in range(8):
        dx = int(i * 25 - pan)
        d.line((dx, 90, dx + 23, 15), fill=(222, 185, 105), width=1)
    d.rectangle((55 - pan, 35, 181 - pan, 86), fill=(32, 47, 66), outline=(194, 214, 228))
    d.text((65 - pan, 44), "NATIVE 256 / Detail", font=FONT, fill=(233, 238, 245))
    d.text((65 - pan, 65), "thin lines + type", font=FONT, fill=(233, 238, 245))
    return image


def upscaler():
    labels = ["nearest", "bilinear", "bicubic", "lanczos", "sharpened-bicubic"]
    commands = [rank(labels), dict(name="sequence", args=["experiment", "sequence", "sequence/baseline",
                "sequence/capture", "--pattern", "frame_*.png", "--threshold", "0.005"],
                out="sequence", exit=1)]
    p = case("upscaler", "A native procedural scene with thin lines, text and a checkerboard "
             "floor, compared with half-resolution reconstructions. Bicubic and Lanczos "
             "are separate filters. The 12-frame camera pan uses a Lanczos reconstruction "
             "with alternating floor highlights to simulate temporal shimmer. Temporal "
             "instability is informational and is not a motion-compensated metric.",
             "rank exits 0 and sequence exits 1; the shimmer sequence adds positive temporal instability.",
             commands)
    image = scene()
    half = image.resize((128, 96), RESAMPLE.LANCZOS)
    save(image, p / "baseline/scene.png")
    methods = [RESAMPLE.NEAREST, RESAMPLE.BILINEAR, RESAMPLE.BICUBIC, RESAMPLE.LANCZOS,
               RESAMPLE.BICUBIC]
    for label, method in zip(labels, methods):
        variant = half.resize(image.size, method)
        if label == "sharpened-bicubic":
            variant = variant.filter(ImageFilter.UnsharpMask(radius=1, percent=140, threshold=0))
        save(variant, p / "candidates" / label / "scene.png")
    for n in range(12):
        base = scene(n)
        cap = base.resize((128, 96), RESAMPLE.LANCZOS).resize(base.size, RESAMPLE.LANCZOS)
        a = np.asarray(cap, dtype=np.float64).copy()
        a[108:, :, :] += 32 if n % 2 else -32
        save(base, p / "sequence/baseline" / f"frame_{n:02}.png")
        save(rgb(a), p / "sequence/capture" / f"frame_{n:02}.png")


def object_frame(n, detailed):
    image = Image.new("RGB", (192, 160), (28, 39, 57))
    d = ImageDraw.Draw(image)
    cx, cy = 92 + n, 78
    d.ellipse((cx - 54, cy - 54, cx + 54, cy + 54), fill=(114, 160, 180))
    d.ellipse((cx - 24, cy - 24, cx + 24, cy + 24), fill=(28, 39, 57))
    if detailed:
        for angle in np.linspace(0, 2 * np.pi, 48, endpoint=False):
            d.line((cx + 27 * np.cos(angle), cy + 27 * np.sin(angle),
                    cx + 51 * np.cos(angle), cy + 51 * np.sin(angle)), fill=(30, 68, 93), width=2)
    d.text((10, 143), "LOD / moving gear", font=FONT, fill=(195, 215, 228))
    return image


def lod():
    p = case("lod-transition", "A smoothly moving procedural gear. At frame 6 (zero-based), "
             "the capture loses its fine spokes for one frame, simulating LOD threshold "
             "jitter; the reference preserves detail. This creates an isolated error "
             "spike and adds temporal variation at the pop and recovery.",
             "sequence exits 1 with 1 frame over threshold; worst frame is frame 6 "
             "and temporal instability is positive.",
             [dict(name="sequence", args=["experiment", "sequence", "baseline", "capture", "--pattern",
                                         "frame_*.png", "--threshold", "0.005"],
                   out="sequence", exit=1)])
    for n in range(12):
        fine, coarse = object_frame(n, True), object_frame(n, False)
        save(fine, p / "baseline" / f"frame_{n:02}.png")
        save(coarse if n == 6 else fine, p / "capture" / f"frame_{n:02}.png")


def model_image(seed, moved=False):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[:144, :192]
    colours = rng.integers(25, 120, 3)
    a = colours + (x / 10 + y / 12)[..., None]
    image = rgb(a)
    d = ImageDraw.Draw(image)
    for i in range(5):
        cx = int(rng.integers(25, 165)) + (22 if moved and i == 2 else 0)
        cy = int(rng.integers(30, 115))
        radius = int(rng.integers(12, 26))
        colour = tuple(int(v) for v in rng.integers(85, 210, 3))
        d.ellipse((cx - radius, cy - radius, cx + radius, cy + radius), fill=colour)
    d.text((8, 8), "Procedural seed " + str(seed), font=FONT, fill=(231, 231, 224))
    return image


def ml():
    commands = [compare(), dict(name="explain", args=["inspect", "evidence",
                "@REPORTS@/compare/saccade-report.v1.json"], out="explain", exit=0),
                dict(name="export", args=["inspect", "export", "@REPORTS@/compare/saccade-report.v1.json", "--format", "markdown", "--out", "@REPORTS@/summary.md"], exit=0)]
    p = case("ml-image-model", "Six fixed procedural seeds stand in for shared prompts "
             "across checkpoints A and B. B adds a colour cast to seeds 101 and 104, moves "
             "one shape for seed 105 and preserves the other three outputs. Compare, "
             "explain strips and a portable Markdown export provides judge-ready evidence; "
             "no model or external judge is invoked.",
             "compare exits 1 with 3 fail and 3 pass; inspect evidence and inspect export exit 0.",
             commands, 'threshold = 0.001')
    for seed in range(100, 106):
        base = model_image(seed)
        cap = tone(base, (1.28, .94, .77)) if seed in (101, 104) else model_image(seed, seed == 105)
        save(base, p / "baseline" / f"seed_{seed}.png")
        save(cap, p / "capture" / f"seed_{seed}.png")


def gbuffer():
    p = case("render-gbuffer", "A lit image plus native 16-bit depth, RGB signed normals "
             "and RG signed motion. The capture changes a local shade and reduces depth "
             "to 32 levels, still stored in a 16-bit PNG. Normal and motion stay identical. "
             "Numerical buffers bypass FLIP and retain their own units.",
             "2 fail (lit and depth), 2 pass (normal and motion); depth error uses normalised units.",
             [compare()], '''threshold = 0.001
[[buffer]]
glob = "depth.png"
kind = "depth"
encoding = "linear01"
threshold = 0.002
[[buffer]]
glob = "normal.png"
kind = "normal"
encoding = "rgb_snorm"
threshold = 1.0
[[buffer]]
glob = "motion.png"
kind = "motion"
encoding = "rg_snorm"
scale = 32.0
threshold = 0.5
''')
    y, x = np.mgrid[:96, :128]
    depth = .1 + .8 * (x + y) / 222
    save(np.rint(depth * 65535).astype(np.uint16), p / "baseline/depth.png")
    save(np.rint(np.rint(depth * 31) / 31 * 65535).astype(np.uint16), p / "capture/depth.png")
    nx, ny = (x - 64) / 160, (y - 48) / 160
    normal = rgb((np.stack((nx, ny, np.sqrt(1 - nx * nx - ny * ny)), -1) + 1) * 127.5)
    motion = rgb(np.stack((128 + x / 16, 128 + y / 16, np.zeros_like(x)), -1))
    for name, image in [("normal", normal), ("motion", motion)]:
        for side in ("baseline", "capture"):
            save(image, p / side / (name + ".png"))
    base = model_image(202)
    a = np.asarray(base, dtype=np.float64).copy()
    a[55:105, 60:130] *= .65
    save(base, p / "baseline/lit.png")
    save(rgb(a), p / "capture/lit.png")


def perf():
    p = case("perf-identity", "A synthetic optimisation proof: two images are bit-identical, "
             "one image changes a pixel. Flat sidecars pair timing.gpu_ms 4.85 to 2.71. "
             "These timings illustrate pairing and are not benchmark measurements.",
             "identity exits 1: 1 fail, 2 pass, with bit-identical and gpu_ms timing text.",
             [compare(mode="identity")], 'meta_name = "saccade-meta.json"')
    for n in range(3):
        base = model_image(300 + n)
        a = np.asarray(base).copy()
        if n == 2:
            a[72, 96] = (255, 0, 255)
        save(base, p / "baseline" / f"view_{n}.png")
        save(a, p / "capture" / f"view_{n}.png")
    for side, ms in [("baseline", 4.85), ("capture", 2.71)]:
        (p / side / "saccade-meta.json").write_text(json.dumps({
            "renderer.mode": "lit", "resolution.output": "192x144", "timing.gpu_ms": ms
        }, indent=2) + "\n")


def main():
    for generate in (webapp, cover, texture, upscaler, lod, ml, gbuffer, perf):
        generate()
    (SHOWCASES / "README.md").write_text(
        "# Reproducible showcases\n\n"
        "Run `python3 scripts/gen-showcases.py` to generate the eight cases using "
        "Python 3, Pillow and numpy. No downloaded imagery, GPU or model is used.\n\n"
        "With the release `saccade` on PATH, `scripts/run-showcases.sh` regenerates "
        "the data, checks each command's exit verdict, and reproduces every case's "
        "EXPECTED.txt byte for byte. Reports are kept outside this repository. "
        "A changed expectation is a failure; the runner never blesses new output.\n\n"
        "Each case README contains portable commands, a description and expected "
        "verdict. EXPECTED.txt holds measured stdout, not predicted metric values. "
        "The generator preserves those files. commands.json is the same command "
        "list used by the runner (`@REPORTS@` denotes that case's report directory).\n\n"
        "Generated bytes are stable for a fixed Pillow/numpy version (initial "
        "generation: Pillow 10.2.0, numpy 2.5.2). All seeds and simulated timestamps "
        "are fixed. Case data stays below 1.5 MB and the total below 12 MB.\n")
    sizes = {p.name: sum(f.stat().st_size for f in p.rglob("*") if f.is_file())
             for p in SHOWCASES.iterdir() if p.is_dir()}
    for name, size in sorted(sizes.items()):
        print(f"{name}: {size:,} bytes")
        if size > 1_500_000:
            raise SystemExit(f"{name} exceeds 1.5 MB")
    total = sum(f.stat().st_size for f in SHOWCASES.rglob("*") if f.is_file())
    if total > 12_000_000:
        raise SystemExit("showcases exceeds 12 MB")
    print(f"Total: {total:,} bytes")


if __name__ == "__main__":
    main()
