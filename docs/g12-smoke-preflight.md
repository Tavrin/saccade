# G12 smoke preflight diagnosis

The exact epoch-3 smoke request file passes local admission, scorer self-test,
epoch identity, user root authorization and cache/request checks with both scorer
proof variables supplied. Without the variables it refuses before credentials:
`paid_run_requires_development_scorer_proof_corpus`. This error was absent from
the refusal allowlist and previously became `smoke_preflight_invalid_or_unavailable`.
The original failed process environment is unavailable, so the historical cause
cannot be proved beyond this exact-input reproduction.

`--preflight-only` runs those local checks and returns before campaign writes,
credential loading or HTTP. It does not validate provider availability, credits,
or response compatibility. `--validate-only` retains reservation-only semantics.
Known scorer, epoch, user policy, accounting, local IO and integer errors now have
sanitized diagnostics; unknown string errors still use the generic refusal.

Offline evidence: 26 example tests pass; scorer self-test passes 60 development
roots and 96 proof rows. Exact request digest:
`sha256:71e7524be0eabcdd3b324f19e70adc944ea6293ab750f62d536071b2036f2c8f`.
No provider calls or key reads were made. Cargo target peaked below 1 GB and is
removed after verification. The updated binary is copied outside the target as
`/mnt/linux-extra/moss-scratch/saccade-g12-epoch3/assist_openrouter_smoke-preflight-fixed`.

## Prepared smoke command (not executed)

Use inline proof variables so direct invocation cannot lose exported shell state.
Use a fresh campaign directory because the original `smoke-live` contains the
refusal artifact and a new run uses exclusive directory creation.

```sh
cd /home/etienne/dev/saccade-wt/g12live
SACCADE_SCORER_DEV_CORPUS=/mnt/linux-extra/moss-scratch/saccade-g12-dev-pilot/pilot-corpus \
SACCADE_SCORER_SOURCE_REVISION=ab305c00bc4536d4ff482b5a3645fe5b67f9a6d4 \
/mnt/linux-extra/moss-scratch/saccade-g12-epoch3/assist_openrouter_smoke-preflight-fixed --stage2 --budget-bounded --requests /mnt/linux-extra/moss-scratch/saccade-g12-epoch3/smoke/requests.json --roots 10 --max-spend-usd 0.252199500 --max-consecutive-invalid-answers 5 --max-invalid-answer-percent 50 --invalid-answer-min-sample 20 --user-policy /home/etienne/.config/saccade/user.toml --out /mnt/linux-extra/moss-scratch/saccade-g12-epoch3/smoke-live-preflight-fixed
```

For offline verification only, add `--preflight-only` to this command.
