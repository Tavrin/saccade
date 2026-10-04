#!/usr/bin/env python3
"""Validate generated showcase JSON records against the shipped schema IDs."""
import json
import pathlib
import sys
import jsonschema

root = pathlib.Path(__file__).resolve().parent.parent
report_root = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else root / 'target/showcase-reports'
schemas = {}
for path in (root / 'crates/saccade-core/schemas').glob('*.schema.json'):
    schema = json.loads(path.read_text())
    schemas[path.name.removesuffix('.schema.json')] = jsonschema.validators.validator_for(schema)(schema)
count = 0
for path in sorted(report_root.rglob('*.json')):
    value = json.loads(path.read_text())
    if not isinstance(value, dict) or 'schema' not in value:
        continue
    schema_id = value['schema']
    if schema_id not in schemas:
        raise SystemExit(f'{path}: unknown shipped schema {schema_id}')
    schemas[schema_id].validate(value)
    count += 1
if count == 0:
    raise SystemExit(f'{report_root}: no schema-bearing showcase output')
print(f'validated {count} showcase JSON documents against shipped schemas')
