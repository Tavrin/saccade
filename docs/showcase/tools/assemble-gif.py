#!/usr/bin/env python3
"""Assemble the frames from capture-gif-frames.cjs into an optimized GIF (Pillow only).

Usage: python3 assemble-gif.py <frames-dir> <out.gif> [--fps 12] [--width 760]
"""
import argparse
from pathlib import Path

from PIL import Image

ap = argparse.ArgumentParser()
ap.add_argument('frames')
ap.add_argument('out')
ap.add_argument('--fps', type=float, default=12)
ap.add_argument('--width', type=int, default=760)
a = ap.parse_args()

paths = sorted(Path(a.frames).glob('f*.png'))
frames = []
for p in paths:
    im = Image.open(p).convert('RGB')
    if im.width > a.width:
        im = im.resize((a.width, round(im.height * a.width / im.width)), Image.LANCZOS)
    frames.append(im)
# one shared palette from a sample of frames keeps colours stable between frames
sample = Image.new('RGB', (frames[0].width, frames[0].height * 4))
for i, k in enumerate(range(0, len(frames), max(1, len(frames) // 4))):
    if i < 4:
        sample.paste(frames[k], (0, frames[0].height * i))
pal = sample.quantize(colors=255, method=Image.Quantize.MEDIANCUT)
q = [f.quantize(palette=pal, dither=Image.Dither.NONE) for f in frames]
# merge identical consecutive frames into longer durations
out, dur = [], []
step = round(1000 / a.fps)
for f in q:
    if out and f.tobytes() == out[-1].tobytes():
        dur[-1] += step
    else:
        out.append(f)
        dur.append(step)
dur[-1] += 1200  # pause on the last frame before looping
out[0].save(a.out, save_all=True, append_images=out[1:], duration=dur, loop=0, optimize=True, disposal=1)
print(f'{a.out}: {len(out)} frames, {Path(a.out).stat().st_size / 1e6:.2f} MB')
