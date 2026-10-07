#!/usr/bin/env python3
"""Generate deterministic moving-shape frame sequences; no downloads or providers."""
import argparse
import hashlib
import json
import pathlib
import struct
import zlib


def png(width, height, position):
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            inside = (x-position)**2 + (y-height//2)**2 <= 10**2
            rows.extend((80, 170, 220) if inside else (24, 28, 36))
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(rows, 9)) + chunk(b'IEND', b''))


def generate(out):
    out.mkdir()
    for name, positions in [('continuous', [24, 36]), ('displaced', [24, 96])]:
        clip = out / name
        clip.mkdir()
        frames = []
        for index, position in enumerate(positions):
            data = png(128, 64, position)
            file = f'{index:03}.png'
            (clip / file).write_bytes(data)
            frames.append(dict(index=index, timestamp_s=float(index), file=file,
                               sha256=hashlib.sha256(data).hexdigest()))
        (clip / 'frame-map.json').write_text(json.dumps(dict(schema='saccade-frame-map.v1', nominal_fps=1, frames=frames), indent=2)+'\n')
    (out / 'manifest.json').write_text(json.dumps(dict(schema='saccade-video-procedural.v1',
        licence='CC0-1.0', generator='video_corpus.py/1', seed=0,
        clips=['continuous/frame-map.json', 'displaced/frame-map.json'],
        truth='constructed displacement; no semantic naturalness truth or qualification'), indent=2)+'\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=pathlib.Path, required=True)
    generate(parser.parse_args().out)
