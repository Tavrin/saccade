# Field friction round 1 decisions

`arms check --out FILE` writes the JSON validation report through the same
linked-report writer as sibling commands, including report-index registration.
It still prints the requested text/JSON summary and returns the validation verdict.
Removing the output promise would leave an unnecessary inconsistency with siblings.

Ablation uses the same plain rank, repeat count (`n`), spread and confidence
interval formatting in Markdown, text and HTML. IQR needs two arm repeats;
CI needs two repeats in both base and arm. A single repeat retains its measured
median and shift, but displays `-` for unavailable spread/CI with an explanation.
The underlying timing evidence and statistical method are unchanged.

An arm whose repeats all have no images remains an `EXCLUDED` row with
`excluded: no images`, no timing rank or acceptance flags, and a recapture action.
Other input failures retain their existing error behavior. Missing baseline
images still prevent measurement. `--require-all-arms` returns exit 1 after
writing the table whenever an arm repeat is excluded. Strict producer identity
checks remain independent and may refuse the command before measurement.

The compatibility GPU-clock warning is emitted once per CLI process, covering
multiple repeat reads and comparisons within one invocation. Its stderr text
is unchanged. Each new invocation emits it again when compatibility decoding
is used. The generated regression fixture takes the existing schema identifier
from the compatibility reader to avoid introducing producer-specific names.

Noise directory discovery continues to refuse symlinks. It reports counts and
the first three paths for each inspected directory on stderr and retains those
diagnostics in successful reports. Empty inputs and the 1024-image bound have
separate errors; neither is presented as a measured noise floor.

Effect masks must be relative to their declaring config/policy file and remain
inside its directory. Absolute paths, parent traversal and escaping symlinks
remain refused; diagnostics and documentation now spell out that rule.

Each item has a focused generated-fixture regression in
`crates/saccade/tests/field_friction.rs`. These tests establish offline CLI and
presentation behavior, not capture conditions or performance qualification.

## Verification

Every final gate exited 0:

| Gate | Exit | Evidence scope |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | Final Rust formatting |
| Default workspace/all-targets clippy with `-D warnings` | 0 | Final default feature configuration |
| Workspace/all-targets clippy with every feature except `imgtune-avif`, `-D warnings` | 0 | Includes optional Python package features |
| Default workspace tests | 0 | 736 passed, 25 ignored |
| Workspace tests with every feature except `imgtune-avif` | 0 | 851 passed, 60 ignored |
| `gen-docs --check --allow-missing-imgtune-avif` | 0 | Regenerated CLI; published all-features header lines 5–8 unchanged |
| Public hygiene and its guard regressions | 0 | Worktree and staged content |
| Default guides | 0 | 32 blocks run, 2 feature-based skips |
| Feature-complete guides | 0 | 28 blocks run, 6 feature-based skips |

The initial feature-complete test invocation exited 101 because system Python
lacked pytest. The final invocation used an available pytest environment via
`SACCADE_W8_PYTHON` and passed, including the native Python package test.
Ignored tests and guide skips remain explicit; no live provider/model, GPU,
release-wheel or capture-condition qualification is claimed.

Builds used the dedicated Cargo target with incremental compilation and dev/test
debug information disabled. Target size checks remained below 10 GB; completed
default test executables were pruned before the feature-complete run. The target
was removed after verification. No push was performed.
