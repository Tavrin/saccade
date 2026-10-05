#!/usr/bin/env python3
"""Small offline preflight tests, including reflected secret suppression."""
from pathlib import Path
import tempfile
import unittest
from preflight import check_credentials

class PreflightTests(unittest.TestCase):
    def test_missing_empty_symlink_and_valid_fixed_files(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            with self.assertRaises(ValueError): check_credentials(directory)
            (directory / "gemini.env").write_text("SACCADE_GEMINI_API_KEY=fixture-secret\n")
            (directory / "jev.env").write_text("JEV_API_KEY=\n")
            with self.assertRaises(ValueError) as error: check_credentials(directory)
            self.assertNotIn("fixture-secret", str(error.exception))
            (directory / "jev.env").write_text("export JEV_API_KEY='fixture-secret'\n")
            check_credentials(directory)
            (directory / "jev.env").unlink()
            (directory / "jev.env").symlink_to(directory / "gemini.env")
            with self.assertRaises(ValueError): check_credentials(directory)

if __name__ == "__main__": unittest.main()
