# Development campaign diagnosis and scores

The resumed campaign stopped at zero-based row 401 (cause, compact_jev, reverse, f00-tone): `invalid_answer`. The response probabilities sum to 0.99, outside the frozen 1e-6 tolerance. Choice global_tone matches truth but the response is invalid and unavailable. Do not normalize or relax this validation.

Receipt 9b99672e82ef005f83e5c4d7bdd9e60f is completed, charged 23,898 nano-USD from returned 569 input tokens. No transport failure or identity error was recorded for this attempt. Total conservatively accounted spend is 8,313,732 nano-USD ($0.008313732), below the $0.50 allowance. The only transport-unavailable attempt is row 100, settled at its full 203,322 nano-USD reservation; its historical `other` class does not establish HTTP status or rate limiting. No safety valve is recorded; 1/402 failures cannot trip the declared >10% or >5 consecutive limits.

400 valid answers, one invalid answer, one transport-unavailable attempt and 598 unscheduled tail variants. Evidence files were read only.

Two-arm reports previously lived under tasks.<task>.arms, so single-arm smoke consumers saw empty/default fields. The scorer now exposes task_arm_table and physical-answer metrics. Root accuracy and availability require all scheduled order variants; missing variants remain unavailable in denominators. Option disagreement counts conflicting valid answers, not a missing answer.

| Task | Arm | Valid / scheduled | Valid answer accuracy | Root accuracy | Root availability | Precision LB95 | Challenge recall LB95 | Control FP | Necessary skips / UB99 | Option disagreement | Rules accuracy | Candidate − rules |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| routing | compact_jev | 99 / 100 | 100.00% | 98.00% | 98.00% | 92.61% | 91.80% | 0 | 1 / 15.47% | 0.00% | 100.00% | -2.00% |
| routing | enriched_jev | 100 / 100 | 100.00% | 100.00% | 100.00% | 92.78% | 91.80% | 0 | 0 / 10.87% | 0.00% | 100.00% | 0.00% |
| classification | compact_jev | 100 / 100 | 67.00% | 60.00% | 100.00% | 71.16% | 56.37% | 4 | n/a | 14.00% | 100.00% | -40.00% |
| classification | enriched_jev | 100 / 100 | 70.00% | 70.00% | 100.00% | 90.50% | 56.37% | 0 | n/a | 6.00% | 100.00% | -30.00% |
| cause | compact_jev | 1 / 100 | 100.00% | 0.00% | 0.00% | 0.00% | 0.00% | 0 | n/a | 0.00% | 40.00% | -40.00% |
| cause | enriched_jev | 0 / 100 | unavailable | 0.00% | 0.00% | 0.00% | 0.00% | 0 | n/a | 0.00% | 40.00% | -40.00% |
| priority | compact_jev | 0 / 100 | unavailable | 0.00% | 0.00% | 0.00% | 0.00% | 0 | n/a | 0.00% | 100.00% | -100.00% |
| priority | enriched_jev | 0 / 100 | unavailable | 0.00% | 0.00% | 0.00% | 0.00% | 0 | n/a | 0.00% | 100.00% | -100.00% |
| support | compact_jev | 0 / 100 | unavailable | 0.00% | 0.00% | 0.00% | 0.00% | 0 | n/a | 0.00% | 20.00% | -20.00% |
| support | enriched_jev | 0 / 100 | unavailable | 0.00% | 0.00% | 0.00% | 0.00% | 0 | n/a | 0.00% | 20.00% | -20.00% |

Each task/arm has 50 scheduled roots (100 physical variants). Rules comparison uses the same scheduled roots, including the unavailable candidate tail. Lower bounds use committed eligible roots and challenge denominators; these are development, constructed-domain results, not held-out qualification. `qualified=false`; no paired vision or verified provider debit evidence.

Resume decision: no resume command is issued. The stop is money-accounted but is an invalid provider answer, not a benign pacing failure. Existing resume deliberately replays and stops on that invalid row; continuation-at-reservation does not authorize bypassing answer validation. Scorer changes also remain outside the frozen resume source allowlist. A resume command under the current contract would refuse before dispatch or stop at the same row.

Machine-readable score: `development-score-diagnosed.json`. No provider calls and no keys read.

Validation: 9 Python tests passed; 3 Rust runner tests passed, 1 operator fixture test ignored; cargo fmt and git diff --check passed. Cargo ran offline with the specified target and settings (644 MB), and the target was deleted afterward. No live acceptance is claimed.
