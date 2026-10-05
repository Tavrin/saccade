# Wave 8 progress

| Item | Status | Item commit | Remaining qualification |
|---|---|---|---|
| 8.1 media records | done | 76f2b60 | installed models, provider execution |
| 8.2 Python package | done | a565eb7 | release/manylinux wheel archives |
| 8.3 text/image search | partial | e4b3ac2 | official joint model/tokenizer/export pins and licence; calibration |
| 8.4 deterministic saliency | done | 41ef69a | learned saliency intentionally skipped |
| 8.5 keyframes/video records | done | 288aec2 | external ffmpeg execution |
| 8.6 usage matching | done | 987a1e8 | broader real-world corpus |
| 8.7 local HTTP API/container recipe | done | eac530e | Docker build/smoke |
| 8.8 endpoint adapters | done | 0d27ea5 | live calls intentionally excluded |

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
