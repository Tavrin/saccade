# Guide: controlled rendering and strict producers

Use this when the images come from a renderer, an export pipeline or any other
producer where a setting that silently changed (build, input, mode, warm-up)
would make a pixel comparison meaningless. The cross-domain example is a lab
instrument or a report generator: any producer that can write a small JSON
record next to its output. The sample here is a shaded sphere with a settings
record, because the sphere is easy to generate; nothing about it is
renderer-specific.

Run the tested blocks with `scripts/test-guides.py` (inputs from
`scripts/gen-guide-fixtures.py samples`).

## What strict mode compares, and why it refuses

Strict mode (`--require-valid-arms`) compares the producer identity of both sides
before it looks at pixels: producer build, input identity, run mode, readiness.
It refuses a verdict when a field differs and is not declared as an intended
variable, or when a required field is absent. It compares everything the two
records contain unless you give a mapping with `compare = "mapped_only"`, in
which case it compares only the fields the map names. A smaller map therefore
never claims complete setup validation; the check output lists what it covered
(`covered_by_vary`, `ignored`, `unmapped`). Details: [arm validity](../arm-validity.md).

| Exit | Meaning |
| --- | --- |
| 0 | the two records are comparable |
| 3 | a field differs and was not declared as intended |
| 4 | a required field is absent (not null, not different: absent) |
| 2 | input could not be read |

## Known good: the only difference is the declared variable

The two records in `examples/arm-validity` differ only in `settings.mode`.
Declaring it makes the pair comparable:

```sh case=known-good exit=0 says="valid_comparison"
saccade arms check examples/arm-validity/baseline/capture.json examples/arm-validity/candidate/capture.json \
  --fingerprint-map examples/arm-validity/fingerprint-map.toml --vary mode --json
```

## Known bad: an undeclared difference

The same pair without the declaration is refused with exit 3, and the output
names the offending key (`run.mode`):

```sh case=known-bad exit=3 says="run.mode"
saccade arms check examples/arm-validity/baseline/capture.json examples/arm-validity/candidate/capture.json \
  --fingerprint-map examples/arm-validity/fingerprint-map.toml --json
```

## Missing input: the producer did not write its record

An image directory without the producer record cannot pass strict mode. Start
from the strict preset, which turns the check on and waives nothing:

```sh case=missing-input exit=4 says="missing"
saccade init --template producer-strict --dir strict
saccade compare samples/render/parent samples/render/same --config strict/saccade.toml --out strict-report
```

The preset is a plain `saccade.toml` (`require_matching_meta = true`,
`require_valid_arms = true`); open it to see the commented places to declare
intended variables, a mapping file and explicit waivers. Presets do not change
any default: without `--config` or the flags, `compare` keeps its permissive
behavior.

## Known bad, pixels: the declared setup matched but the render changed

Setup validity and pixel difference are separate questions. A dimmer render with
the same settings is comparable and then fails the pixel gate:

```sh case=known-bad exit=1 says="1 fail"
saccade compare samples/render/parent samples/render/dim --out dim-report --threshold 0.01
```

## Known bad, sidecars: the producer settings differ

Producers that write a flat `saccade-meta.json` next to their images (string,
number, bool or null values) can have undeclared differences refused per image
with `--require-matching-meta`; `--declare KEY` marks an intended difference:

```sh case=known-bad exit=1 says="run_mode"
saccade compare samples/render/parent samples/render/other-mode --require-matching-meta --out meta-report
```

## Unavailable dependency: localizing the divergence with a model

Asking a model where a render diverged needs a `local-models` build and a pulled
model; without it the command refuses with exit 2 and names what to install:

```sh case=unavailable-dependency unavailable=local-models exit=2 says="local-models"
saccade locate samples/render/parent/sphere.png "bright highlight"
```
