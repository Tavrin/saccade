#!/usr/bin/env python3
"""Check that every rendering-evidence surface and its limits are documented."""
from pathlib import Path
root = Path(__file__).resolve().parents[1]
doc = (root / 'docs/render-evidence.md').read_text()
for term in ['Required effects', 'Intended experiment variables', 'Structure and texture',
             'Automatic regions', 'Capture layers and scope', 'Fixed-camera temporal stability',
             'Noisy offline references', 'Preregistered blind trials', 'Warmup qualification',
             'trial_plan_changed', 'warmup_not_converged', 'reference_compare', 'trial_import',
             'sample-mean variance', 'Pixel CIs assume independence']:
    assert term in doc, f'Missing documentation: {term}'
guide = (root / 'integrations/agent-guide.md').read_text()
assert '../docs/render-evidence.md' in guide
print('wave9 documentation coverage: PASS')
