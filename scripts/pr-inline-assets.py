#!/usr/bin/env python3
"""Opt-in GitHub Action publisher for bounded PR images; no checkout mutation.

GitHub Git Database API: https://docs.github.com/en/rest/git
A root commit with no parents creates the dedicated orphan branch.
"""
from __future__ import annotations

import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from urllib.error import HTTPError
from urllib.parse import quote, urlparse
from urllib.request import Request, urlopen

MARKER = b"saccade-assets.v1\n"
MAX_IMAGE_BYTES = 2 * 1024 * 1024
SHA = re.compile(r"[0-9a-f]{40}\Z")
REPO = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\Z")
BRANCH = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}\Z")


def checked_sha(value: object) -> str:
    if not isinstance(value, str) or not SHA.fullmatch(value):
        raise ValueError("GitHub returned an invalid Git object SHA")
    return value


def settings(env: dict[str, str]) -> tuple[str, str, int, str, str]:
    repo = env["REPO"]
    branch = env["ASSET_BRANCH"]
    pr = int(env["PR_NUMBER"])
    server = env["SERVER_URL"].rstrip("/")
    parsed = urlparse(server)
    if not REPO.fullmatch(repo) or repo.startswith(".") or "/." in repo:
        raise ValueError("invalid repository name")
    if not BRANCH.fullmatch(branch) or ".." in branch or branch.endswith(".lock"):
        raise ValueError("asset branch must be one simple branch name")
    if pr <= 0 or parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password or parsed.path not in ("", "/") or parsed.query or parsed.fragment:
        raise ValueError("invalid PR number or GitHub server URL")
    api_root = "https://api.github.com" if parsed.hostname == "github.com" else server + "/api/v3"
    return repo, branch, pr, server, api_root


class Api:
    def __init__(self, root: str, repo: str, token: str):
        self.base = root + "/repos/" + repo
        self.token = token

    def request(self, method: str, path: str, data: dict | None = None) -> dict:
        payload = None if data is None else json.dumps(data, separators=(",", ":")).encode()
        request = Request(self.base + path, data=payload, method=method,
                          headers={"Accept": "application/vnd.github+json",
                                   "Authorization": "Bearer " + self.token,
                                   "X-GitHub-Api-Version": "2022-11-28",
                                   "Content-Type": "application/json"})
        with urlopen(request, timeout=20) as response:
            return json.load(response)


def image_entries(report_path: Path, temp: Path) -> tuple[str, list[tuple[str, bytes, str]]]:
    report_bytes = report_path.read_bytes()
    report = json.loads(report_bytes)
    if report.get("schema") != "saccade-report.v1" or not isinstance(report.get("entries"), list):
        raise ValueError("unsupported or malformed saccade report")
    digest = hashlib.sha256(report_bytes).hexdigest()[:16]
    files = []
    for index, entry in enumerate((e for e in report["entries"]
                                   if e.get("status") == "fail" and e.get("paths", {}).get("heatmap"))):
        if index >= 3:
            break
        name = entry["name"]
        if not isinstance(name, str) or not name:
            raise ValueError("report entry has no name")
        for label, state in (("thumbnail", "layout=swipe"), ("heatmap", "layout=heatmap&heat=1")):
            output = temp / f"{index + 1:02d}-{label}.png"
            subprocess.run(["saccade", "inspect", "export", str(report_path), "--format", "png",
                            "--entry", name, "--state", state, "--width", "640", "--out", str(output)],
                           check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=60)
            data = output.read_bytes()
            if not data.startswith(b"\x89PNG\r\n\x1a\n") or len(data) > MAX_IMAGE_BYTES:
                raise ValueError("inline snapshot is not a PNG or exceeds 2 MiB")
            files.append((output.name, data, name))
    return digest, files


def blob(api: Api, data: bytes) -> str:
    return checked_sha(api.request("POST", "/git/blobs", {
        "content": base64.b64encode(data).decode("ascii"), "encoding": "base64"})["sha"])


