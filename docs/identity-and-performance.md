# Identity and performance

```sh
saccade identity examples/baseline examples/baseline --out identity-proof --json
```

Exit 0 establishes native decoded-sample equality across nonempty, completely
paired selected inputs. Dimensions, channel interpretation and sample type must
match. File-byte equality is recorded separately. Entry selection names scope;
masks, tolerance flags and region-only acceptance are rejected.
Non-finite samples can be equal while failing capture validity.
Absent metadata means capture comparability is unknown.

`compare --threshold 0 --metric max` measures zero perceptual error; it is not
native sample identity. The proof applies only to supplied captures.

Graphics builds read `saccade-perf.v2` and historical v1 sidecars. Qualification
includes capture/configuration hashes, source/timer identity, raw samples,
window completeness, hardware conditions, aggregation, attribution and repeat
noise. Missing qualification stays unknown. `qualification.*` fields cannot be
hidden by timing ignores. Illustrative metadata timings are not benchmarks.

The effective change threshold is the maximum of three times repeat range,
two timer quanta, 0.05 ms and 0.5% of baseline frame time. Missing repeat noise or
timer information cannot become zero. Do not add unrelated medians or nested
scope times into a purported measured frame.

`NO-EFFECT` needs image identity, qualified timing, known noise, no changes beyond
the effective frame/pass threshold and no material unresolved terms.
`PERF-ONLY` can describe a named pass change without a frame change.
`IMAGE-CHANGE` proves neither correctness nor speed; missing evidence is
`INCONCLUSIVE`. Unbounded terms block a strong conclusion. Aggregate disjoint
remainders use the materiality floor; nested scopes are not double-counted.

`noise --kind image` writes image-noise evidence in FLIP units.
`noise --kind performance` writes qualified performance noise in ms.
Supplying one kind to the other consumer returns `wrong_noise_kind`.
`experiment ablate --base 'base_r*' --arm 's2=s2_r*'` accepts repeat groups,
excludes incomplete directories with named reasons, and derives a performance
noise floor from complete base repeats. Positional `BASE ARM...` remains
supported. A rejected repeat calibration remains rejected: missing optional
timing terms are listed, and differing configuration hashes require captures
from the same renderer configuration.
For visual comparisons, a passing mean can carry `pass_with_local_change`.
The default requires a hotspot with max FLIP at least 0.5 over at least 16
pixels. Set `hotspot_local_max` and `hotspot_local_min_pixels` in
`saccade.toml` to change these cutoffs. Native decoded-sample inequality is
never described as identical, including when the FLIP score rounds to zero.
Within-arm repeat images are compared to each other. If an arm varies more
than the base, ablation records a nondeterminism validity finding, image hashes,
per-image maximum FLIP and a recapture action. `inspect export ABLATION
--format markdown --out summary.md` includes the finding.
A disabled feature can have identical pixels because it contributed nothing at
that camera. A model cannot qualify timing or establish a speedup.
