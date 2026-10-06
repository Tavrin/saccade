#!/usr/bin/env bash
# Always-on public repository checks, independent of the private client denylist.
set -euo pipefail
exec python3 - <<'PY'
import pathlib, re, subprocess, sys
root = pathlib.Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip())
# Assemble the forbidden literals so the guard also checks its own source.
paths = re.compile(rb'/(?:' + b'home|mnt' + rb')/', re.IGNORECASE)
internal = [b''.join(parts).lower() for parts in (
    (b'Indigo', b'Moose'), (b'Plum', b'Aspen'), (b'opus-', b'perf'),
    (b'moss-', b'heavy'), (b'gpu-', b'lease'), (b'moss-', b'codex'),
    (b'codex-', b'companion'), (b'moss-', b'scratch'), (b'moss-', b'flipdiff-reports'),
)]
notes = re.compile(r'^(?:[^/]*-(?:NOTES|PROGRESS)\.md|INTEGRATION-REPORT[^/]*\.md)$')
files = subprocess.check_output(['git', 'ls-files', '-z', '--cached'], cwd=root).split(b'\0')
violations = set()
for raw in set(files):
    if not raw:
        continue
    name = raw.decode('utf-8', 'surrogateescape')
    path = root / name
    sources = []
    if path.is_symlink():
        sources.append(('worktree symlink', str(path.readlink()).encode()))
    elif path.is_file():
        sources.append(('worktree', path.read_bytes()))
    staged = subprocess.run(['git', 'show', ':' + name], cwd=root,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    if staged.returncode == 0:
        sources.append(('index', staged.stdout))
    for source, data in sources:
        for label, content in [('filename', raw), ('content', data)]:
            lower = content.lower()
            if paths.search(content) or any(term in lower for term in internal):
                violations.add((name, source, label))
        if notes.match(name):
            violations.add((name, source, 'internal process document'))
if violations:
    for name, source, kind in sorted(violations):
        print(f'public-hygiene: {name!r} ({source}): forbidden {kind}', file=sys.stderr)
    sys.exit(1)
print('public-hygiene: PASS')
PY
