#!/usr/bin/env python3
"""Run every tested command block in docs/guides/*.md against a built saccade.

A fenced block is tested when its info string has `case=` attributes:

    ```sh case=known-bad exit=1
    saccade compare ...
    ```

Attributes: `case` (known-good, known-bad, missing-input, unavailable-dependency),
`exit` (expected exit status of the block's last command), `says="text"` (output
must contain it), `requires=FEATURE` (run only when this build has it) and
`unavailable=FEATURE` (run only when this build lacks it). Every guide must
contain all four cases. Blocks run with the sample inputs from
scripts/gen-guide-fixtures.py under `samples/` and the repository `examples/`.

Usage: scripts/test-guides.py [--bin PATH] [GUIDE.md ...]
"""
import argparse
import json
import os
import pathlib
import re
import shlex
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
CASES = {"known-good", "known-bad", "missing-input", "unavailable-dependency"}
FENCE = re.compile(r"^```(?:sh|bash) ([^\n]+)\n(.*?)^```", re.S | re.M)


def blocks(text):
    for m in FENCE.finditer(text):
        attrs = dict(
            part.split("=", 1) for part in shlex.split(m.group(1)) if "=" in part
        )
        if "case" in attrs:
            yield attrs, m.group(2)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=os.environ.get("SACCADE_BIN"))
    ap.add_argument("guides", nargs="*")
    args = ap.parse_args()
    if not args.bin:
        sys.exit("test-guides: pass --bin PATH or set SACCADE_BIN")
    binary = pathlib.Path(args.bin).resolve()
    guides = [pathlib.Path(g) for g in args.guides] or sorted((ROOT / "docs/guides").glob("*.md"))
    doctor = json.loads(subprocess.run([binary, "doctor", "--json"], capture_output=True, text=True, check=True).stdout)
    have = {r["feature"] for r in doctor["command_availability"] if r["status"] == "available"}
    failures, ran, skipped = [], 0, 0
    with tempfile.TemporaryDirectory(prefix="saccade-guides-") as tmp:
        tmp = pathlib.Path(tmp)
        (tmp / "bin").mkdir()
        (tmp / "bin/saccade").symlink_to(binary)
        subprocess.run([sys.executable, ROOT / "scripts/gen-guide-fixtures.py", tmp / "samples"], check=True)
        env = dict(os.environ, PATH=f"{tmp / 'bin'}:{os.environ['PATH']}")
        for guide in guides:
            found = set()
            work = tmp / guide.stem
            work.mkdir()
            (work / "samples").symlink_to(tmp / "samples")
            (work / "examples").symlink_to(ROOT / "examples")
            (work / "testdata").symlink_to(ROOT / "testdata")
            for attrs, body in blocks(guide.read_text()):
                case = attrs["case"]
                found.add(case)
                name = f"{guide.name} [{case}]"
                if case not in CASES or "exit" not in attrs:
                    failures.append(f"{name}: needs a known case and exit=")
                    continue
                if "requires" in attrs and attrs["requires"] not in have:
                    skipped += 1
                    print(f"SKIP {name}: build lacks {attrs['requires']}")
                    continue
                if "unavailable" in attrs and attrs["unavailable"] in have:
                    skipped += 1
                    print(f"SKIP {name}: build has {attrs['unavailable']}")
                    continue
                out = subprocess.run(["bash", "-c", body], cwd=work, env=env, capture_output=True, text=True)
                ran += 1
                text = out.stdout + out.stderr
                ok = out.returncode == int(attrs["exit"]) and attrs.get("says", "") in text
                print(f"{'ok  ' if ok else 'FAIL'} {name}: exit {out.returncode}")
                if not ok:
                    failures.append(f"{name}: expected exit {attrs['exit']} and {attrs.get('says', '')!r}, got {out.returncode}\n{text[-600:]}")
            if found != CASES:
                failures.append(f"{guide.name}: missing cases {sorted(CASES - found)}")
    print(f"{ran} blocks run, {skipped} skipped, {len(failures)} failed")
    for f in failures:
        print("FAILURE", f)
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
