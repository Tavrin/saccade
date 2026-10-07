#!/usr/bin/env python3
"""Fail-closed checks for the authority runner's verdict handling."""
import importlib.util
from pathlib import Path
import unittest
import json
import subprocess
import tempfile
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("authority", Path(__file__).with_name("harness.py"))
authority = importlib.util.module_from_spec(spec)
spec.loader.exec_module(authority)


class Refusals(unittest.TestCase):
    def test_success_wrong_code_and_side_effect_cannot_pass(self):
        for exit_code, code, unchanged in [(0, None, True), (2, "io", True),
                                           (2, "unsafe_path", False)]:
            self.assertEqual(authority.classify(2, "unsafe_path", exit_code, code, unchanged), "FAIL")

    def test_known_failure_cannot_hide_a_changed_result(self):
        self.assertEqual(authority.classify(0, None, 0, None, True, "credential absent"), "XFAIL")
        self.assertEqual(authority.classify(0, None, 2, "io", True, "credential absent"), "FAIL")
        self.assertEqual(authority.classify(0, None, 0, None, False, "credential absent"), "FAIL")


class PreviewControl(unittest.TestCase):
    def test_all_review_previews_refused_cannot_pass_positive_control(self):
        with tempfile.TemporaryDirectory() as directory:
            harness = authority.Harness(Path("/generated/binary"), Path(directory))
            # Capture the real runner's first MCP group. The regression is in
            # that group, not a hand-built list which already assumes a control.
            class Captured(Exception):
                pass
            calls = []
            def capture(group, startup=()):
                calls.extend(group)
                raise Captured()
            def fake_cli(argv):
                if argv[0] == "compare":
                    output = Path(argv[argv.index("--out") + 1])
                    output.mkdir()
                    (output / "saccade-report.v1.json").write_text("{}")
                return 0, {}, {}
            with patch.object(harness, "cli", side_effect=fake_cli), patch.object(harness, "mcp", side_effect=capture), patch.object(harness, "record"):
                with self.assertRaises(Captured):
                    harness.run()
            controls = [c for c in calls if c[1] == "saccade_review" and c[3] is None]
            self.assertEqual(len(controls), 1)
            calls = [controls[0], next(c for c in calls if c[0] == "mcp-read-outside")]
            replies = [
                {"id": 1, "result": {"protocolVersion": "2025-06-18"}},
                {"id": 2, "result": {"tools": [{"name": "saccade_measure"}, {"name": "saccade_review"}]}},
                {"id": 3, "result": {}},
                *[{"id": i, "result": {"isError": True,
                   "structuredContent": {"errors": [{"code": "unsafe_path"}]}}} for i in (4, 5)],
            ]
            reply = subprocess.CompletedProcess([], 0, "\n".join(map(json.dumps, replies)), "")
            with patch.object(authority.subprocess, "run", return_value=reply):
                harness.mcp(calls)
            self.assertEqual([c["status"] for c in harness.cases], ["FAIL", "PASS"])


if __name__ == "__main__":
    unittest.main()
