# Preserve critical text

Use a frozen region/string policy alongside a global image gate. The generated
pack needs no OCR model and uses image-bound source text on a stock build.
[Contract, policy choices and limits](../critical-text.md).

Unchanged source and pixels pass every declared region:

```sh case=known-good exit=0 says='"state":"pass"'
saccade critical-text testdata/critical-text/app-decimal/baseline.png testdata/critical-text/app-decimal/same.png --policy testdata/critical-text/app-decimal/policy.json --a-source testdata/critical-text/app-decimal/baseline-source.json --b-source testdata/critical-text/app-decimal/same-source.json --json
```

The lost decimal fails independently of a full-image average:

```sh case=known-bad exit=1 says=critical_string_mismatch
saccade critical-text testdata/critical-text/app-decimal/baseline.png testdata/critical-text/app-decimal/edited.png --policy testdata/critical-text/app-decimal/policy.json --a-source testdata/critical-text/app-decimal/baseline-source.json --b-source testdata/critical-text/app-decimal/edited-source.json --json
```

Missing images are execution errors:

```sh case=missing-input exit=2
saccade critical-text absent.png testdata/critical-text/app-decimal/same.png --policy testdata/critical-text/app-decimal/policy.json --json
```

Unavailable text observations prevent a pass even on identical pixels:

```sh case=unavailable-dependency exit=4 says=insufficient_evidence
saccade critical-text testdata/critical-text/app-decimal/baseline.png testdata/critical-text/app-decimal/same.png --policy testdata/critical-text/app-decimal/policy.json --json
```

Read all region reasons and treat every nonzero exit as a non-pass. Source text
must match the actual capture; a hash cannot certify truthful source export.
