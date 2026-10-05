# Wave 8 decisions and read-only scout

Starting branch `feat/wave8`, HEAD `3bd1bf5`; clean worktree. `git log -n 15`
and the complete integration report were read before edits. Waves 4–7 are integrated;
inherited model accuracy/parity and Rust OCR qualification remain explicitly incomplete.

## Scout map (before implementation)
- `general/input.rs:9`: bounded encoded reads and SDR decode.
- `general/integrity.rs:257`, `metadata.rs:6`: EXIF/XMP/IPTC/ICC container extraction.
- `general/assessment.rs:46`, `hashing.rs:42`: deterministic quality and hashes.
- `general/registration.rs:447`: FAST/oriented BRIEF + RANSAC, reference-to-target transform.
- `general/embedding.rs:178`: optional lazy ONNX image embeddings.
- `wave7/faces.rs:82,198`: detector and crop checks; `runtime.rs:27`: pinned ONNX sessions.
- `wave7/models.rs:313`: official pinned registry; `runtime_install.rs`: runtime provisioning.
- `wave7/observation.rs:244`: generic description observation boundary.
- `main.rs:612,1628`: existing loopback viewer; `mcp.rs:1076,2270`: additive operations.
- `wave7/providers.rs:156`: GPT Responses fixture mapping (no live transport).

## Resource and scope decisions
- One agent, one Cargo command at a time, nice 19, jobs 4, owned wave8 target,
  incremental/debug info disabled. No model/provider/browser/GPU work during development.
- The shared development rule forbids release builds, so local maturin release build is
  wired into the heavy gate; a development wheel and light pytest are the local goal.
- Do not claim learned saliency or text-image calibration from deterministic fixtures.
- Records are built from one retained encoded buffer. Optional unavailability is explicit;
  description is off by default and requires an injected observation provider.
- Rust structs own library contracts; CLI, MCP, Python and HTTP delegate to them.
  Reversal cost: additive APIs/schemas only; no legacy contract migration.

- Disk crossed below floor after the first targeted test completed: 22 GB available. Cargo paused; continuing source/tests/docs. First media test receipt: 4 passed, 110 filtered out, no models, no downloads.
