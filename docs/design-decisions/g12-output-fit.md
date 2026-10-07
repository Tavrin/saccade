# G12 epoch 4b: output fit and truncation

Development only. No provider calls or keys read. Prompt epoch remains g12-pilot/4:
all oracle-perfect answers fit the existing 4096-token budget, so epoch 5 and
reservation increases are unnecessary. No observation cap or scorer relaxation.

The mandatory offline scorer proof serializes every development oracle-perfect
closed answer, including orders and counterfactuals. One UTF-8 byte counts as one
estimated token, conservatively exceeding ordinary chars/4 estimates. Require
2 * answer bytes + requested reasoning hint <= max_tokens. This is an estimate
with margin, not a guarantee of model behavior or observed reasoning usage.
Any individual failed fit fails the gate. Per-workload worst sizes:

| Workload | Answer bytes / estimated tokens | Reasoning hint | Required with 2x margin | Budget |
|---|---:|---:|---:|---:|
| audit_mask | 805 | 1024 | 2634 | 4096 |
| check_ui | 325 | 512 | 1162 | 4096 |
| explain | 360 | 1024 | 1744 | 4096 |
| routing | 325 | 512 | 1162 | 4096 |

Prepared request rows carry reviewed offline output_fit declarations outside the
provider payload. The runner binds the declaration to request_hash and the actual
output/reasoning budgets and checks the rule, arithmetic and margin. Invalid
provided declarations fail offline validation. Missing declarations retain the
legacy campaign-stop behavior. These are trusted reviewed plan facts, not a
runtime oracle or independently authenticated certificates; review requests and
selftest artifacts together. Provider payload and prompt identity are unchanged.

After model identity, single-choice/refusal and monetary usage checks, a length
finish with valid fit becomes invalid_answer / truncated_output. It is counted
by the existing consecutive/rate safety valve and the campaign can continue.
Without fit it remains a campaign failure. Bad billing, refusal, misrouting and
storage failures remain campaign errors. Truncated content never produces a
validated answer file or becomes eligible for qualification.

## Root 0 diagnosis

Epoch-4 development root 83b28204... returned the correct observed outcome and two
true appearance:changed assertions. Its second normalized x=0.193416 converts to
94.000176 pixels, inside the exclusion boundary x=94. Task evidence requires full
coverage; that exclusion was not enclosed. The first box fully covers R0 and the
first exclusion. This is model geometry rounding error, not remaining protocol
ambiguity: epoch 4 already explicitly requires enclosing boundaries and says
there is no rounding tolerance. The oracle uses outward enclosure. Retain the
prompt and scorer; do not round provider geometry or lower coverage requirements.
See root0-diagnosis.json in the artifact directory for exact independent replay.

## Plans and commands

Artifacts: the operator-owned epoch-4b evidence directory

Smoke: 10 development roots, 10 single_gemini calls; full reservation $0.252169500.
Targeted: 10 development roots across four admitted arms, 20 calls; full
reservation $0.510225000, allowance $0.60. Old-epoch transferred cost proxies are
$0.046281000 and $0.092562000, respectively, unverified estimates. Actual new
spend is zero. Both plans remain unauthorized and unqualified.

Exact operator commands, including SACCADE_SCORER_DEV_CORPUS and
SACCADE_SCORER_SOURCE_REVISION, are in commands.sh. Only offline proof, plan
generation, local tests/build and --validate-only are executed here. Live smoke,
targeted dispatch, and reconciliation commands are prepared only.

Verification: frozen development proof PASS (60 roots, 96 scorer rows); stable
scorer unittest module 6/6; Rust runner tests 27/27; both plans validate offline.
Exploratory broad Python runs overlapped source edits, detected source drift,
and were terminated. No full Python suite acceptance is claimed.
