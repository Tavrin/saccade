#!/usr/bin/env python3
"""Validate plugin manifests offline. Requires Python 3.11+ and jsonschema."""
import hashlib
import json
import re
import sys
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path

import jsonschema

ROOT = Path(__file__).resolve().parents[1]
SCHEMAS = ROOT / 'scripts/plugin-schemas'
PORTABLE = 'https://agent-plugins.org/schemas/1.0.0/plugin.schema.json'
MCP = 'https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json'
STRING = {'type': 'string', 'minLength': 1}
NAME = {**STRING, 'pattern': '^[a-z0-9]+(?:-[a-z0-9]+)*$'}


def obj(properties, required=None):
    return {'type': 'object', 'properties': properties,
            'required': list(properties) if required is None else required,
            'additionalProperties': False}


# Maintained subsets of documented fields used by these packages, not upstream
# schemas. See plugin-schemas/README.md for the official field references.
AUTHOR = obj({'name': STRING, 'url': {'type': 'string', 'format': 'uri'}}, ['name'])
CLAUDE = obj({
    'name': NAME, 'displayName': STRING, 'version': STRING,
    'description': STRING, 'author': AUTHOR, 'homepage': STRING,
    'repository': STRING, 'license': STRING,
    'keywords': {'type': 'array', 'items': STRING},
})
CLAUDE_MARKET = obj({
    'name': NAME, 'description': STRING, 'owner': AUTHOR,
    'plugins': {'type': 'array', 'minItems': 1, 'items': obj({
        'name': NAME, 'source': STRING, 'description': STRING})},
})
CODEX_MARKET = obj({
    'name': NAME, 'interface': obj({'displayName': STRING}),
    'plugins': {'type': 'array', 'minItems': 1, 'items': obj({
        'name': NAME, 'source': obj({'source': {'const': 'local'}, 'path': STRING}),
        'policy': obj({'installation': {'enum': ['AVAILABLE', 'INSTALLED_BY_DEFAULT', 'NOT_AVAILABLE']},
                       'authentication': {'enum': ['ON_INSTALL', 'ON_USE']}}),
        'category': STRING})},
})
INTERFACE = obj({
    'displayName': {**STRING, 'maxLength': 30},
    'shortDescription': {**STRING, 'maxLength': 30},
    'longDescription': {**STRING, 'maxLength': 4000},
    'developerName': {**STRING, 'maxLength': 80}, 'category': STRING,
    'logo': STRING, 'composerIcon': STRING,
})


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique_keys(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f'duplicate JSON key: {key}')
        result[key] = value
    return result


def read(path):
    return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=unique_keys)


def validate(data, schema):
    cls = jsonschema.validators.validator_for(schema)
    cls.check_schema(schema)
    # No network fallback, even if a future schema accidentally adds a remote ref.
    def offline(uri):
        raise ValueError(f'unvendored schema reference: {uri}')
    resolver = jsonschema.RefResolver.from_schema(
        schema, handlers={'http': offline, 'https': offline})
    cls(schema, resolver=resolver, format_checker=jsonschema.FormatChecker()).validate(data)


def contained(base, relative):
    require(isinstance(relative, str) and relative.startswith('./'),
            f'expected ./-prefixed path: {relative}')
    path = (base / relative).resolve()
    require(path.is_relative_to(base.resolve()) and path != base.resolve(),
            f'path escapes package: {relative}')
    require(path.exists(), f'missing path: {path}')
    return path


def check_svg(path):
    require(path.stat().st_size <= 5 * 1024 * 1024, f'oversized image: {path}')
    svg = ET.parse(path).getroot()
    require(svg.tag == '{http://www.w3.org/2000/svg}svg', f'not SVG: {path}')
    width, height = float(svg.attrib['width']), float(svg.attrib['height'])
    require(width == height and width >= 48, f'icon must be square and at least 48px: {path}')


