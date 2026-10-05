#!/usr/bin/env python3
"""Wave-scoped documentation/schema checks without regenerating shared CLI docs."""
import json
from pathlib import Path
root=Path(__file__).resolve().parents[2]
for name in ["assist","assist-qualification"]:
    page=root/"docs"/(name+".md")
    if not page.is_file():raise SystemExit("Missing docs: "+str(page))
assist=(root/"docs/assist.md").read_text()
for command in ["review explain","review audit-mask","review check-ui","review assist batch submit","review assist batch status","review assist batch collect","--jev-routing"]:
    if command not in assist:raise SystemExit("Undocumented command: "+command)
for name in ["saccade-assist.v1","saccade-assist-batch.v1","saccade-assist-batch-plan.v1","saccade-constructed-truth.v1"]:
    schema=json.loads((root/"crates/saccade-core/schemas"/(name+".schema.json")).read_text())
    if not schema["$id"].endswith(name+".schema.json"):raise SystemExit("Schema identity mismatch")
guide=(root/"integrations/agent-guide.md").read_text()
if "<!-- wave4 -->" not in guide or "--experimental" not in guide:raise SystemExit("Missing agent guidance")
print("Wave 4 docs and schema references pass; shared generated packs/CLI reference are coordinator-owned.")
