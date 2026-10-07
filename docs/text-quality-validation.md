# Text-quality generated validation

Implementation: `e12c6adc056a4bde31b48589b754288f70dcd7e4`.
Both commands, generated versioned schemas, docs and CHANGELOG are included.

The finite generated corpus passes all 25 cases. It covers rendered
text, font-generated/injected tofu, actual replacement characters, correctly
rendered Latin/CJK/Arabic negatives, contrast/size/blur variants, dark/scaled
captures, abstention cases, and a multi-region/multi-variant matrix.
Three capture constructions (page-style, control-panel-style, perspective/noise)
use the same commands and options. Defects are independently declared by the
fixture generator; expected states, reasons, exits and coordinates are asserted.
No expected positive may be hidden by an abstention.

False reassurance: **0/11 defective fixture cases (0%)**.
False tofu: **0/3 correctly rendered script fixture cases (0%)**.
These are case-level generated results, not population-rate estimates.

## Final gates

Environment: CARGO_INCREMENTAL=0,
CARGO_PROFILE_DEV_DEBUG=0, CARGO_PROFILE_TEST_DEBUG=0; builds nice -n 19, -j4.

| Command | Exit |
| --- | --- |
| cargo fmt --check | 0 |
| cargo clippy --locked -j4 -p saccade-core -p saccade --all-targets -- -D warnings | 0 |
| cargo test --locked -j4 -p saccade-core -p saccade | 0 |
| python3 scripts/gen-docs.py --check | 0 |
| bash scripts/check-public-hygiene.sh | 0 |
| cargo clippy --locked -j4 -p saccade-core -p saccade --features text-quality,schema --all-targets -- -D warnings | 0 |
| cargo test --locked -j4 -p saccade-core -p saccade --features text-quality,schema | 0 |
| UPDATE_SCHEMAS=1 cargo test --locked -j4 -p saccade-core --features graphics,compression,schema,text-quality,ai,workbench,evaluation --test schemas | 0 |
| python3 scripts/text-quality/fixtures.py --out DIR --binary PATH | 0 |


The command-count assertion and command reference include the two new operations.
Only their generated help sections and command-list/feature entries were added
to the existing reference. The schema drift gate also regenerated the existing
compression-report schema's 12 missing transport-identity property lines, without
changing its Rust implementation. Existing tests remain enabled and detection/
acceptance thresholds were not reduced.

## Tested artifact identity

- Tested Git parent: `95b9460967c1e8bb3f0df29c710f78633440f847`; the binary was built before commits with dirty-checkout metadata. Source contents were checked unchanged after the implementation commit.
- Source snapshot SHA-256: `23ff2f013dea9253f43f7d326ce07b6e698e37e2c8244bf63b9c53381b17bbac`. This hashes sorted relative-path/SHA-256 pairs for both crates' src/assets/schemas/build scripts/Cargo manifests and workspace Cargo.toml/Cargo.lock.
- Tested binary SHA-256: `83c8577eb21d32cf0b53638fc4b10aa84657362a0cfec62cff91a782d6c5a777`.
- Generated fixture manifest SHA-256: `aa62c6dae6b889c52fada56e0efad80b3d4738aee6f2e2022963b8c553b032d1`; it records installed-font hashes/licenses, Pillow version, fixed seed and known answers.
- Compiled features: `ai`, `compression`, `evaluation`, `graphics`, `mcp`, `parallel`, `schema`, `text-quality`, `workbench`.
- The build target was removed after validation. Its last observed size was 6,331,750,341 bytes, below the 8 GB limit.

## Not verified

Cached OCR agreement explicitly skipped: the qualification binary lacks `ocr`.
Imported OCR disagreement and stale-image rejection pass without models. Native
OCR combination/model accuracy was not run. `--all-features` was not run because
system dav1d is absent (`pkg-config --exists dav1d` exited 1). Existing ignored
heavy tests remain ignored. No release, browser, network/provider, GPU,
natural-photo, arbitrary-font/script, human-readability, shaping or compliance
qualification is claimed. MCP mirrors remain a follow-up.
