# Product MCP operations

Build with `products,mcp`. The additive `saccade_products` tool mirrors the Wave 5
CLI under the existing read-root/write-root registry. It returns a bounded
`saccade-result.v2` envelope with a full artifact reference. HTTP operations need
human startup `--allow-product-network`; webhook calls separately need
`--allow-webhook-notifications`. Tool/project arguments cannot enable either.
These flags grant product egress from all registered roots; AI review retains its
independent existing provider budget/root policy. Neither enables baseline writes.

| Operation | Required arguments | Output |
| --- | --- | --- |
| `sweep_plan` | `artifact` (URL list), `before_origin`, `after_origin`, `out` | manifest JSON |
| `sweep_compare` | `artifact` (manifest), `captures`, `out` | directory |
| `last_good_compare` | `captures` (directory), `history_store`, `out` | directory |
| `imgtune_audit` | `artifact` (URL list), `accept` (array), `out` | JSON file |
| `imgtune_search` | `artifact` (tuning manifest), `out` | JSON file |
| `design_pull` | `artifact` (mapping), `out`, `cache` | directory |
| `design_compare` | `artifact` (mapping), `pull`, `captures`, `out` | directory |
| `notify` | `artifact` (report) | delivery result |

Optional plan settings: `seed`, `samples`, `viewports` (`[[width,height]]`).
Comparisons accept `config`. Sweep comparison optionally takes `baseline:last-good`
and `history_store`. Design pull optionally takes `fixture_dir`, `scale`; recorded
fixtures require no network authority. Design compare accepts `align:none|translation`.
Notify accepts `template:generic|slack|teams` and a display-only `report_link`.
Provider credentials/endpoints come from the same user env-file conventions as CLI.

Nested capture/image/config/fixture paths are validated against registered roots;
last-good validates history objects, origins and retained capture paths. Generated
cache/output paths require out-root. A denied operation fails before HTTP execution.
MCP sweep planning takes a URL-list artifact; sitemap expansion stays in the CLI
planner, whose resulting URL-list/manifest can be supplied to MCP. This is a
recorded transport limitation, avoiding uncontrolled transitive sitemap file reads.
