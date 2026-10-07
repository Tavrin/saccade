#!/usr/bin/env python3
"""Regression checks for compiled feature discovery in documentation generation."""
import importlib.util
import io
import json
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('gen_docs', __file__.replace('test-gen-docs.py', 'gen-docs.py'))
gen_docs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen_docs)


class CompiledFeatures(unittest.TestCase):
    def generate(self, features, allow_missing_imgtune_avif=False, preserve_all_features_header=False):
        def run(argv, **kwargs):
            if argv[1:] == ['inspect', 'capabilities', '--json']:
                # The legacy inventory can omit newer feature flags.
                value = {'data': {'features': [], 'operations': ['text']}}
            elif argv[1:] == ['capabilities', '--json']:
                value = {'compiled_features': features}
            else:
                return subprocess.CompletedProcess(argv, 0, stdout='Usage: saccade\n')
            return subprocess.CompletedProcess(argv, 0, stdout=json.dumps(value))

        with patch.object(gen_docs.subprocess, 'run', side_effect=run):
            return gen_docs.generated('saccade', allow_missing_imgtune_avif, preserve_all_features_header)

    def features(self):
        manifest = gen_docs.tomllib.loads((gen_docs.ROOT / 'crates/saccade/Cargo.toml').read_text())
        return sorted(set(manifest['features']) - {'default'})

    def test_catalogue_features_and_inspected_operations_are_used(self):
        cli = self.generate(self.features())['docs/cli.md']
        self.assertIn('`ocr-provider`', cli)
        self.assertIn('## saccade text\n', cli)

    def test_missing_ocr_provider_still_rejects_binary(self):
        features = [f for f in self.features() if f != 'ocr-provider']
        with self.assertRaisesRegex(ValueError, 'missing: ocr-provider'):
            self.generate(features)

    def test_avif_exception_preserves_canonical_header(self):
        features = [f for f in self.features() if f != 'imgtune-avif']
        with self.assertRaisesRegex(ValueError, 'missing: imgtune-avif'):
            self.generate(features)
        cli = self.generate(features, True)['docs/cli.md']
        inventory = next(line for line in cli.splitlines() if line.startswith('Compiled features:'))
        self.assertIn('`imgtune-avif`', inventory)
        self.assertEqual(cli.splitlines()[4:8], self.generate(self.features())['docs/cli.md'].splitlines()[4:8])
        self.assertEqual(cli, self.generate(self.features())['docs/cli.md'])
        self.assertIn('## saccade text\n', cli)

    def test_preserved_header_keeps_reference_and_actual_inventory_separate(self):
        full = self.generate(self.features())['docs/cli.md']
        features = [f for f in self.features() if f != 'imgtune-avif']
        cli = self.generate(features, True, True)['docs/cli.md']
        self.assertEqual(cli.splitlines()[4:8], full.splitlines()[4:8])
        actual = cli.split('Actual generation binary', 1)[1]
        inventory = next(line for line in actual.splitlines() if line.startswith('Compiled features:'))
        self.assertNotIn('`imgtune-avif`', inventory)
        self.assertIn('--allow-missing-imgtune-avif', actual)

    def test_avif_exception_rejects_other_missing_features(self):
        features = [f for f in self.features() if f not in {'imgtune-avif', 'ocr-provider'}]
        with self.assertRaisesRegex(ValueError, 'missing: ocr-provider'):
            self.generate(features, True)


class CheckDiffs(unittest.TestCase):
    def check(self, existing, generated):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, body in existing.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(body, encoding='utf-8')
            stdout, stderr = io.StringIO(), io.StringIO()
            with patch.object(gen_docs, 'ROOT', root), \
                    patch.object(gen_docs, 'generated', return_value=generated), \
                    patch('sys.argv', ['gen-docs.py', '--check']), \
                    redirect_stdout(stdout), redirect_stderr(stderr):
                with self.assertRaises(SystemExit) as exit:
                    gen_docs.main()
            self.assertEqual(exit.exception.code, 1)
            # Check mode must leave existing files untouched and missing files absent.
            for name in generated:
                path = root / name
                if name in existing:
                    self.assertEqual(path.read_text(encoding='utf-8'), existing[name])
                else:
                    self.assertFalse(path.exists())
            return stdout.getvalue(), stderr.getvalue()

    def test_every_differing_file_has_a_unified_diff(self):
        stdout, stderr = self.check(
            {'docs/cli.md': 'Usage: old\n'},
            {'docs/cli.md': 'Usage: saccade\n', 'missing.md': 'new\n'})
        self.assertIn('--- docs/cli.md\n+++ docs/cli.md (generated)\n', stdout)
        self.assertIn('-Usage: old\n+Usage: saccade\n', stdout)
        self.assertIn('--- missing.md\n+++ missing.md (generated)\n', stdout)
        self.assertIn('+new\n', stdout)
        self.assertIn('Generated files differ: docs/cli.md, missing.md', stderr)

    def test_large_diff_is_bounded_per_file(self):
        stdout, _ = self.check(
            {'large.md': 'old\n' * 300},
            {'large.md': 'new\n' * 300, 'next.md': 'still shown\n'})
        first, second = stdout.split('--- next.md\n')
        self.assertEqual(len(first.splitlines()), 201)
        self.assertTrue(first.endswith('... diff truncated after 200 lines: large.md\n'))
        self.assertIn('+still shown\n', second)


if __name__ == '__main__':
    unittest.main()