def publish(api: Api, repo: str, branch: str, pr: int, digest: str,
            files: list[tuple[str, bytes, str]], server: str) -> list[tuple[str, str, str]]:
    ref_path = "/git/ref/heads/" + quote(branch, safe="")
    try:
        ref = api.request("GET", ref_path)
    except HTTPError as error:
        if error.code != 404:
            raise
        parent = None
        base_tree = None
    else:
        parent = checked_sha(ref["object"]["sha"])
        previous = api.request("GET", "/git/commits/" + parent)
        base_tree = checked_sha(previous["tree"]["sha"])
        tree = api.request("GET", "/git/trees/" + base_tree)
        marker = next((e for e in tree.get("tree", []) if e.get("path") == ".saccade-assets"), None)
        if marker is None:
            raise ValueError("asset branch exists without the saccade orphan marker")
        marker_blob = api.request("GET", "/git/blobs/" + checked_sha(marker["sha"]))
        if base64.b64decode(marker_blob["content"]) != MARKER:
            raise ValueError("asset branch marker does not match")
    prefix = f"pr-{pr}/{digest}/"
    entries = [{"path": prefix + filename, "mode": "100644", "type": "blob", "sha": blob(api, data)}
               for filename, data, _ in files]
    if parent is None:
        entries.append({"path": ".saccade-assets", "mode": "100644", "type": "blob", "sha": blob(api, MARKER)})
    tree_data = {"tree": entries}
    if base_tree:
        tree_data["base_tree"] = base_tree
    new_tree = checked_sha(api.request("POST", "/git/trees", tree_data)["sha"])
    commit = checked_sha(api.request("POST", "/git/commits", {
        "message": f"saccade PR #{pr} inline images ({digest})", "tree": new_tree,
        "parents": [] if parent is None else [parent]})["sha"])
    if parent is None:
        api.request("POST", "/git/refs", {"ref": "refs/heads/" + branch, "sha": commit})
    else:
        api.request("PATCH", "/git/refs/heads/" + quote(branch, safe=""), {"sha": commit, "force": False})
    if urlparse(server).hostname == "github.com":
        root = "https://raw.githubusercontent.com/" + repo + "/" + commit + "/"
    else:
        root = server + "/" + repo + "/raw/" + commit + "/"
    return [(name, label, root + prefix + quote(filename)) for filename, _, name in files
            for label in ["heatmap" if filename.endswith("-heatmap.png") else "thumbnail"]]


def main(env: dict[str, str] | None = None) -> int:
    env = os.environ if env is None else env
    # Guard at the write boundary as well as in action.yml. Forks retain the
    # uploaded artifact link in the existing job summary, with no branch write.
    if env.get("EVENT_NAME") != "pull_request" or env.get("FORK") == "true" or env.get("HEAD_REPO") != env.get("REPO"):
        return 0
    try:
        repo, branch, pr, server, api_root = settings(env)
        if not env.get("GH_TOKEN"):
            raise ValueError("a token with contents:write is required")
        report = Path(env["REPORT_DIR"]) / "saccade-report.v1.json"
        summary = Path(env["SUMMARY_PATH"])
        with tempfile.TemporaryDirectory(prefix="saccade-inline-") as temporary:
            digest, images = image_entries(report, Path(temporary))
            if not images:
                return 0
            links = publish(Api(api_root, repo, env["GH_TOKEN"]), repo, branch, pr, digest, images, server)
        with summary.open("a", encoding="utf-8") as output:
            output.write("\n### Inline visual evidence\n\n")
            for name, label, url in links:
                safe_name = re.sub(r"[^A-Za-z0-9._/-]", "_", name)[:80]
                output.write(f"{label} for `{safe_name}`: ![{label}]({url})\n\n")
        return 0
    except Exception as error:
        # Publishing is optional. Keep the artifact-linked summary unchanged.
        reason = f"HTTP {error.code}" if isinstance(error, HTTPError) else type(error).__name__
        print(f"::warning::Inline image publishing failed ({reason}); keeping the artifact link.", file=sys.stderr)
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
