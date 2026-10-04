# Identity and performance

```sh
saccade identity examples/baseline examples/baseline --out identity-proof --json
```

Exit 0 establishes native decoded-sample equality across nonempty, completely
paired selected inputs. Dimensions, channel interpretation and sample type must
match. File-byte equality is recorded separately. Selecting entries sets the scope;
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
timer information cannot become zero. Do not add up unrelated medians or nested
scope times and present the sum as a measured frame.

## GPU clock sidecar

Place `gpu_clock.json` beside `saccade-perf.json`. Generic producers write
`saccade-gpu-clock.v1`:

```json
{"schema":"saccade-gpu-clock.v1","device_id":"GPU-1","power_state":"ac-performance","windows":[{"name":"frame","core_mhz":{"min":1800,"median":1800,"max":1800},"memory_mhz":null,"sample_count":32,"expected_frames":8,"observed_frames":8,"query_failures":0,"throttle_reasons":[],"stabilized":true}]}
```

Every window needs samples, complete frames, zero query failures, no throttle
reason, an established stable state, and a core/memory clock range no wider
than 5% of its median. A concrete power state is required; `unknown` does not
qualify. Pairing compares device, power state, named windows and median clocks
exactly. Missing clock evidence rejects a qualified performance claim unless
the user passes `--gpu-clocks-not-applicable`.
Moss `moss.gpu-clock.v2` files are read directly: `sm_mhz.p50` becomes core
median, the sample windows and throttle mask are retained, and
`warm_to_boost.met` supplies stabilization. Moss does not record an explicit
power state in this sidecar, so those captures are unqualified unless a
producer supplies the generic sidecar with that fact. Clock reasons are
separate from image thresholds and repeat noise.

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
from the same renderer configuration. If `--perf-noise` supplies an explicit
floor, ablation reports both that floor and the derived floor and uses the
stricter spread. `--perf-noise-override` records an intentional choice to use
the explicit floor alone.
For visual comparisons, a passing mean can carry `pass_with_local_change`.
The default requires a connected component in the full error map with FLIP
at least 0.5 over at least 16
pixels. Set `hotspot_local_max` and `hotspot_local_min_pixels` in
`saccade.toml` to change these cutoffs. Native decoded-sample inequality is
never described as identical, including when the FLIP score rounds to zero.
Within-arm repeat images are compared to each other. If an arm varies more
than the base, ablation records a nondeterminism validity finding, image hashes,
per-image maximum FLIP and a recapture action. Its flag and combined verdict
become inconclusive and the command exits nonzero. `inspect export ABLATION
--format markdown --out summary.md` includes the finding.
A disabled feature can have identical pixels because it contributed nothing at
that camera. A model cannot qualify timing or establish a speedup.
