"""Exercise the installed distribution on a generated PNG, without models."""
import hashlib
from importlib.metadata import version
import struct
import zlib

import saccade


def chunk(kind, data):
    return (struct.pack(">I", len(data)) + kind + data
            + struct.pack(">I", zlib.crc32(kind + data)))


png = (b"\x89PNG\r\n\x1a\n"
       + chunk(b"IHDR", struct.pack(">IIBBBBB", 40, 30, 8, 2, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress((b"\0" + bytes([20, 40, 70]) * 40) * 30))
       + chunk(b"IEND", b""))
assert version("saccade-vision") == "0.2.2"
analyzer = saccade.Analyzer(profile="cpu-lite", allow_download=False)
record = analyzer.analyze_media(png)
assert record["schema"] == "saccade-media-record.v1"
assert record["profile"] == "cpu-lite"
assert record["identity"]["data"]["sha256"] == hashlib.sha256(png).hexdigest()
assert record["description"]["status"] == "skipped"
assert analyzer.compare(png, png)["metrics"]["mean"] == 0
print("Installed saccade-vision wheel: cpu-lite smoke PASS")
