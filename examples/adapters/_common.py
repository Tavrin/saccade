"""Shared plumbing for the saccade decision adapters (standard library only).

An adapter reads one or more `saccade decision-request` documents, asks a
provider each question, and pipes the answers into `saccade decide` as JSON
lines. saccade itself never sees an API key: keys are read here, from the
environment, and are never printed or logged.
"""
import argparse
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request


def env_key(name, env_file=None):
    """Reads an API key from the environment, then from a KEY=VALUE file."""
    value = os.environ.get(name)
    if value:
        return value
    if env_file:
        path = os.path.expanduser(env_file)
        try:
            with open(path, encoding="utf-8") as fh:
                for line in fh:
                    line = line.strip()
                    if line and not line.startswith("#") and "=" in line:
                        k, v = line.split("=", 1)
                        if k.strip() == name:
                            return v.strip().strip("\"'")
        except OSError:
            pass
    sys.exit(f"{name} is not set (environment{' or ' + env_file if env_file else ''})")


def post_json(url, headers, body, timeout=60):
    """POSTs JSON and returns the decoded JSON reply. Errors never echo headers."""
    req = urllib.request.Request(
        url, data=json.dumps(body).encode(), method="POST",
        headers={"Content-Type": "application/json", **headers})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        sys.exit(f"{url}: HTTP {e.code}: {e.read()[:300].decode(errors='replace')}")
    except (urllib.error.URLError, TimeoutError) as e:
        sys.exit(f"{url}: {e}")


def parse_args(description):
    ap = argparse.ArgumentParser(description=description)
    ap.add_argument("requests", nargs="+",
                    help="decision-request JSON files from `saccade decision-request` ('-' for stdin)")
    ap.add_argument("--target", required=True,
                    help="what `saccade decide` records into: the report JSON, a view directory or a decisions file")
    ap.add_argument("--saccade", default="saccade", help="the saccade executable")
    ap.add_argument("--dry-run", action="store_true",
                    help="print the answers as JSON lines instead of recording them")
    return ap.parse_args()


def load_requests(paths):
    out = []
    for p in paths:
        text = sys.stdin.read() if p == "-" else open(p, encoding="utf-8").read()
        doc = json.loads(text)
        if doc.get("schema") != "saccade-decision-request.v1":
            sys.exit(f"{p}: not a saccade-decision-request.v1 document")
        out.append(doc)
    return out


def record(answers, args):
    """Pipes answer objects (one per line) into `saccade decide`."""
    lines = "".join(json.dumps(a) + "\n" for a in answers)
    if args.dry_run or not answers:
        sys.stdout.write(lines)
        return
    proc = subprocess.run([args.saccade, "decide", args.target, "--json"],
                          input=lines, text=True)
    sys.exit(proc.returncode)
