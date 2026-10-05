# Contributing

Bug reports, feature requests and pull requests are welcome.

## Build

You need Rust 1.89 or newer. FLIP is implemented by the published pure-Rust
`flip-rs` dependency. No private dependency checkout or deploy key is needed.

```sh
git clone https://github.com/Tavrin/saccade
cd saccade
cargo build --workspace
cargo run -p saccade -- compare examples/baseline examples/capture --out target/contributor-report
```

The workspace has two crates: `saccade-core` (comparison, report model, HTML
and Markdown rendering, viewer) and `saccade` (the CLI). The HTML assets are in
`crates/saccade-core/assets/` and are embedded into the binary. See
[docs/design.md](docs/design.md) for the formats.

## Test and check

CI runs these on Linux, macOS and Windows, and a pull request must pass them:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Code conventions that the lints enforce:

- `unsafe` is forbidden.
- Public items need rustdoc (`missing_docs`).
- No `unwrap` or `expect` in non-test code. A bad input becomes an error value
  (and, per image, an `error` entry in the report), not a panic.

Tests live next to the code (`#[cfg(test)]`) and in `crates/*/tests/`. Generate
image fixtures in code. Use deterministic generated fixtures and record their provenance.

To try the GitHub Action's shell steps locally against `examples/`, without
GitHub, run `scripts/action-dry-run.sh`.

## Regenerate the examples

`examples/` is produced deterministically by a script (Python 3 with Pillow and
NumPy):

```sh
pip install pillow numpy
python3 scripts/gen-examples.py
```

The output is byte-stable for a given Pillow version, so a diff in `examples/`
after regenerating usually means a different Pillow. If you change the script,
commit the regenerated images with it and check that the README output for
`examples/` is still correct.

## Documentation

If a change alters a flag, a default, an output or a file format, update the
README and the relevant task documentation in the same pull request, and add a
line to [CHANGELOG.md](CHANGELOG.md). Paste real output into the README: run
the command and copy what it prints.

## Pull requests

- Keep a pull request to one change. Say what it does and why.
- Add a test for new behaviour, one focused test per behaviour is enough.
- Run the three checks above before you push.
- Dependencies must be under MIT, Apache-2.0, BSD, Zlib or ISC terms. Mention
  any new dependency in the pull request, and update
  [THIRD_PARTY.md](THIRD_PARTY.md) when it is not covered there.
- Changes to the report JSON, the decisions file or the blind-key file must stay
  readable by existing v1 files: add fields with defaults, do not rename or
  remove them.

By contributing you agree that your contribution is licensed under
`MIT OR Apache-2.0`, as the rest of the project.
