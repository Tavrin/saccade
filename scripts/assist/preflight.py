#!/usr/bin/env python3
"""Qualification credential preflight; no HTTP and no credential-bearing output."""
from pathlib import Path
import sys

def check_credentials(directory):
    """Validate the two fixed ordinary files without returning or printing values."""
    for filename, variable in (("gemini.env", "SACCADE_GEMINI_API_KEY"), ("jev.env", "JEV_API_KEY")):
        path = directory / filename
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 65536:
            raise ValueError("Qualification requires fixed Gemini and Jev credential files.")
        valid = False
        for raw in path.read_text().splitlines():
            line = raw.strip()
            if line.startswith("export "):
                line = line[7:].lstrip()
            name, separator, value = line.partition("=")
            if separator and name.strip() == variable and value.strip().strip("\"'").strip():
                valid = True
        if not valid:
            raise ValueError("Qualification requires nonempty fixed Gemini and Jev credentials.")

if __name__ == "__main__":
    try:
        check_credentials(Path.home() / ".config" / "saccade")
    except (ValueError, OSError, UnicodeError):
        print("Qualification refused: fixed Gemini and Jev credentials are unavailable; no provider call was made.", file=sys.stderr)
        sys.exit(4)
