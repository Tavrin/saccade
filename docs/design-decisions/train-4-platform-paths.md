# Train 4: native paths and portable capture names

Scope: `integration/train-4`, starting at
`bc3f2fb2358a250fcec4b0f087519acd8e937ec3`.

## Evidence and causes

[CI run 37646549771](https://github.com/Tavrin/saccade/actions/runs/37646549771)
shows the Windows sensitivity failure and the macOS capture failures:

- `sensitivity`: native nested relative paths contain Windows separators. The
  preflight check rejected those separators before the later serialization
  replaced them with `/`.
- `capture_conformance`: `bounded_read` walked from the filesystem root with
  no-follow handles. macOS temporary directories reached through `/var` were
  rejected at the OS alias, so a valid image was unavailable before hashing.
- `capture_legacy`: the same traversal rejected temporary map/source roots at
  `/var`, producing `invalid_record` and exit 2 before field adaptation, image
  checks, or archive reporting could produce the expected codes and exits.
  The retrieved failed logs do not show these capture failures on Windows;
  native Windows behavior still requires the pushed revision's CI.

## Decisions and boundaries

1. Derive native baseline names by validating individual UTF-8 filename
   components, then joining with `/`. Windows separators are structural;
   literal Unix backslashes and control characters remain errors. Preflight
   validation and trial serialization use the same helper.
2. Validate untrusted portable capture names independently of the host parser.
   Reject backslashes, controls, colons (drive prefixes/alternate streams),
   absolute/UNC/verbatim names, empty components, and dot/parent traversal.
   Apply this to native image names and legacy source/record names.
3. Retain the complete Windows filesystem root, including drive, UNC and
   verbatim prefixes. Restore recorded verbatim spelling before filesystem
   operations. Preserve parent components until preceding directories have
   been inspected with no-follow handles; even verbatim-path concatenation
   must not silently fold `link/..`.
4. On macOS recognize only the OS aliases `/var`, `/tmp`, and `/etc`, and only
   when their canonical targets are exactly `/private/var`, `/private/tmp`,
   and `/private/etc`. Walk the physical components with no-follow handles.
   Keep their parent handles so `..` retains filesystem meaning.
5. Reject arbitrary root, ancestor, descendant, map and source symlinks as
   before. Final-file reads retain capability-relative no-follow opens,
   regular-file checks and byte bounds. Existing native receipt directory
   alias canonicalization remains explicit; legacy roots receive no general
   canonicalization exemption.

Rejected alternatives: canonicalizing an entire legacy path would conceal
user-created symlinks; blindly replacing backslashes would reinterpret Unix
filenames and portable user input; lexical parent folding would erase symlink
routes before inspection. No existing tests were skipped or relaxed.

## Validation

All Cargo commands use the spec's target directory, `CARGO_INCREMENTAL=0`,
and `CARGO_PROFILE_DEV_DEBUG=0`. Local target usage stayed below 5 GB.

- `cargo fmt --all --check`: exit 0.
- `cargo clippy --locked -p saccade-core -p saccade --all-targets -- -D warnings`:
  exit 0.
- `cargo test --locked -p saccade-core --lib paths::tests`: exit 0, 8 passed.
- `cargo test --locked -p saccade-core --test capture_conformance -p saccade
  --test sensitivity --test capture_legacy`: exit 0 on the final implementation;
  capture conformance 7 passed, sensitivity 3 passed, legacy capture 8 passed.
  Cargo also ran 3 existing CLI capture conformance integration tests successfully.
- Default-feature checks of both touched crates for `x86_64-pc-windows-gnu`
  and `x86_64-apple-darwin`: exit 101 in ring's native build. Windows lacks
  `x86_64-w64-mingw32-gcc`; the macOS build selects host GCC, which rejects
  Apple-specific flags. These are unavailable cross-C toolchains.
- `cargo check --locked --no-default-features -p saccade-core -p saccade
  --all-targets --target TARGET`: exit 0 for each installed cross target.
  This checks the changed Rust branches and platform regression tests;
  pre-existing reduced-feature CLI warnings remain. It is not native runtime
  or default-feature platform proof.

Native macOS and Windows execution belongs to CI for the pushed revision.
