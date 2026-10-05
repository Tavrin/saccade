#!/usr/bin/env python3
"""Regression tests for the packaged README release guard."""
import importlib.util
import io
import tarfile
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('check_packages', Path(__file__).with_name('check-packages.py'))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class PackagedReadmeTests(unittest.TestCase):
    def test_model_manifest_does_not_allow_weights_or_other_model_files(self):
        self.assertTrue(checker.approved_file('saccade-core', 'models/semantic-regions.json'))
        self.assertFalse(checker.approved_file('saccade', 'models/semantic-regions.json'))
        for name in ['model.onnx', 'model.safetensors', 'sam.pt', 'extra.json',
                     'semantic-regions.json/weights.pt']:
            with self.subTest(name=name):
                self.assertFalse(checker.approved_file('saccade-core', 'models/' + name))

    def test_compression_images_do_not_allow_arbitrary_core_or_private_images(self):
        for name in ('reference', 'brightness', 'blocks', 'patch'):
            self.assertTrue(checker.approved_image('saccade-core', f'tests/fixtures/compression-reference/{name}.png'))
        for name in ('private.png', 'reference.png/photo.png', 'reference.jpg'):
            self.assertFalse(checker.approved_image('saccade-core', f'tests/fixtures/compression-reference/{name}'))
        self.assertFalse(checker.approved_image('saccade-core', 'tests/fixtures/private.png'))
        self.assertFalse(checker.approved_image('saccade', 'tests/fixtures/compression-reference/reference.png'))

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.intended = self.root / 'README.md'
        self.body = b'# Library\n' + b'Library documentation.\n' * 30
        self.intended.write_bytes(self.body)
        self.crate = self.root / 'crate'
        self.crate.mkdir()
        self.manifest = self.crate / 'Cargo.toml'
        self.manifest.write_text('[package]\nreadme = "../README.md"\n')
        self.package = {'name': 'example', 'version': '0.1.1', 'manifest_path': str(self.manifest)}
        self.archive = self.root / 'example-0.1.1.crate'

    def package_readme(self, body, path='README.md'):
        with tarfile.open(self.archive, 'w:gz') as archive:
            for name, data in [('Cargo.toml', f'[package]\nreadme = "{path}"\n'.encode()),
                               ('README.md', body)]:
                info = tarfile.TarInfo('example-0.1.1/' + name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))

    def check(self, marker=None):
        with patch.object(checker, 'ROOT', self.root), redirect_stdout(io.StringIO()):
            checker.check_readme(self.package, self.archive, self.intended, marker)

    def test_intended_readme_passes(self):
        self.package_readme(self.body)
        self.check()

    def test_short_readme_fails_even_when_source_matches(self):
        self.intended.write_bytes(b'# Stub\n')
        self.package_readme(b'# Stub\n')
        with self.assertRaisesRegex(AssertionError, 'fewer than 30 lines'):
            self.check()

    def test_wrong_source_fails_even_when_bytes_match(self):
        (self.crate / 'README.md').write_bytes(self.body)
        self.manifest.write_text('[package]\nreadme = "README.md"\n')
        self.package_readme(self.body)
        with self.assertRaisesRegex(AssertionError, 'wrong README source'):
            self.check()

    def test_stale_packaged_readme_fails(self):
        self.package_readme(self.body + b'Old content.\n')
        with self.assertRaisesRegex(AssertionError, 'differs from intended source'):
            self.check()

    def test_wrong_packaged_path_fails(self):
        self.package_readme(self.body, 'stub.md')
        with self.assertRaisesRegex(AssertionError, 'wrong packaged README path'):
            self.check()

    def test_registry_marker_must_be_visible_in_archive(self):
        marker = 'mcp-name: io.github.Tavrin/saccade'
        for suffix, accepted in [('', False), (f'<!-- {marker} -->\n', False),
                                 (f'MCP Registry name: `{marker}`\n', True)]:
            with self.subTest(suffix=suffix):
                body = self.body + suffix.encode()
                self.intended.write_bytes(body)
                self.package_readme(body)
                if accepted:
                    self.check(marker)
                else:
                    with self.assertRaisesRegex(AssertionError, 'visible MCP ownership marker'):
                        self.check(marker)


if __name__ == '__main__':
    unittest.main()
