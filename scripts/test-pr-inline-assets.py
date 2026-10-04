#!/usr/bin/env python3
"""Offline contract tests for the opt-in PR image publisher."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from urllib.error import HTTPError

MODULE = Path(__file__).with_name("pr-inline-assets.py")
spec = importlib.util.spec_from_file_location("pr_inline_assets", MODULE)
assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assets)


class FakeApi:
    def __init__(self, existing=False, marker=True):
        self.calls = []
        self.existing = existing
        self.marker = marker
        self.next_sha = 10

    def request(self, method, path, data=None):
        self.calls.append((method, path, data))
        if method == "GET" and path.startswith("/git/ref/"):
            if not self.existing:
                raise HTTPError("https://api.github.com", 404, "missing", {}, None)
            return {"object": {"sha": "1" * 40}}
        if path == "/git/commits/" + "1" * 40:
            return {"tree": {"sha": "2" * 40}}
        if path == "/git/trees/" + "2" * 40:
            return {"tree": [{"path": ".saccade-assets", "sha": "3" * 40}] if self.marker else []}
        if path == "/git/blobs/" + "3" * 40:
            return {"content": "c2FjY2FkZS1hc3NldHMudjEK"}
        if method == "POST" and path in ("/git/blobs", "/git/trees", "/git/commits"):
            sha = f"{self.next_sha:040x}"
            self.next_sha += 1
            return {"sha": sha}
        if method in ("POST", "PATCH") and path.startswith("/git/refs"):
            return {}
        raise AssertionError((method, path, data))


class InlineAssetsTest(unittest.TestCase):
    def test_snapshot_selection_is_bounded_and_uses_argument_arrays(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            report = root / "saccade-report.v1.json"
            report.write_text(json.dumps({"schema":"saccade-report.v1","entries":[
                {"status":"fail","name":f"scene {i};$(false).png","paths":{"heatmap":"heat.png"}}
                for i in range(5)]}))
            commands = []
            def fake_run(argv, **_kwargs):
                commands.append(argv)
                Path(argv[argv.index("--out")+1]).write_bytes(b"\x89PNG\r\n\x1a\nfixture")
            with mock.patch.object(assets.subprocess, "run", side_effect=fake_run):
                _digest, files = assets.image_entries(report, root)
            self.assertEqual(len(files), 6)
            self.assertEqual(len(commands), 6)
            self.assertEqual(commands[0][commands[0].index("--entry")+1], "scene 0;$(false).png")
            self.assertEqual(commands[1][commands[1].index("--state")+1], "layout=heatmap&heat=1")

    def test_new_branch_is_orphan_and_urls_pin_commit(self):
        api = FakeApi()
        links = assets.publish(api, "owner/repo", "saccade-assets", 7, "deadbeef",
                               [("01-thumbnail.png", b"png", "scene.png")], "https://github.com")
        commit = next(data for method, path, data in api.calls if path == "/git/commits")
        self.assertEqual(commit["parents"], [])
        self.assertIn(("POST", "/git/refs", {"ref": "refs/heads/saccade-assets", "sha": links[0][2].split("/")[5]}), api.calls)
        self.assertIn("raw.githubusercontent.com/owner/repo/", links[0][2])
        self.assertIn("/pr-7/deadbeef/01-thumbnail.png", links[0][2])
        self.assertEqual(sum(path == "/git/blobs" for _, path, _ in api.calls), 2)

    def test_existing_branch_requires_marker_and_nonforce_update(self):
        api = FakeApi(existing=True)
        assets.publish(api, "owner/repo", "saccade-assets", 7, "deadbeef",
                       [("01-heatmap.png", b"png", "scene.png")], "https://github.com")
        commit = next(data for _, path, data in api.calls if path == "/git/commits")
        self.assertEqual(commit["parents"], ["1" * 40])
        patch = next(data for method, _, data in api.calls if method == "PATCH")
        self.assertFalse(patch["force"])
        with self.assertRaisesRegex(ValueError, "orphan marker"):
            assets.publish(FakeApi(existing=True, marker=False), "owner/repo", "saccade-assets",
                           7, "deadbeef", [("01-heatmap.png", b"png", "scene.png")], "https://github.com")

    def test_fork_and_publish_failure_keep_artifact_summary(self):
        env = {"EVENT_NAME": "pull_request", "FORK": "true", "HEAD_REPO": "fork/repo", "REPO": "owner/repo"}
        self.assertEqual(assets.main(env), 0)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "report").mkdir()
            (root / "report/saccade-report.v1.json").write_text(json.dumps({"schema":"saccade-report.v1","entries":[]}))
            summary = root / "summary.md"
            summary.write_text("artifact: https://example.invalid/1\n")
            env.update({"FORK":"false", "HEAD_REPO":"owner/repo", "ASSET_BRANCH":"saccade-assets",
                        "PR_NUMBER":"7", "SERVER_URL":"https://github.com", "GH_TOKEN":"token",
                        "REPORT_DIR":str(root / "report"), "SUMMARY_PATH":str(summary)})
            with mock.patch.object(assets, "image_entries", return_value=("deadbeef", [("x.png", b"PNG", "scene")])):
                with mock.patch.object(assets, "publish", side_effect=HTTPError("https://api.github.com",403,"denied",{},None)):
                    self.assertEqual(assets.main(env), 0)
            self.assertEqual(summary.read_text(), "artifact: https://example.invalid/1\n")

    def test_invalid_branch_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "asset branch"):
            assets.settings({"REPO":"owner/repo", "ASSET_BRANCH":"main/../../bad",
                             "PR_NUMBER":"1", "SERVER_URL":"https://github.com"})


if __name__ == "__main__":
    unittest.main()
