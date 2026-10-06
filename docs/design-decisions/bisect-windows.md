# Windows Git bisect capture commands

CI run [37244372444](https://github.com/Tavrin/saccade/actions/runs/37244372444)
failed `finds_first_bad_and_restores_original_head` on Windows with exit 2,
one step and no first bad commit. The implementation selects `cmd /C` on
Windows, but the test supplied `cp` and `$SACCADE_CAPTURE_DIR` (and later
`touch` and `;`). Those are Unix shell commands and syntax. Even if `cp` is
on PATH, cmd cannot expand `$SACCADE_CAPTURE_DIR`. A failed capture correctly
returns callback exit 125, which Git treats as a skipped revision; skipping
the only candidate explains the inconclusive result. The parent retains
capture stderr in evidence logs, so empty CLI stderr does not mean capture
succeeded.

Keep Windows support and the test's original intent. The test uses `copy /Y`
and `%SACCADE_CAPTURE_DIR%` on Windows, and the existing Unix tools elsewhere.
Its dirty capture uses cmd redirection and `&` on Windows. Paths with spaces,
a bad commit before the final HEAD, successful good/bad evidence, dirty-clone
abort evidence and an explicit failing capture check the actual behavior
instead of accepting any inconclusive result. The original HEAD, branch and
clean tree must still be preserved.

Use `cmd /D /S /C` with an outer quoted capture string passed through
[`CommandExt::raw_arg`](https://doc.rust-lang.org/std/os/windows/process/trait.CommandExt.html#tymethod.raw_arg).
Regular `Command::arg` uses C-runtime escaping, which cmd does not understand.
[`/S`](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/cmd)
removes the outer quotes while preserving inner shell syntax, and `/D`
disables registry AutoRun commands. The Unix `sh -c` behavior and Git callback
mapping (0 good, 1 bad, 125 skip, 128 abort) remain unchanged. Git directly
invokes the saccade executable; there is no generated script, executable bit
or shebang to repair. Document cmd environment syntax and a PowerShell 7.3+
example, where native argument passing preserves embedded quotes.
