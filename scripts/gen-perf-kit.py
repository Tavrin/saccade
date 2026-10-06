#!/usr/bin/env python3
"""Generate the deterministic performance-sidecar conformance kit in examples/perf-kit.

Standard library only. The kit has one baseline arm, one unchanged-arm control and
three candidate arms (unchanged, 8% slower, 8% faster), each as a hyperfine-style
export of 12 independently acquired runs, plus one timing session per candidate
and a valid and an invalid performance sidecar. Rerun to regenerate byte for byte.
"""
import json
import math
import random
from pathlib import Path

OUT = Path(__file__).resolve().parents[1] / "examples" / "perf-kit"
RUNS = 12
BASE_S = 0.0100          # nominal seconds per run
NOISE = 0.02             # lognormal sigma of run-to-run noise


def runs(seed, scale):
    rng = random.Random(seed)
    return [round(BASE_S * scale * math.exp(rng.gauss(0, NOISE)), 9) for _ in range(RUNS)]


def hyperfine(command, times):
    mean = sum(times) / len(times)
    return {"results": [{"command": command, "mean": mean, "times": times}]}


def session(name, candidate):
    pairs = []
    for control in (False, True):
        for i in range(RUNS):
            right = "a-control.json" if control else candidate
            pairs.append({
                "id": f"{'aa' if control else 'ab'}-{i:02d}",
                "block": f"{'aa' if control else 'ab'}-{i:02d}",
                "order": "ab" if i % 2 == 0 else "ba",
                "aa": control,
                "a": {"file": "a.json", "format": "hyperfine", "index": i},
                "b": {"file": right, "format": "hyperfine", "index": i},
            })
    return {"schema": "saccade-timing-session.v1", "session": name, "band_pct": 3,
            "max_pairs": RUNS, "confidence": 0.95, "seed": 7, "resamples": 2048, "pairs": pairs}


def write(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    write("a.json", hyperfine("workload --baseline", runs(1, 1.0)))
    write("a-control.json", hyperfine("workload --baseline", runs(2, 1.0)))
    for name, seed, scale in (("unchanged", 3, 1.0), ("slower", 4, 1.08), ("faster", 5, 0.92)):
        write(f"b-{name}.json", hyperfine("workload --candidate", runs(seed, scale)))
        write(f"session-{name}.json", session(f"kit-{name}", f"b-{name}.json"))
    valid = {"schema": "saccade-perf.v1", "unit": "ms",
             "frame": {"value": 10.0, "samples": 64, "stat": "mean"}, "terms": [], "counters": {}}
    write("sidecar-valid.json", valid)
    write("sidecar-invalid.json", {"schema": "saccade-perf.v1", "unit": "ms", "frame": {"value": -1.0}})


if __name__ == "__main__":
    main()
