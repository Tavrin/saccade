# Bisect a captured regression

`saccade bisect` runs [Git's `bisect run`](https://git-scm.com/docs/git-bisect)
in a disposable local clone of the current repository. It refuses a dirty
original tree or an existing bisect, requires a known good ancestor and bad
revision, and calls `git bisect reset` in the clone after the search. The
original HEAD remains unchanged. It never searches saccade's history unless
you invoke it in the saccade repository.

```sh
cd my-renderer
saccade bisect --good GOOD_REV --bad BAD_REV \
  --baseline /path/to/known-good-images \
  --capture 'my-renderer-capture --out "$SACCADE_CAPTURE_DIR"' \
  --out ../my-renderer-saccade-bisect --json
```

The capture command runs through `sh -c` on Unix and `cmd /D /S /C` on Windows
in the disposable clone at each selected commit. Use the selected shell's
syntax: `$SACCADE_CAPTURE_DIR` on Unix, `%SACCADE_CAPTURE_DIR%` in Windows cmd.
Windows cmd AutoRun commands are disabled. For example, from PowerShell 7.3+
with its default native argument passing (the single quotes preserve the cmd
capture string):

```powershell
saccade bisect --good GOOD_REV --bad BAD_REV --baseline C:\known-good-images --capture 'copy /Y "frame.png" "%SACCADE_CAPTURE_DIR%\frame.png"' --out ..\my-renderer-saccade-bisect --json
```

The capture still runs in cmd; PowerShell syntax such as
`$env:SACCADE_CAPTURE_DIR` does not apply inside it.
It needs tracked source files and any external tools;
ignored or untracked local files are not copied into the clone.
It must write images to the absolute `SACCADE_CAPTURE_DIR` environment path
and leave the repository clean. Saccade copies the baseline before changing
revisions. The output directory must be new and outside the repository and
baseline; if omitted, it defaults to a sibling directory named after the
repository. Each probed commit gets `steps/<sha>/capture`, `report`, capture
logs and `step.json`. The root `saccade-git-bisect.v1.json` names the first
bad commit, original HEAD and all probed evidence. Exit 1 means a first bad
commit was found, 2 means inconclusive or a usage/restore error.

With `--perf`, a slower measured frame counts as bad only when paired
performance and repeat noise are qualified. Missing or unqualified timing
evidence skips that revision. A failed capture or incomplete image comparison
also skips it (`git bisect run` callback exit 125). A capture command that
changes tracked or untracked repository files aborts the search; inspect its
step logs before another attempt. The original repository is never cleaned
or modified. Git's monotonicity assumption still
applies; skip-heavy results can be inconclusive.
