#!/usr/bin/env python3
"""Wave-local docs/schema checks; generated shared CLI docs belong to integration."""
import json
from pathlib import Path
root = Path(__file__).resolve().parent.parent
for schema in ('saccade-general-result.v1', 'saccade-registration.v1', 'saccade-hash.v1', 'saccade-dedupe.v1', 'saccade-similar.v1', 'saccade-embedding-model.v1', 'saccade-embedding-index.v1', 'saccade-embedding-query.v1', 'saccade-text.v1', 'saccade-assess.v1', 'saccade-inspect-image.v1'):
    value = json.loads((root / f'crates/saccade-core/schemas/{schema}.schema.json').read_text())
    assert value['properties']['schema']['const'] == schema
    assert value['$id'].endswith(f'/{schema}.schema.json')
for name in ('registration', 'hashing', 'embeddings', 'text', 'assessment', 'documents', 'inspect-image'):
    assert (root / f'docs/{name}.md').is_file()
assert 'wave6' in (root / 'integrations/agent-guide.md').read_text()
print('Wave 6 docs and schema discriminators checked')
