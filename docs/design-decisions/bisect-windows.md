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

## Local verification

All Cargo commands set `CARGO_BUILD_RUSTC_WRAPPER` and `RUSTC_WRAPPER` empty and use
`CARGO_TARGET_DIR=/mnt/linux-extra/moss-cargo-targets/codex-saccade-bisect`.

| Gate | Result |
| --- | --- |
| `cargo test -p saccade --test git_bisect` | Passed on Linux, 1 test |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings` | Blocked in `ring`: missing `x86_64-w64-mingw32-gcc` |
| `cargo fmt --check` | Passed |
| `cargo check --workspace --all-targets --target x86_64-pc-windows-gnu --no-default-features` | Passed with the existing unused-variable warnings below |

A supplementary Windows Clippy run with `--no-default-features` reached
saccade but failed on existing unused `out`, `absolute` and `user_config`
parameters in `local_cmd::preview`. These unrelated warnings are outside this
fix. Native Windows runtime acceptance remains pending CI; cross compilation
does not establish runtime behavior. The named target directory is removed
after verification.
