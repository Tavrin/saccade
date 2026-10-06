#!/usr/bin/env bash
# Always-on public repository checks, independent of the private client denylist.
set -euo pipefail
exec python3 - <<'PY'
import hashlib, json, pathlib, re, subprocess, sys
root = pathlib.Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip())
# Assemble the forbidden literals so the guard also checks its own source.
paths = re.compile(rb'/(?:' + b'home|mnt' + rb')/', re.IGNORECASE)
internal = [b''.join(parts).lower() for parts in (
    (b'Indigo', b'Moose'), (b'Plum', b'Aspen'), (b'opus-', b'perf'),
    (b'mo', b'ss-', b'heavy'), (b'gpu-', b'lease'), (b'mo', b'ss-', b'codex'),
    (b'codex-', b'companion'), (b'mo', b'ss-', b'scratch'), (b'mo', b'ss-', b'flipdiff-reports'),
    (b'mo', b'ss'), (b'coo', b'ker'), (b'probe-', b'cache'),
    (b'probe_', b'cache'), (b'receiver_', b'ready'),
    (b'bi', b'stro'), (b'spon', b'za'), (b'manhat', b'tan'), (b'li', b'01'),
)]
notes = re.compile(r'^(?:[^/]*-(?:NOTES|PROGRESS)\.md|INTEGRATION-REPORT[^/]*\.md)$')
files = subprocess.check_output(['git', 'ls-files', '-z', '--cached'], cwd=root).split(b'\0')
violations = set()
allow_name = 'scripts/public-hygiene-allowlist.json'
def allowed_lines(source):
    if source == 'index':
        result = subprocess.run(['git', 'show', ':' + allow_name], cwd=root,
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        data = result.stdout if result.returncode == 0 else b'[]'
    else:
        path = root / allow_name
        data = path.read_bytes() if path.is_file() and not path.is_symlink() else b'[]'
    entries = json.loads(data)
    if not isinstance(entries, list) or len(entries) > 16:
        sys.exit('public-hygiene: invalid allowlist')
    allowed = set()
    for entry in entries:
        if set(entry) != {'path', 'sha256'} or not re.fullmatch('[0-9a-f]{64}', entry['sha256']):
            sys.exit('public-hygiene: invalid allowlist entry')
        allowed.add((entry['path'], entry['sha256']))
    return allowed
allow = {source: allowed_lines(source) for source in ('worktree', 'index')}

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
        # Exact whole-line hashes keep exceptions from admitting other text.
        if name != allow_name:
            data = b'\n'.join(line for line in data.split(b'\n')
                              if (name, hashlib.sha256(line).hexdigest()) not in allow.get(source, set()))
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
