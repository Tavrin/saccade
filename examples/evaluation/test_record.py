"""Offline checks for the paid-tier aggregate cost record."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


RECORD = Path(__file__).resolve().parents[2] / 'scripts/evaluation/record.py'
spec = importlib.util.spec_from_file_location('evaluation_record', RECORD)
record = importlib.util.module_from_spec(spec)
spec.loader.exec_module(record)


class PaidSpend(unittest.TestCase):
    def test_usage_includes_thinking_tokens_and_reports_missing_usage(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for index, usage in enumerate([{'promptTokenCount': 1000, 'totalTokenCount': 1500}, {}]):
                folder = root/'live'/'job'/f'attempt-{index}'
                folder.mkdir(parents=True)
                (folder/'exchange.json').write_text(json.dumps({'model': 'gemini-3.8-flash',
                                                                  'outcome': 'received'}))
                (folder/'response.json').write_text(json.dumps({'usageMetadata': usage}))
            spend = record.gemini_spend(root)
            self.assertEqual(spend['responses_without_usage'], 1)
            self.assertEqual(spend['by_model'][0]['input_tokens'], 1000)
            self.assertEqual(spend['by_model'][0]['output_tokens'], 500)
            self.assertEqual(spend['estimated_usd'], 0.002625)


if __name__ == '__main__':
    unittest.main()
