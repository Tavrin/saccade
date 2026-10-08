#!/usr/bin/env python3
"""Generate the small, deterministic sample inputs used by docs/guides/*.md.

Standard library only. Usage: scripts/gen-guide-fixtures.py OUT_DIR
The images are synthetic (a UI screenshot, document pages, a product photo, a
shaded sphere); nothing here is copied from any project.
"""
import json
import math
import pathlib
import struct
import sys
import zlib


def write_png(path, rows):
    height, width = len(rows), len(rows[0])
    raw = b"".join(b"\x00" + bytes(v for px in row for v in px) for row in rows)

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    data = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def canvas(w, h, colour):
    return [[colour for _ in range(w)] for _ in range(h)]


def rect(img, x, y, w, h, colour):
    for yy in range(max(y, 0), min(y + h, len(img))):
        for xx in range(max(x, 0), min(x + w, len(img[0]))):
            img[yy][xx] = colour


def ui(button_x, button_colour):
    img = canvas(200, 140, (245, 246, 248))
    rect(img, 0, 0, 200, 24, (40, 44, 52))
    for i in range(4):
        rect(img, 16, 40 + i * 14, 120 - i * 14, 6, (90, 96, 108))
    rect(img, button_x, 108, 64, 20, button_colour)
    return img


def page(footer_x):
    img = canvas(200, 260, (255, 255, 255))
    for i in range(14):
        rect(img, 20, 24 + i * 14, 160 - (i % 3) * 22, 5, (20, 20, 20))
    rect(img, footer_x, 240, 12, 5, (20, 20, 20))
    return img


def photo(noise=0, shape="ellipse"):
    img = canvas(128, 128, (0, 0, 0))
    for y in range(128):
        for x in range(128):
            v = 200 - y // 2
            img[y][x] = (v, v - 10, v - 30)
    cx, cy = 64, 70
    for y in range(128):
        for x in range(128):
            if shape == "ellipse":
                inside = ((x - cx) / 44) ** 2 + ((y - cy) / 30) ** 2 < 1
            else:
                inside = abs(x - cx) < 20 and abs(y - cy) < 46
            if inside:
                shade = int(150 + 60 * math.cos((x - cx) / 44))
                img[y][x] = (shade, 60, 40)
    if noise:
        for y in range(0, 128, 7):
            for x in range(0, 128, 5):
                r, g, b = img[y][x]
                img[y][x] = (min(r + noise, 255), g, b)
    return img


def sphere(light):
    img = canvas(128, 128, (18, 18, 24))
    for y in range(128):
        for x in range(128):
            dx, dy = (x - 64) / 48, (y - 64) / 48
            d2 = dx * dx + dy * dy
            if d2 < 1:
                nz = math.sqrt(1 - d2)
                lit = max(0.0, 0.5 * dx * -0.4 + dy * -0.5 + nz * 0.75) * light
                v = int(min(1.0, 0.1 + lit) * 255)
                img[y][x] = (v, v, int(v * 0.9))
    return img


def main():
    out = pathlib.Path(sys.argv[1])
    write_png(out / "ui/baseline/login.png", ui(120, (30, 100, 220)))
    write_png(out / "ui/same/login.png", ui(120, (30, 100, 220)))
    write_png(out / "ui/moved/login.png", ui(132, (30, 100, 220)))
    write_png(out / "doc/v1/page-1.png", page(170))
    write_png(out / "doc/same/page-1.png", page(170))
    write_png(out / "doc/v2/page-1.png", page(120))
    for name, x in (("v1", 170), ("v2", 120)):
        (out / f"doc/{name}.svg").write_text(
            '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="260">'
            '<rect width="200" height="260" fill="white"/>'
            '<rect x="20" y="24" width="160" height="5"/>'
            f'<rect x="{x}" y="240" width="12" height="5"/></svg>\n'
        )
    write_png(out / "product/shots/front.png", photo())
    write_png(out / "product/shots/front-copy.png", photo(noise=1))
    write_png(out / "product/shots/side.png", photo(shape="bar"))
    (out / "product/corrupt").mkdir(parents=True, exist_ok=True)
    (out / "product/corrupt/broken.png").write_bytes(b"not an image")
    write_png(out / "render/parent/sphere.png", sphere(1.0))
    write_png(out / "render/same/sphere.png", sphere(1.0))
    write_png(out / "render/dim/sphere.png", sphere(0.7))
    for name, mode in (("parent", "fixed"), ("same", "fixed"), ("dim", "fixed"), ("other-mode", "varied")):
        meta = out / "render" / name
        meta.mkdir(parents=True, exist_ok=True)
        (meta / "saccade-meta.json").write_text(
            json.dumps({"run_mode": mode, "renderer_build": "synthetic-1"}, indent=1) + "\n"
        )
    write_png(out / "render/other-mode/sphere.png", sphere(1.0))
    # Automatic accessibility: known blank and low-contrast closed controls.
    write_png(out / "a11y/blank.png", canvas(100, 60, (255, 255, 255)))
    controls = canvas(100, 60, (255, 255, 255))
    rect(controls, 20, 15, 24, 24, (180, 180, 180))
    rect(controls, 22, 17, 20, 20, (255, 255, 255))
    write_png(out / "a11y/low-contrast.png", controls)
    # Delivery tuning: a source image and the currently shipped encoding of it.
    write_png(out / "delivery/source.png", photo())
    write_png(out / "delivery/current.png", photo(noise=2))


if __name__ == "__main__":
    main()