def main():
    hashes = read(SCHEMAS / 'sources.json')
    schemas = {}
    for name, source in hashes.items():
        path = SCHEMAS / name
        require(hashlib.sha256(path.read_bytes()).hexdigest() == source['sha256'],
                f'vendored schema changed: {name}; review source and update sources.json')
        schemas[source['url']] = read(path)
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']
    files = {
        'integrations/claude-code/.claude-plugin/plugin.json': CLAUDE,
        '.claude-plugin/marketplace.json': CLAUDE_MARKET,
        'integrations/codex/plugin.json': schemas[PORTABLE],
        '.agents/plugins/marketplace.json': CODEX_MARKET,
        'server.json': schemas[MCP],
    }
    data = {}
    for name, schema in files.items():
        data[name] = read(ROOT / name)
        try:
            validate(data[name], schema)
        except jsonschema.ValidationError as error:
            raise ValueError(f'{name}: {error.message}') from error
        print(f'PASS {name}')
    # Do not silently leave a newly added package/catalog JSON unchecked.
    discovered = {str(p.relative_to(ROOT)) for folder in
                  ['.agents/plugins', '.claude-plugin', 'integrations/claude-code', 'integrations/codex']
                  for p in (ROOT / folder).rglob('*.json')} | {'server.json'}
    require(discovered == set(files), f'update validator coverage: {discovered ^ set(files)}')
    claude = data['integrations/claude-code/.claude-plugin/plugin.json']
    portable = data['integrations/codex/plugin.json']
    for manifest in [claude, portable]:
        require(manifest['version'] == version, 'plugin/workspace version mismatch')
        require(manifest['name'] == 'saccade', 'plugin identity mismatch')
        require(manifest['license'] == 'MIT OR Apache-2.0', 'license mismatch')
    require(portable['$schema'] == PORTABLE, 'portable schema mismatch')
    for name, expected in [('.claude-plugin/marketplace.json', 'integrations/claude-code'),
                           ('.agents/plugins/marketplace.json', 'integrations/codex')]:
        market = data[name]
        require(market['name'] == 'saccade', f'{name}: marketplace name mismatch')
        require(len(market['plugins']) == 1, f'{name}: unexpected plugin inventory')
        entry = market['plugins'][0]
        source = entry['source']
        path = contained(ROOT, source if isinstance(source, str) else source['path'])
        require(path == ROOT / expected and entry['name'] == 'saccade', f'{name}: wrong plugin target')
    interface = portable['extensions']['com.openai']['interface']
    validate(interface, INTERFACE)
    for key in ['logo', 'composerIcon']:
        check_svg(contained(ROOT / 'integrations/codex', interface[key]))
    for package, names in [('claude-code', ['saccade']), ('codex', ['saccade', 'check-visual-change'])]:
        base = ROOT / 'integrations' / package
        require(len((base / 'README.md').read_text().split()) >= 40, f'{package}: README too short')
        for name in names:
            text = (base / 'skills' / name / 'SKILL.md').read_text()
            require(text.startswith('---\n'), f'{package}/{name}: missing frontmatter')
            frontmatter = text.split('---\n', 2)[1]
            require(f'name: {name}\n' in frontmatter and re.search(r'^description: .+', frontmatter, re.M),
                    f'{package}/{name}: missing skill name/description')
    for name in ['saccade', 'check-visual-change']:
        require((ROOT / f'integrations/claude-code/commands/{name}.md').is_file(), f'missing command: {name}')
    server = data['server.json']
    require(server['$schema'] == MCP and server['name'] == 'io.github.Tavrin/saccade', 'registry identity mismatch')
    require(server['version'] == version and len(server['packages']) == 1, 'registry version/package mismatch')
    package = server['packages'][0]
    require(package['registryType'] == 'cargo' and package['identifier'] == 'saccade'
            and package['version'] == version and package['transport'] == {'type': 'stdio'},
            'registry must describe the versioned Cargo stdio binary')
    require('runtimeHint' not in package and 'runtimeArguments' not in package and '_meta' not in server,
            'unexpected runtime or stale registry draft metadata')
    args = package['packageArguments']
    require(len(args) == 3 and args[0] == {'type': 'positional', 'value': 'mcp'}, 'wrong registry invocation')
    for arg, flag in zip(args[1:], ['--root', '--out-root']):
        require(arg['type'] == 'named' and arg['name'] == flag and arg['isRequired'] is True
                and arg['format'] == 'filepath' and 'value' not in arg and 'default' not in arg,
                f'{flag} must be supplied by the user')
    crate = ROOT / 'crates/saccade'
    metadata = tomllib.loads((crate / 'Cargo.toml').read_text())['package']
    readme = (crate / metadata['readme']).read_text()
    visible = re.sub(r'<!--.*?-->', '', readme, flags=re.S)
    require(f'mcp-name: {server["name"]}' in visible, 'crate README lacks visible ownership marker')
    print('PASS package paths, skills, images, versions, MCP roots and ownership marker')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, ET.ParseError,
            jsonschema.SchemaError, jsonschema.ValidationError) as error:
        sys.exit(f'FAIL {error}')
