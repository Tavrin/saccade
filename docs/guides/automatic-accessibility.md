# Automatic image accessibility

Use a `prechecks` build for offline image accessibility candidates. These are
pixel pre-checks, never complete detection, certification or DOM accessibility.

A completed blank capture reports UNMEASURABLE and explicitly no text detected:

```sh case=known-good requires=prechecks exit=0 says="no text detected"
saccade a11y auto samples/a11y/blank.png --out blank-report
```

A known low-contrast boundary fails the candidate SC 1.4.11 measurement:

```sh case=known-bad requires=prechecks exit=1 says="FAIL"
saccade a11y auto samples/a11y/low-contrast.png --out bad-report
```

Missing input cannot become a completed result:

```sh case=missing-input requires=prechecks exit=2
saccade a11y auto absent.png --out absent-report
```

An absent model cache remains explicitly unavailable; the free fallback runs:

```sh case=unavailable-dependency requires=prechecks exit=0 says="unavailable"
saccade a11y auto samples/a11y/blank.png --model-cache absent-cache --out fallback-report
```

Inspect detector availability, provenance, confidence and each check's
PASS/FAIL/WARN/UNMEASURABLE. Large text is a body-height/display-scale assumption,
and absent tofu candidates do not establish complete glyph coverage. See
[pre-check limits](../safety-a11y.md) and the `saccade-a11y` library for typed results.
