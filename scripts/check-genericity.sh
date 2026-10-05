#!/usr/bin/env bash
# Check the union of staged/tracked paths, including staged content and worktree content.
set -euo pipefail
exec python3 - "$@" <<'PY'
import pathlib, re, subprocess, sys
root = pathlib.Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip())
default = pathlib.Path.home() / '.config/saccade/denylist.txt'
deny = pathlib.Path(sys.argv[1]).expanduser().resolve() if len(sys.argv) > 1 else default
try:
    deny.relative_to(root)
except ValueError:
    pass
else:
    sys.exit('genericity: denylist must be outside the repository')
if not deny.exists():
    print('genericity: external denylist absent; skipped')
    sys.exit(0)
if deny.stat().st_size > 1024 * 1024:
    sys.exit('genericity: denylist exceeds 1 MiB')
terms = [line.strip().casefold() for line in deny.read_text(encoding='utf-8').splitlines() if line.strip()]
if not terms:
    print('genericity: external denylist empty; skipped')
    sys.exit(0)
files = subprocess.check_output(['git','ls-files','-z','--cached'], cwd=root).split(b'\0')
violations = []
for raw in set(files):
    if not raw:
        continue
    name = raw.decode('utf-8', 'surrogateescape')
    path = root / name
    sources = []
    if path.is_file() and not path.is_symlink():
        sources.append(('worktree', path.read_bytes()))
    staged = subprocess.run(['git','show', ':' + name], cwd=root, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    if staged.returncode == 0:
        sources.append(('index', staged.stdout))
    for source, data in sources:
        # Byte decode preserves readable strings in binary files too; matching
        # terms never get printed (the private denylist remains outside the repo).
        text = data.decode('utf-8', 'replace').casefold()
        for index, term in enumerate(terms, 1):
            if term in text or term in name.casefold():
                violations.append((name, source, index))
                break
if violations:
    for name, source, index in sorted(set(violations)):
        print(f'genericity: {name!r} ({source}) matches denylist line {index}', file=sys.stderr)
    sys.exit(1)
print('genericity: PASS')
PY
