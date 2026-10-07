# G12 resume after missing-generation reconciliation

A missing generation is an unknown charge, not proof of a zero-cost dispatch.
With `--settle-unknown-at-reservation`, resume may validate a stopped campaign
whose only demonstrated stop cause is an incomplete, unknown-cost receipt with
no generation ID and the exact `openrouter_generation_missing` mismatch. The
frozen campaign identity and every durable payload/root binding are checked
before mutation. Settlement retains the full reservation and reconciliation
history, records `settled_conservatively`, clears only this recoverable stop,
and excludes that root from further dispatch and scoring.

Cost or identity mismatches with a found generation, bound breaches, ceiling
refusals, and explicit operator stops remain blocking. Operator stops now retain
a durable event, so a later missing-generation mismatch cannot erase that stop.
Legacy missing-generation ledgers are classified from their existing receipts
and ceiling events; no provider lookup or cost guessing is used for recovery.

Resume/preflight diagnostics use sanitized local codes in stderr and the
`smoke.json` `refusal` object (`code`, `reason`). Existing campaign artifacts are
preserved. A fresh output directory may be created solely to record a preflight
refusal. Unknown external errors use a stable unavailable/invalid code without
printing paths, provider bodies, or credentials. No smoke artifact can be written
when the destination is absent for resume, locked, corrupt, or unwritable.

Offline regression tests cover the reconciled missing-generation recovery,
reservation retention, idempotence, reopen, unavailable-root continuation,
genuine cost/identity mismatch rejection, other stop causes, and sanitized
persisted refusal diagnostics. These do not establish live-provider acceptance.

Exact operator resume command (printed only, never executed here). Use a binary
built from this commit; the old pilot binary predates this fix:

```sh
CARGO_TARGET_DIR=/mnt/linux-extra/moss-cargo-targets/codex-saccade-g12live \
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
cargo run --offline --locked -p saccade-core --features assist \
  --example assist_openrouter_smoke -- --stage2 \
  --requests /mnt/linux-extra/moss-scratch/saccade-g12-reasoning-hint/pilot-plan/requests.json \
  --roots 216 --max-spend-usd 5 \
  --max-consecutive-invalid-answers 5 --max-invalid-answer-percent 50 --invalid-answer-min-sample 20 \
  --user-policy /home/etienne/.config/saccade/user.toml \
  --resume /mnt/linux-extra/moss-scratch/saccade-g12-reasoning-hint/pilot-live \
  --settle-unknown-at-reservation
```

Gate receipts are kept outside the repository in
`/mnt/linux-extra/moss-scratch/saccade-g12-resume-mismatch/gates.json`.

Final verification: fmt, clippy (`saccade-core`, all targets, `assist`, warnings
denied), core tests (`assist,schema,evaluation`), and all 23 smoke example tests
exited 0. The four existing offline pilot-scoring fixtures also exited 0.
The workspace feature suite passed before the final settlement-history guard;
the final core and smoke suites cover that final guard. All final gates ran with
network disabled and machine-local config hidden. No provider calls or real key
reads were performed; the pilot directory was inspected read-only.

Focused regressions:

- `g12_operator_settlement_retains_charge_skips_failed_root_and_survives_reopen`
- `g12_resume_genuine_cost_or_identity_mismatch_blocks_settlement`
- `g12_resume_refusal_is_specific_persisted_and_sanitized`
- `g12_missing_generation_settlement_preserves_other_stop_causes`
- `g12_later_missing_generation_recovers_with_prior_conservative_history`
- `test_partial_unknown_transport_is_unavailable_and_keeps_full_charge`
