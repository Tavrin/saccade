#!/usr/bin/env python3
"""Fail-closed checks for the authority runner's verdict handling."""
import importlib.util
from pathlib import Path
import unittest

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


if __name__ == "__main__":
    unittest.main()
