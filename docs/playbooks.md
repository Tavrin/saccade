# Playbook acceptance runner

The thirteen W01–W13 recipes have reviewed commands under `playbooks/W*/commands.json`.
Run them against an installed binary, using a new evidence directory:

```sh
python3 scripts/run-playbooks.py --bin "$(command -v saccade)" --out ../playbook-evidence
```

The runner generates all inputs outside the checkout, checks each expected exit,
checks explicit effects, then checks the SHA-256 of each command's declared JSON
projection. A mismatch fails; CI never updates expectations. stdout, stderr,
projections, actual hashes, compiled features and the exact binary SHA-256 are
retained with `saccade-playbook-run.v1` in `result.json`. The manifests, run
receipt and input provenance have schemas under `playbooks/schemas/`. Future
incompatible changes need successor IDs; existing CLI/report schemas are unchanged.

Python 3.10+ is required. Fixtures use its standard library, with no source
imagery or fonts. `inputs/provenance.json` records every generated input hash,
generator identity and MIT OR Apache-2.0 licence. Run directories include source
JSON, strict arm maps/config, a local-edit pair and collateral control, six repeat
sidecars and raw records, a tuning manifest, two-page PDFs, a 24-triangle cube and
12-triangle LOD, and three software-rasterized bound views (front, rear, underside).
The timings and qualification declarations are synthetic acceptance controls;
they do not qualify real hardware, performance or an application. The CPU-only
repeat proof intentionally remains `INCONCLUSIVE`: the current v2 qualification
contract requires GPU/driver identity. The runner asserts this unknown state and
the measured synthetic delta separately; it supplies `--gpu-clocks-not-applicable`.

| Playbook | Bundle/features | Checked workflow |
| --- | --- | --- |
| W01 | default | demo, view, compare, inspect, portable evidence |
| W02 | media/products | local HTTP sweep, real browser capture, missing capture retained |
| W03 | default | declared/undeclared arm change, strict comparison config |
| W04 | default | intended pixels changed, complement preserved, collateral rejected |
| W05 | default source route | lost minus sign and stale image binding rejected |
| W06 | default | recursive external assessment loop, corrupt file retained |
| W07 | media/products | finite JPEG/WebP search and local HTTP delivery audit |
| W08 | default | provenance inspection, local reuse leads |
| W09 | default/graphics | schema discovery, all six sidecars, identity and repeat proof |
| W10 | media/documents | equal two-page PDFs and changed second page |
| W11 | default | numbered frame sequence, generated clip and keyframes |
| W12 | default/mcp | initialize/list tools under declared roots, offline review preview |
| W13 | full/geometry,graphics | distinct ordered mesh identity, complete bound view coverage |

W02 needs Node and `@playwright/test@1.51.1` with its Chromium installed. W11
needs `ffmpeg` and `ffprobe`. W02 and W07 contact only the runner's loopback
server. W05 uses image-bound source observations; optional OCR model provisioning
and inference are separate qualification work. W08 does not claim signed-credential
verification from an unsigned generated PNG. W12 tests startup and the tool surface;
the adversarial authority-boundary harness remains a separate follow-up. No provider
is contacted, no model is downloaded and no baseline is approved.

Missing features or external prerequisites produce `SKIP` with an explicit reason.
`--require-all` fails if any playbook skips and runs all thirteen; `--only W04 W05`
is available for focused development but cannot be combined with that flag.
CI's all-features job executes the complete runner in a clean container. It mounts
only the installed binary, runner/generator helpers, manifests and the browser
adapter, with no repository examples, build caches or user configuration. See
`scripts/playbooks/Dockerfile` and `scripts/run-playbooks-container.sh`.

The hash projection is explicit per command. Relocatable paths, server ports,
wall-clock durations and producer interpreter identity are retained as provenance
but omitted from selected decision projections. Selected numbers are hashed as
reported, without rounding or tolerance. This checks declared acceptance evidence,
not byte identity of every report artifact or native-platform release installation.

When deliberately changing a recipe, a maintainer can use `--record-hashes` in a
fresh directory. Exits and effect assertions still must pass before a manifest is
written. Review the diff and run again without that flag. Never record expectations
to hide a failing control. Generation and checks are additive; the native batch
command and MCP mirrors are outside this runner's scope.
