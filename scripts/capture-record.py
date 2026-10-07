#!/usr/bin/env python3
"""Wrap externally observed renderer/browser acquisitions; never acquire images.

Plan: JSON array of expected slots. Acquisitions: JSON array matching the record
contract, except image is a relative path or null; hashes are computed here.
Both files must be at most 1 MiB. No settings/identity/clock values are inferred
from the plan. Failed and skipped attempts are retained without image evidence.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path

IMAGE_LIMIT = 64 * 1024 * 1024


def image_binding(root, name):
    if name is None:
        return None
    path = Path(name)
    if not name or path.is_absolute() or any(p in ("..", ".") for p in name.split("/")):
        raise ValueError("image path must be relative without traversal")
    current = root
    for part in path.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError("image path cannot use symlinks")
    if not current.exists():
        return None
    if not current.is_file() or current.stat().st_size > IMAGE_LIMIT:
        raise ValueError("image must be a regular file of at most 64 MiB")
    with current.open("rb") as stream:
        data = stream.read(IMAGE_LIMIT + 1)
    if len(data) > IMAGE_LIMIT:
        raise ValueError("image exceeds 64 MiB")
    return {"path": name, "sha256": hashlib.sha256(data).hexdigest()}


def record(plan, acquisitions, producer, run_id, completion, root):
    """Preserve supplied observations, binding only successful acquired bytes."""
    observed = copy.deepcopy(acquisitions)
    for item in observed:
        item["image"] = (image_binding(root, item.get("image"))
                         if item["status"] == "captured" and item.get("error") is None
                         else None)
    return {"schema": "saccade-capture-record.v1", "producer": producer,
            "run_id": run_id, "completion": completion,
            "expected": copy.deepcopy(plan), "acquisitions": observed}


def read_json(path):
    with path.open("rb") as stream:
        data = stream.read(1024 * 1024 + 1)
    if len(data) > 1024 * 1024:
        raise ValueError("producer input exceeds 1 MiB")
    return json.loads(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--acquisitions", type=Path, required=True)
    parser.add_argument("--producer", choices=("renderer", "browser"), required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--completion", choices=("complete", "partial"), required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    value = record(read_json(args.plan), read_json(args.acquisitions), args.producer,
                   args.run_id, args.completion, args.out.parent)
    # Refuse overwriting a producer's existing evidence.
    with args.out.open("x", encoding="utf-8") as output:
        output.write(json.dumps(value, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
