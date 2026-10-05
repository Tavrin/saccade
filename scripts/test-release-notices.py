#!/usr/bin/env python3
"""Follow the binary packaging recipe for tar/zip and check retained notices."""
import importlib.util
import pathlib
import subprocess
import tarfile
import tempfile
import unittest
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('release_notices', ROOT / 'scripts/check-release-notices.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class ReleaseNoticesTests(unittest.TestCase):
    def test_w3_f03_release_archives_retain_daltonlens(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            notices = root / 'THIRD_PARTY_NOTICES.md'
            subprocess.run(['python3', str(ROOT / 'scripts/generate-third-party-notices.py'), str(notices)],
                           cwd=ROOT, check=True)
            (root / 'saccade').write_bytes(b'constructed packaging test executable')
            files = [root / 'saccade', ROOT / 'LICENSE-MIT', ROOT / 'LICENSE-APACHE', notices]
            tar = root / 'release.tar.gz'
            with tarfile.open(tar, 'w:gz') as archive:
                for file in files:
                    archive.add(file, arcname=file.name)
            zip_path = root / 'release.zip'
            with zipfile.ZipFile(zip_path, 'w') as archive:
                for file in files:
                    archive.write(file, arcname=file.name)
            for path in [tar, zip_path]:
                checker.check_archive(path)
            notices.write_text('# Third-party notices\nDaltonLens\n')
            with zipfile.ZipFile(zip_path, 'w') as archive:
                archive.write(notices, arcname=notices.name)
            with self.assertRaisesRegex(AssertionError, 'missing full DaltonLens'):
                checker.check_archive(zip_path)


if __name__ == '__main__':
    unittest.main()
