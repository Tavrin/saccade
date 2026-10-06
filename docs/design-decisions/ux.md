# UX polish before open-source release

## CLI

1. Help follows one layout: purpose line, usage, then a short explanation and
   examples (most common first) and exit codes, then flags grouped under
   Output, Gate, Selection, HDR images, Metadata sidecars, Performance, Review
   context and Global options. A shared `HELP_TEMPLATE` places the examples
   before the flags. Rejected alternative: clap's default (examples after a long
   flag list) and `long_about`, which switches `--help` to a much longer
   one-flag-per-paragraph layout. Reversal: free.
2. Top-level commands are listed by use (`demo`, `compare`, `identity`, `view`,
   `approve`, `serve`, ...) with `display_order`. The enum order is unchanged, so
   the `inspect capabilities` operations array keeps its order.
3. The broken summaries of `serve` and `mcp` (fragments of an old doc comment)
   were rewritten. `identity --threshold/--metric` and `approve --force` are
   hidden from help; they still exist and still return their targeted errors.
   `view --key-out` help now states it is required with `--blind`, matching the
   code.
4. `compare` prints a result line first (`compare: ❌ regression: 1 fail, 1
   missing, 1 new of 6 images`), like `identity` already did, then the table,
   then `report:` (the HTML page) and `next:` (the concrete next step, with an
   `approve --dry-run` command for regressions).
5. Warnings shared by every compared pair (in practice the absent-provenance
   warnings) are printed once under "Shared by all N compared pairs" instead of
   under every row. Stderr warnings are unchanged because `provenance-warnings`
   is a named script-facing capability.
6. Colour: the status word, the header and the result line are coloured only
   when stdout is a terminal and `NO_COLOR` is unset or empty. Every colour
   accompanies a word, so piped and `NO_COLOR` output is identical to before in
   meaning. clap usage errors keep clap's own colour handling.
7. Errors print `saccade: error: <what and why>` then `fix: <repair>`. The hint
   no longer repeats the message, and clap usage errors are printed once
   without a second copy appended as a hint. `CliError::hint` is not part of
   the JSON error envelope, so `--json` errors are unchanged. Two messages were
   made specific: a missing input directory (`nope: no such directory`, or
   "this is a file" for a file) and `view` on a directory that is not a report.
8. The demo footer says why exit 1 is expected, lists the three reports with
   correct paths (the identity and review paths were printed relative to the
   demo directory instead of the working directory) and ends with the command
   for the user's own images. `view REPORT_DIR` prints "Open this page in a
   browser" instead of `view: complete / artifact:`; `--json` is unchanged.

## HTML report and workbench

9. The report summary card starts with a verdict banner (fail, pass, nothing
   compared) computed from the same totals and `is_regression()` as the exit
   code. "Reviewer exposure" moved below the summary so the result is the
   first thing on the page.
10. A visible shortcut hint (`[ ]` previous/next, `?` all shortcuts; `Space`
    flicker in the viewer) sits in the report toolbar and the viewer set bar,
    hidden at phone width and on coarse pointers. A one-line hint says a row
    opens the swipe/flicker/heatmap stage. Rejected: auto-opening the first
    failing entry, because it would change the initial `window.saccade` state
    and URL hash that agents read. Reversal: cheap.
11. Empty states: "No failures: all N entries passed. Choose All to list them."
    and a distinct message for a run with no entries.
12. File names get line-break opportunities after `/ _ . -` (`UI.breakable`),
    and the phone status column is wide enough for the "missing" chip, so names
    no longer break mid-word or collide with the chip at 390 px.
13. Contrast: `--faint` raised to at least 4.8:1 on every surface colour in
    both themes (it was 3.2-4.1:1);
    zero-count status chips are outlined in the muted colour instead of 45 %
    opacity, which put their text below AA.
14. Large runs: table rows are cached per entry and open state, so opening an
    entry rebuilds one row and its detail instead of every row. On an 800-entry
    run (headless Chromium, this machine) opening an entry went from about
    415-487 ms to 310-347 ms; first table paint is about 200-320 ms.
15. Workbench: run rows in the narrow rail let actions wrap below the name
    instead of squeezing the path to one character per line; card rows keep
    their natural height; "1 images" became "1 image".
