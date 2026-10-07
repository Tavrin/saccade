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
    def generate(self, features, allow_missing_imgtune_avif=False):
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
            return gen_docs.generated('saccade', allow_missing_imgtune_avif)

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
