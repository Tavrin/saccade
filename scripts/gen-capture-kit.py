#!/usr/bin/env python3
"""Generate local capture-adapter conformance fixtures; stdlib only, no capture."""
import argparse
import copy
import hashlib
import importlib.util
import json
import struct
import tempfile
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "examples/capture-kit"
SPEC = importlib.util.spec_from_file_location("capture_record", ROOT / "scripts/capture-record.py")
ADAPTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADAPTER)


def png():
    def chunk(name, data):
        return struct.pack(">I", len(data)) + name + data + struct.pack(">I", zlib.crc32(name + data))
    rows = b"".join(b"\0" + b"".join(bytes((x * 32, y * 32, 96)) for x in range(8)) for y in range(8))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 8, 8, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b"")


def write(root, name, value):
    (root / name).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def generate(root):
    root.mkdir(parents=True, exist_ok=True)
    (root / "image.png").write_bytes(png())
    manifest = []
    for producer in ("renderer", "browser"):
        run_id = f"generated-{producer}-001"
        fingerprint = {
            "schema": "saccade-arm-fingerprint.v1",
            "producer": {"binary": "sha256:" + hashlib.sha256(f"synthetic-{producer}-executable".encode()).hexdigest(),
                         "build": {"profile": "fixture", "features": []}},
            "inputs": {"identity": "sha256:" + hashlib.sha256(b"generated-8x8-gradient-input").hexdigest()},
            "run": {"mode": "fixed", "env": {}, "session": run_id,
                    "readiness": [{"criterion": {"name": "settled", "parameters": {}}, "reached": True, "observed": True}]}}
        settings = ({"width": 8, "height": 8, "seed": 7, "quality": "fixed"}
                    if producer == "renderer" else {"viewport": [8, 8], "animations": "disabled", "caret": "hide", "fonts_ready": True})
        plan = [{"id": "frame-001", "settings": settings, "fingerprint": fingerprint, "clock_domain": "synthetic-monotonic"}]
        acquired = [{"id": "frame-001", "status": "captured", "error": None, "image": "image.png",
                     "settings": settings, "fingerprint": fingerprint,
                     "clock": {"domain": "synthetic-monotonic", "run_id": run_id, "start_ns": 100, "end_ns": 200},
                     "details": {} if producer == "renderer" else {"final_url": "https://example.org/", "http_status": 200, "test": "generated screenshot"}}]
        write(root, f"{producer}-plan.json", plan)
        write(root, f"{producer}-acquisitions.json", acquired)
        valid = ADAPTER.record(plan, acquired, producer, run_id, "complete", root)
        cases = [("valid", None, 0, valid)]

        def case(name, code, mutation, exit_code=1):
            value = copy.deepcopy(valid)
            mutation(value, value["acquisitions"][0])
            cases.append((name, code, exit_code, value))

        case("missing-image", "missing_image", lambda r, a: a["image"].update(path="absent.png"))
        case("stale-hash", "stale_hash", lambda r, a: a["image"].update(sha256="0" * 64))
        case("failed-acquisition", "acquisition_failed", lambda r, a: a.update(status="failed", error="generated acquisition timeout"))
        case("mismatched-settings", "settings_mismatch", lambda r, a: a["settings"].update(extra_setting=True))
        case("partial-run", "partial_run", lambda r, a: r.update(acquisitions=[]))
        case("declared-partial", "partial_run", lambda r, a: r.update(completion="partial"))
        case("skipped-acquisition", "partial_run", lambda r, a: a.update(status="skipped", image=None))
        case("clock-mismatch", "clock_mismatch", lambda r, a: a["clock"].update(domain="different-domain"))
        case("clock-run-mismatch", "clock_mismatch", lambda r, a: a["clock"].update(run_id="other-run"))
        case("clock-reversed", "clock_mismatch", lambda r, a: a["clock"].update(end_ns=99))
        case("clock-missing", "clock_mismatch", lambda r, a: a.update(clock=None))
        case("identity-mismatch", "identity_mismatch", lambda r, a: a["fingerprint"]["producer"].update(binary="sha256:" + "1" * 64))
        case("session-mismatch", "identity_mismatch", lambda r, a: a["fingerprint"]["run"].update(session="other-session"))
        case("missing-identity", "missing_identity", lambda r, a: a.update(fingerprint=None))
        case("readiness-not-reached", "readiness_not_reached", lambda r, a: a["fingerprint"]["run"]["readiness"][0].update(reached=False))
        case("unsafe-image-path", "unsafe_image_path", lambda r, a: a["image"].update(path="../image.png"))
        case("duplicate-slot", "invalid_record", lambda r, a: r["acquisitions"].append(copy.deepcopy(a)), 2)
        case("unsupported-schema", "unsupported_schema", lambda r, a: r.update(schema="saccade-capture-record.v99"), 2)
        for name, code, exit_code, value in cases:
            file = f"{producer}-{name}.json"
            write(root, file, value)
            manifest.append({"file": file, "code": code, "exit": exit_code})
    write(root, "expected.json", manifest)
    write(root, "provenance.json", {"licence": "CC0-1.0", "source": "procedural local generation; no third-party imagery",
          "generator": "scripts/gen-capture-kit.py", "image_sha256": hashlib.sha256(png()).hexdigest(),
          "identities": "synthetic declarations, not acquired executable or prepared-input hashes",
          "clocks": "synthetic, not performance qualification", "browser": "Playwright-style observations; no browser executed"})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--out", type=Path, default=OUT)
    args = parser.parse_args()
    if not args.check:
        generate(args.out)
        return
    with tempfile.TemporaryDirectory(prefix="capture-kit-") as directory:
        root = Path(directory)
        generate(root)
        stale = [p.name for p in root.iterdir() if not (args.out / p.name).is_file() or p.read_bytes() != (args.out / p.name).read_bytes()]
        if stale:
            raise SystemExit("stale capture kit: " + ", ".join(sorted(stale)))
    print("capture-kit: deterministic bytes PASS")


if __name__ == "__main__":
    main()
