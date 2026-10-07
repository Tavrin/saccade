# Guide: visual CI for screenshots

Use this when a pipeline produces screenshots (a web page, a mobile app, a game
menu, a rendered chart) and a build should fail when one changes without
approval. The cross-domain example here is a UI screenshot: a login screen whose
button moved.

Sample inputs come from `python3 scripts/gen-guide-fixtures.py samples` in a
source checkout. Every block below marked with a case runs in
`scripts/test-guides.py`, so the exit codes shown are the tested ones. Exit
codes: 0 ok, 1 regression or claim not proven, 2 could not run, 3 and 4 strict
producer refusals (see `saccade --help`).

## Known good: the screenshot did not change

```sh case=known-good exit=0
saccade compare samples/ui/baseline samples/ui/same --out ok-report
```

Images pair by relative path. Each pair gets a FLIP score (0 to 1, 0 means
identical); a pair fails when its `--metric` value (default `mean`, also `p95`,
`p99`, `max`) is above `--threshold`. Open `ok-report/index.html`, or read
`ok-report/saccade-report.v1.json`.

## Known bad: the button moved

```sh case=known-bad exit=1 says="1 fail"
saccade compare samples/ui/baseline samples/ui/moved --out bad-report --metric max --threshold 0.02
```

Exit 1 means the evidence shows a change, not that the tool broke. Page through
the failures and write the evidence pack that explains where:

```sh case=known-bad exit=0
saccade inspect bad-report/saccade-report.v1.json --status fail --limit 5
saccade inspect evidence bad-report/saccade-report.v1.json --entry login.png --out bad-evidence
```

For a build gate, add `--junit report/junit.xml` and run `saccade compare` as
the failing step. A pass never approves a baseline; approval is a separate,
human-authorized step (`saccade approve --dry-run`).

## Missing input: the capture directory is not there

```sh case=missing-input exit=2 says="no such directory"
saccade compare samples/ui/baseline samples/ui/not-captured --out missing-report
```

Exit 2 means no verdict was produced. A directory that exists but shares no file
names with the baseline exits 1 with "nothing compared": an empty comparison is
never a pass.

## Unavailable dependency: a model-based check on a build without models

Locating a phrase in a screenshot needs a build with `local-models` and a pulled
model. On a build without it the command refuses instead of guessing:

```sh case=unavailable-dependency unavailable=local-models exit=2 says="local-models"
saccade locate samples/ui/baseline/login.png "login button"
```

`saccade doctor` lists which command groups this build can run and which models
are present. The model route is an observation, never a baseline approval; see
[local vision](../wave7.md).

## Next

- Per-region limits and masks: `saccade init --template ui`, then
  [capture policies](../captures.md).
- A producer that must prove its two captures are comparable:
  [controlled rendering](controlled-rendering.md).
