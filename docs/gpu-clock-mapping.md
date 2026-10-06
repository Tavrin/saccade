# GPU telemetry mapping

Write native `saccade-gpu-clock.v1` evidence, or map a producer’s JSON
`gpu_clock.json` with `--gpu-clock-map FILE` (TOML or JSON). Maps take precedence
over the source schema ID. The [example](../examples/gpu-clock/map.toml) maps
sample windows from a rendering or scientific visualization pipeline.

```sh
saccade compare before after --gpu-clock-map examples/gpu-clock/map.toml
saccade noise before repeat --kind performance --gpu-clock-map examples/gpu-clock/map.toml --out noise.json
```

Configuration uses `gpu_clock_map = "clock-map.toml"`. This path resolves
relative to the config directory; CLI paths resolve relative to the working
directory. MCP measurement tools accept `gpu_clock_map` within registered roots.

`fields` maps `device_id` and optional `power_state`. `windows.path` selects
an array; `windows.fields` maps each item to the window fields in
[saccade-gpu-clock.v1](../crates/saccade-core/schemas/saccade-gpu-clock.v1.schema.json).
Destinations can be a complete clock range (`core_mhz`, `memory_mhz`) or its
`min`, `median`, `max` members. Conflicting destinations are rejected.

Each source has `path`: dotted object keys and zero-based array indices,
for example `devices.0.uuid`. Exact flat keys take precedence, as in arm
fingerprint maps. Wildcards are rejected. `fallback` lists alternative paths
in order; `default` supplies an explicit output value only when all paths are
absent. Invalid present values fail rather than taking a default.

Optional `transform` is `value` (default), `first_string` (first string among
path and fallbacks, ignoring absent or non-string candidates), `count` (array length), `sum`
(unsigned integers selected by `item_path` in each array item, with overflow
rejection), or `mask` (unsigned integer, zero becomes `[]`, otherwise one
reason prefixed by `prefix`, default `Throttle mask `). No executable code is
accepted. Mapping files are bounded to 1 MiB and source records to 16 MiB, with at most
16 top-level fields, 32 window fields and 16 fallbacks per source.

The example intentionally does not invent a power state. To map a producer’s
measured power state, add:

```toml
[fields.power_state]
path = "device.power_state"
```

For a photo pipeline with a native-shaped measurement embedded in a job record,
a JSON map can use `fields.device_id.path = "jobs.0.device"`,
`windows.path = "jobs.0.measurements"`, and identity mappings for each window
field. Any producer can use these interfaces without changes to Saccade.

Legacy automatic telemetry adaptation warns during 0.2.x and is removed in
0.3.0. Supply a map to migrate. Absent or unknown power state continues to
reject performance qualification; mapping never changes image thresholds.
