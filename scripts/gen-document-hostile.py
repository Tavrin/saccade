#!/usr/bin/env python3
"""Generate deterministic document intake fixtures. Project MIT OR Apache-2.0 licence.

All vector primitives and PDF bytes are procedural; no fonts, imagery or downloads.
Includes report-export and scientific-figure examples with inserted/reordered pages.
"""
import hashlib
import json
import pathlib
import sys
import zlib


def pdf(objects):
    data = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for i, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(f"{i} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(data)
    data.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        data.extend(f"{offset:010} 00000 n \n".encode())
    data.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return bytes(data)


def pages(colors, box="0 0 72 72", extra=(), style="solid"):
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>",
               f"<< /Type /Pages /Count {len(colors)} /Kids [".encode() +
               b" ".join(f"{3+2*i} 0 R".encode() for i in range(len(colors))) + b"] >>"]
    for i, color in enumerate(colors):
        objects.append(f"<< /Type /Page /Parent 2 0 R /MediaBox [{box}] /Contents {4+2*i} 0 R >>".encode())
        content = f"{color} rg 0 0 72 72 re f\n".encode()
        if style == "report":
            # Header and ruled table export, without font dependencies.
            content = f"1 1 1 rg 0 0 72 72 re f {color} rg 4 56 64 12 re f 4 8 64 4 re f 4 20 64 4 re f 4 32 64 4 re f\n".encode()
        elif style == "figure":
            # Plot axes and a piecewise curve, a generated scientific figure export.
            content = f"1 1 1 rg 0 0 72 72 re f 0 0 0 RG 1 w 8 64 m 8 8 l 64 8 l S {color} RG 2 w 8 12 m 20 24 l 32 18 l 44 48 l 60 60 l S\n".encode()
        objects.append(f"<< /Length {len(content)} >>\nstream\n".encode() + content + b"endstream")
    return pdf(objects + list(extra))


def generate(root):
    root.mkdir(parents=True, exist_ok=True)
    fixtures = {
        "oversized-page.pdf": (pages(["1 0 0"], "0 0 1000000 72"), "document_dimensions_limit"),
        "deep-object.pdf": (pages(["1 0 0"], extra=[b"[" * 65 + b"0" + b"]" * 65]), "document_depth_limit"),
        "huge-image.pdf": (pages(["1 0 0"], extra=[b"<< /Type /XObject /Subtype /Image /Width 100000 /Height 100000 /Length 0 >>\nstream\n\nendstream"]), "document_dimensions_limit"),
        "many-pages.pdf": (pages(["1 0 0"] * 501), "document_page_limit"),
    }
    bomb = zlib.compress(b"0" * (64 * 1024 * 1024 + 1), 9)
    fixtures["decompression-bomb.pdf"] = (pages(["1 0 0"], extra=[f"<< /Length {len(bomb)} /Filter /FlateDecode >>\nstream\n".encode() + bomb + b"\nendstream"]), "document_decompressed_limit")
    refs = [f"<< /Next {i+1} 0 R >>".encode() for i in range(5, 71)] + [b"null"]
    fixtures["deep-reference.pdf"] = (pages(["1 0 0"], extra=refs), "document_depth_limit")
    malformed = pages(["1 0 0"])
    start = malformed.rindex(b"startxref")
    fixtures["malformed-xref.pdf"] = (malformed[:start] + b"startxref\n99999999\n%%EOF\n", "document_malformed")
    # Rebuild xref offsets after escaped names rather than relying on repair.
    fixtures["escaped-decompression-bomb.pdf"] = (pages(["1 0 0"], extra=[f"<< /Length {len(bomb)} /Filter /Flate#44ecode >>\nstream\n".encode() + bomb + b"\nendstream"]), "document_decompressed_limit")
    # Parser whitespace includes NUL but excludes vertical tab. Use referenced
    # content streams and rebuilt classic xrefs, so no repair or unused object
    # semantics are needed for this differential.
    for label, separator, expected in [
        ("nul", b"\x00", "document_decompressed_limit"),
        ("vt", b"\x0b", "document_unsupported"),
    ]:
        stream = f"<< /Length {len(bomb)} /Filter".encode() + separator + b"/FlateDecode >>\nstream\n" + bomb + b"\nendstream"
        fixtures[f"{label}-filter-bomb.pdf"] = (pdf([
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Contents 4 0 R >>",
            stream,
        ]), expected)
    for label, filters, expected in [
        ("unknown", b"/Filter /UnknownDecode", "document_unsupported"),
        ("filter-as-value", b"/Foo /Filter /FlateDecode /Bar /Filter /LZWDecode", "document_unsupported"),
        ("array", b"/Filter [/FlateDecode]", "document_unsupported"),
        ("duplicate-array", b"/Filter [] /Filter /FlateDecode", "document_malformed"),
    ]:
        fixtures[f"{label}-filter.pdf"] = (pages(["1 0 0"], extra=[f"<< /Length {len(bomb)} ".encode() + filters + b" >>\nstream\n" + bomb + b"\nendstream"]), expected)
    parents = [f"<< /Parent {i+1} 0 R >>".encode() for i in range(5, 71)] + [b"null"]
    fixtures["deep-parent.pdf"] = (pages(["1 0 0"], extra=parents), "document_depth_limit")
    content = b"BI /W 100000 /H 100000 /F /FlateDecode ID x EI\n"
    fixtures["inline-image.pdf"] = (pages(["1 0 0"], extra=[f"<< /Length {len(content)} >>\nstream\n".encode() + content + b"endstream"]), "document_unsupported")
    safe = zlib.compress(b"1 0 0 rg 0 0 8 8 re f\n")
    safe_pdf = pdf([
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 72 72] /Contents 4 0 R >>",
        f"<< /Length {len(safe)} /Filter".encode() + b"\x00/FlateDecode >>\nstream\n" + safe + b"\x00endstream",
    ]).replace(b"startxref\n", b"startxref\x00")
    (root / "nul-filter-safe.pdf").write_bytes(safe_pdf)
    receipts = []
    for name, (data, code) in fixtures.items():
        (root / name).write_bytes(data)
        receipts.append({"file": name, "sha256": hashlib.sha256(data).hexdigest(), "expected_code": code})
    for domain, colors in [("report", ["1 0 0", "0 0 1"]), ("figure", ["0 1 0", "0.5 0 0.5"])]:
        before, after = pages(colors, style=domain), pages([colors[1], "0 0 0", colors[0]], style=domain)
        (root / f"{domain}-before.pdf").write_bytes(before)
        (root / f"{domain}-after.pdf").write_bytes(after)
        mapping = {"schema": "saccade-page-map.v1", "reference_sha256": hashlib.sha256(before).hexdigest(),
                   "candidate_sha256": hashlib.sha256(after).hexdigest(),
                   "pairs": [{"reference": 1, "candidate": 3}, {"reference": 2, "candidate": 1}, {"reference": None, "candidate": 2}]}
        (root / f"{domain}-map.json").write_text(json.dumps(mapping, indent=2) + "\n")
    (root / "manifest.json").write_text(json.dumps({"licence": "MIT OR Apache-2.0", "provenance": "Procedural PDF primitives from scripts/gen-document-hostile.py; no external assets", "hostile": receipts}, indent=2) + "\n")


if __name__ == "__main__":
    generate(pathlib.Path(sys.argv[1]))
