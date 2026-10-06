#!/usr/bin/env bash
# Explicit feature qualification gates; see docs/releasing.md.
set -u
cd "$(dirname "$0")/.." || exit 2
source scripts/gate-env.sh
export SYSTEM_DEPS_DAV1D_BUILD_INTERNAL=never
failed=0
run_gate() {
  local name="$1"; shift
  if saccade_run "$@"; then printf 'GATE %s PASS\n' "$name"; else printf 'GATE %s FAIL\n' "$name"; failed=1; fi
}
space() { saccade_headroom; }
cargo_gate() { space && saccade_run nice -n 19 cargo "$@"; }
avif_prerequisite() {
  command -v pkg-config >/dev/null && pkg-config --atleast-version=1.3.0 dav1d
}
avif_cargo() { avif_prerequisite && cargo_gate "$@"; }
js_browser() {
  [[ -x "$CARGO_TARGET_DIR/debug/saccade" ]] || return 1
  export SACCADE_BIN="$CARGO_TARGET_DIR/debug/saccade"
  (cd integrations/playwright && saccade_run npx --no-install playwright test --config test/playwright.config.cjs)
}
run_gate fmt cargo fmt --all -- --check
run_gate genericity scripts/check-genericity.sh
run_gate genericity-tests python3 scripts/test-genericity.py
run_gate docs python3 scripts/check-wave5-docs.py
run_gate js-unit node --test integrations/playwright/matcher.test.cjs integrations/playwright/reporter.test.cjs integrations/playwright/sweep.test.cjs integrations/playwright/design-capture.test.cjs
run_gate core-tests cargo_gate test -j 4 -p saccade-core --features compression,schema,graphics,ai,workbench,evaluation
run_gate cli-tests cargo_gate test -j 4 -p saccade --features products,schema
run_gate clippy cargo_gate clippy -j 4 -p saccade -p saccade-core --all-targets --features saccade/products,saccade/schema,saccade-core/schema -- -D warnings
run_gate avif-prerequisite avif_prerequisite
run_gate avif-check avif_cargo check -j 4 -p saccade --features imgtune-avif,schema
run_gate avif-clippy avif_cargo clippy -j 4 -p saccade --all-targets --features imgtune-avif,schema -- -D warnings
# Only this wave's ignored groups: no unrelated live-provider/model tests.
run_gate heavy-imgtune avif_cargo test -j 4 -p saccade --bin saccade --features imgtune-avif imgtune_cmd::tests -- --ignored
run_gate heavy-design cargo_gate test -j 4 -p saccade --bin saccade --features products design_cmd::tests -- --ignored
run_gate heavy-history cargo_gate test -j 4 -p saccade --bin saccade --features products last_good::tests -- --ignored
run_gate heavy-notifier cargo_gate test -j 4 -p saccade --bin saccade --features products notifier_cmd::tests -- --ignored
run_gate heavy-sweep cargo_gate test -j 4 -p saccade --bin saccade --features products sweep_cmd::tests -- --ignored
run_gate cli-binary cargo_gate build -j 4 -p saccade --features products,schema
run_gate browser js_browser
run_gate fixture-cli python3 scripts/test-wave5-cli.py "$CARGO_TARGET_DIR/debug/saccade"
# No showcase was touched. No provider/model download or live Figma/webhook call.
exit "$failed"
