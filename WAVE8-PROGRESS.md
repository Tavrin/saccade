# Wave 8 progress

| Item | Status | Item commit | Remaining qualification |
|---|---|---|---|
| 8.1 media records | done | 76f2b60 | combined model gate blocked by pinned Rust OCR; live providers excluded |
| 8.2 Python package | done | a565eb7; 217b6b3 | local release abi3 wheel installed/tested; dual-architecture manylinux_2_28 CI |
| 8.3 text/image search | done (bounded CPU proof) | dcd1acc; f47874f; 217b6b3 | official checkpoint/tokenizer licence and hashes, export parity, Rust/Python/CLI retrieval PASS; broad calibration deferred |
| 8.4 deterministic saliency | done | 41ef69a | learned saliency intentionally skipped |
| 8.5 keyframes/video records | done | 288aec2 | installed FFmpeg gate PASS; broader media corpus |
| 8.6 usage matching | done | 987a1e8 | broader real-world corpus |
| 8.7 local HTTP API/container recipe | done | eac530e | Docker build/health smoke PASS; deployment qualification remains |
| 8.8 endpoint adapters | done | 0d27ea5 | live calls intentionally excluded |

## Development handoff (historical)

Final corrections and qualification wiring: `b1f753d` (strict Clippy and focused tests PASS). `WAVE8-NOTES.md`
records decisions and reversal costs. `scripts/gates-wave8.sh` is coordinator-owned and
has not been run; it explicitly fails the deferred text-image model gate.

Light checks PASS: formatting/docs/shell syntax, model-feature CLI/core/Python check and
strict Clippy; 15 generated core media tests (1 ffmpeg ignored), 3 CLI/MCP schema
fixtures, and 3 light pytest tests against the compiled abi3 extension. Exact receipts
and cleanup are recorded in the closing handoff and lane milestones.

Resource history: builds paused when free disk crossed below 25 GiB (22/20/17 GB
observations), source work continued, builds resumed after recovery above 43 GiB. One
Cargo command at a time, jobs 4, nice 19. No release build, full suite, model download,
model inference, ffmpeg, browser, GPU, Docker or live provider execution in development.

Owner documentation: add README/CHANGELOG entries for media records, Python, keyframes,
usage matching, local API and endpoint configuration; regenerate docs/cli.md and generated
schema/agent documentation through existing owner workflows. These shared generated and
release files were intentionally not changed.

Cleanup complete: exact owned target removed under lane/Cargo locks; free disk
51.85 -> 53.93 GiB. Retained abi3 package, source/binary hashes and command logs:
`/mnt/linux-extra/moss-scratch/saccade-wave8/FINAL-RECEIPT.json`. No build after cleanup.

## Integration completion

Main merged at `555a4ec`; all implementation and corrective gate commits are local.
SigLIP2 official immutable Apache-2.0 checkpoint/tokenizer, reproducible pinned
local CPU exports, unchanged parity tolerance, installed Rust/Python/CLI text
retrieval and tokenizer identity rejection PASS. Scores remain uncalibrated.
All-features CLI/schema/agent docs regenerated; README equals merged main.

Wave 8 aggregate remains exit 1 for the combined installed-model OCR failure;
corrective local release wheel/install and joint Python proof PASS. Full core,
CLI, workspace, feature-matrix and MSRV checks PASS. FFmpeg and Docker health
smoke PASS. Private-denylist genericity PASS. Package inventory/notices and strict
lint PASS. Retained all-features debug CLI reproduces showcases and validates
40 JSON records. Full Cargo package verification and release CLI build were
refused below the 25 GiB floor; they require shared disk recovery, along with
native-platform and dual-architecture release CI. No live providers ran.

Exact integration target removed under locks; no build after cleanup. Tested
source `ac4ff02`, source/binary/wheel/archive hashes, command logs, model licence
and export receipts, milestones and cleanup are retained at
`/mnt/linux-extra/moss-scratch/saccade-integ-w8/FINAL-RECEIPT.json`.
See the wave 8 section of `INTEGRATION-REPORT.md` for all failures, corrections,
resource stops and remaining qualification. No push or other-worktree changes.
