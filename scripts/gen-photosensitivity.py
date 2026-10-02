#!/usr/bin/env python3
"""Small deterministic 4 Hz fixture; no dependencies, downloads, clock or GPU.

This generates potentially provocative imagery. The report displays only static
frames. Do not play the sequence to a viewer; it is synthetic detector evidence.
"""
import json
import struct
import zlib
from pathlib import Path

CASE = Path(__file__).resolve().parents[1] / "showcases" / "photosensitivity"


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def main():
    frames = CASE / "frames"
    frames.mkdir(parents=True, exist_ok=True)
    width, height = 64, 32
    for i in range(64):
        value = 255 if i % 8 < 4 else 0
        raw = (b"\0" + bytes([value]) * width * 3) * height
        png = b"\x89PNG\r\n\x1a\n"
        png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        png += chunk(b"IDAT", zlib.compress(raw, level=9))
        png += chunk(b"IEND", b"")
        (frames / f"frame_{i:03}.png").write_bytes(png)
    (frames / "saccade-meta.json").write_text(json.dumps({"fps": 32}, indent=2) + "\n")
    print("photosensitivity: 64 frames, 64x32, 32 fps, 4 Hz, deterministic")


if __name__ == "__main__":
    main()
