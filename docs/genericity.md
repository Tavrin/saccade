# Genericity guard

`scripts/check-genericity.sh` checks staged and tracked file names/content against
`~/.config/saccade/denylist.txt` (one literal case-insensitive term per line).
The denylist stays outside the repository. An absent/empty file prints a skip
notice and exits 0; a match exits 1, identifying only the file, source and private
list line number. Private terms are never printed or added to the repo.

An optional first argument selects another external denylist path. A path inside
the repository is refused, including resolved symlink aliases. Both index and
worktree contents are checked so an unstaged edit cannot conceal staged material.
Readable terms in binary files are checked too. The guard never modifies git.
Generated temporary-repository tests: `python3 scripts/test-genericity.py`.
