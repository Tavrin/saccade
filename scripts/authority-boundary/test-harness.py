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

    def test_signed_probes_supply_forged_proof_and_enable_mcp_policy(self):
        import tempfile
        from unittest.mock import Mock
        with tempfile.TemporaryDirectory() as directory:
            h = authority.Harness(Path("/usr/bin/unused"), Path(directory))
            baseline = h.inputs / "baseline/sample.png"
            authority.png(baseline, 20)
            def fake_cli(argv):
                if "--dry-run" in argv:
                    plan = Path(argv[argv.index("--out") + 1]); plan.mkdir()
                    (plan / "approval.json").write_text("{}")
                return 2, {"errors":[{"code":"approval_signature_invalid"}]}, {}
            h.cli = Mock(side_effect=fake_cli); h.record = Mock(); h.mcp = Mock()
            h.signed_probes(h.outputs / "report.json", baseline, authority.digest(baseline))
            argv = h.cli.call_args_list[-1].args[0]
            self.assertIn("--approval-record", argv)
            self.assertIn("--approval-signature", argv)
            self.assertIn("--require-signed-approval", argv)
            self.assertTrue(Path(argv[argv.index("--approval-signature") + 1]).is_file())
            calls, startup = h.mcp.call_args.args
            self.assertIn("--require-signed-approval", startup)
            self.assertEqual(calls[0][1], "saccade_measure")
            self.assertEqual(calls[0][2]["operation"], "compare")
            self.assertEqual(calls[0][3], "approval_signature_required")

    def test_known_failure_cannot_hide_a_changed_result(self):
        self.assertEqual(authority.classify(0, None, 0, None, True, "credential absent"), "XFAIL")
        self.assertEqual(authority.classify(0, None, 2, "io", True, "credential absent"), "FAIL")
        self.assertEqual(authority.classify(0, None, 0, None, False, "credential absent"), "FAIL")


if __name__ == "__main__":
    unittest.main()
