#!/usr/bin/env python3
"""Wave-owned docs/schemas only; generated shared CLI/packs belong to integration."""
import json, pathlib, re
ROOT=pathlib.Path(__file__).resolve().parents[1]
PAGES=['playwright-matcher','sweep','imgtune','design-source','last-good','genericity','wave5-mcp']
for name in PAGES:
    path=ROOT/'docs'/f'{name}.md';text=path.read_text()
    for target in re.findall(r'\]\(([^)]+)\)',text):
        if not target.startswith(('https://','http://','#')):
            assert (path.parent/target.split('#')[0]).exists(),(name,target)
    for block in re.findall(r'```json\n(.*?)\n```',text,re.S):json.loads(block)
assert 'docs/playwright-matcher.md' in (ROOT/'integrations/agent-guide.md').read_text()
for name in ['saccade-playwright-matcher','saccade-sweep','saccade-sweep-captures','saccade-sweep-report','saccade-imgtune','saccade-imgtune-audit','saccade-imgtune-search','saccade-design-map','saccade-design-pull','saccade-design-captures','saccade-design-report','saccade-notification','saccade-notify-result']:
    schema=json.loads((ROOT/'crates/saccade-core/schemas'/f'{name}.v1.schema.json').read_text())
    assert schema['properties']['schema']['const']==f'{name}.v1'
    assert schema['additionalProperties'] is False
print('wave5 docs/schema contracts: PASS')
