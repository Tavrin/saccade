#!/usr/bin/env python3
"""Generate examples/baseline and examples/capture, deterministically.

Each image is a small analytic render: gradient sky, ground plane, a shaded
sphere with a specular highlight and a soft shadow. The pairs cover every
saccade status:

  sphere_identical.png     identical            -> pass
  sphere_subtle.png        +-1 dither noise     -> pass (below threshold)
  sphere_shadow.png        shadow moved, tinted -> fail (regression)
  sphere_new.png           capture only         -> new
  sphere_missing.png       baseline only        -> missing

Requires Python 3, Pillow and numpy. Output is byte-stable for a given
Pillow version; no timestamps or random state outside fixed seeds.

Usage: python3 scripts/gen-examples.py [OUT_DIR]   (default: examples)
"""

import sys
from pathlib import Path

import numpy as np
from PIL import Image

SIZE = 256
HORIZON = 132


def _smoothstep(edge0, edge1, x):
    t = np.clip((x - edge0) / (edge1 - edge0), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def render(
    sphere_center=(128.0, 112.0),
    radius=58.0,
    albedo=(0.80, 0.25, 0.20),
    light=(-0.55, -0.65, 0.52),
    light_color=(1.0, 0.97, 0.90),
    shadow_shift=(0.0, 0.0),
    sky_top=(0.18, 0.38, 0.78),
    sky_bottom=(0.78, 0.88, 0.97),
    ground=(0.42, 0.46, 0.40),
):
    """Return a float RGB array (SIZE, SIZE, 3) in [0, 1], sRGB-encoded."""
    ys, xs = np.mgrid[0:SIZE, 0:SIZE].astype(np.float64)
    img = np.zeros((SIZE, SIZE, 3))

    # Sky gradient above the horizon.
    t = np.clip(ys / HORIZON, 0.0, 1.0)[..., None]
    sky = np.array(sky_top) * (1 - t) + np.array(sky_bottom) * t
    # Ground with a gentle distance falloff and a faint checker pattern.
    depth = np.clip((ys - HORIZON) / (SIZE - HORIZON), 0.0, 1.0)
    checker = (((xs // 32).astype(int) + (ys // 16).astype(int)) % 2) * 0.035
    ground_shade = (0.80 + 0.30 * depth + checker)[..., None]
    ground_img = np.array(ground) * ground_shade
    is_ground = (ys >= HORIZON)[..., None]
    img = np.where(is_ground, ground_img, sky)

    lx, ly, lz = light
    ln = np.sqrt(lx * lx + ly * ly + lz * lz)
    lx, ly, lz = lx / ln, ly / ln, lz / ln

    # Soft elliptical shadow on the ground, cast away from the light.
    cx, cy = sphere_center
    base_y = cy + radius * 0.92
    sx = cx - lx * radius * 1.1 + shadow_shift[0]
    sy = base_y + 8.0 + shadow_shift[1]
    d = np.sqrt(((xs - sx) / (radius * 1.15)) ** 2 + ((ys - sy) / (radius * 0.28)) ** 2)
    shadow = (1.0 - 0.55 * (1.0 - _smoothstep(0.55, 1.15, d)))[..., None]
    img = np.where(is_ground, img * shadow, img)

    # Sphere: Lambert diffuse + ambient + Blinn-Phong specular.
    dx = xs - cx
    dy = ys - cy
    r2 = dx * dx + dy * dy
    nz = np.sqrt(np.clip(1.0 - r2 / (radius * radius), 0.0, 1.0))
    nx = dx / radius
    ny = dy / radius
    ndl = np.clip(nx * lx + ny * ly + nz * lz, 0.0, 1.0)
    hx, hy, hz = lx, ly, lz + 1.0
    hn = np.sqrt(hx * hx + hy * hy + hz * hz)
    ndh = np.clip((nx * hx + ny * hy + nz * hz) / hn, 0.0, 1.0)
    spec = ndh**48 * 0.9
    sky_ambient = 0.18 + 0.12 * (1.0 - ny) * 0.5
    lc = np.array(light_color)
    sphere = (
        np.array(albedo) * (sky_ambient[..., None] + ndl[..., None] * lc)
        + spec[..., None] * lc
    )
    # One-pixel anti-aliased edge.
    edge = np.clip(radius - np.sqrt(r2) + 0.5, 0.0, 1.0)[..., None]
    img = img * (1 - edge) + np.clip(sphere, 0, 1) * edge

    return np.clip(img, 0.0, 1.0)


def to_srgb8(linear_ish):
    return (np.clip(linear_ish, 0.0, 1.0) ** (1 / 1.1) * 255.0 + 0.5).astype(np.uint8)


def add_dither(arr, seed, density=0.05):
    """Flip a small random fraction of channel values by +-1."""
    rng = np.random.default_rng(seed)
    mask = rng.random(arr.shape) < density
    step = rng.integers(0, 2, arr.shape, dtype=np.int16) * 2 - 1
    out = arr.astype(np.int16) + mask * step
    return np.clip(out, 0, 255).astype(np.uint8)


def save(arr, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(arr, "RGB").save(path, format="PNG", optimize=True)


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "examples")
    base, cap = out / "baseline", out / "capture"

    # identical
    a = to_srgb8(render())
    save(a, base / "sphere_identical.png")
    save(a, cap / "sphere_identical.png")

    # subtle, sub-threshold change: +-1 dither on about 5% of channel values
    b = to_srgb8(render(albedo=(0.25, 0.55, 0.80), sky_top=(0.30, 0.20, 0.55)))
    save(b, base / "sphere_subtle.png")
    save(add_dither(b, seed=7), cap / "sphere_subtle.png")

    # clear regression: shadow moved by 7 px right / 3 px down, warmer light
    save(to_srgb8(render(albedo=(0.30, 0.70, 0.35), sphere_center=(118.0, 110.0))),
         base / "sphere_shadow.png")
    save(
        to_srgb8(
            render(
                albedo=(0.30, 0.70, 0.35),
                sphere_center=(118.0, 110.0),
                shadow_shift=(7.0, 3.0),
                light_color=(1.0, 0.82, 0.62),
            )
        ),
        cap / "sphere_shadow.png",
    )

    # new (capture only) and missing (baseline only)
    save(to_srgb8(render(albedo=(0.85, 0.75, 0.20), sphere_center=(150.0, 115.0), radius=46.0)),
         cap / "sphere_new.png")
    save(to_srgb8(render(albedo=(0.55, 0.30, 0.75), sphere_center=(100.0, 118.0), radius=52.0)),
         base / "sphere_missing.png")

    total = sum(p.stat().st_size for p in out.rglob("*.png"))
    print(f"wrote {len(list(out.rglob('*.png')))} images, {total} bytes, under {out}")


if __name__ == "__main__":
    main()
