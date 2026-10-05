# Wave 2 decisions

## 1. Snapshot variation and drift

Extend the local history index additively. A producer-assigned run ID and an explicit frozen environment identity distinguish independent trials from artifact copies. Identical pixels across declared runs count as independent observations. Legacy rows remain readable and retain their provisional artifact-variation advice. They cannot establish independent normal variation.

Only explicitly declared unchanged-build repeats contribute to normal variation. Ten are required for advice. The supplied environment identity must cover browser/device, fonts, viewport, readiness, warmup, temporal phase and cache protocol. The same baseline and configuration remain the anchor across revisions. Recorded sequence orders observations; report timestamps do not establish capture chronology. A conservative median-window and monotonicity heuristic flags cumulative drift beyond the observed repeat range. It reports a candidate and fresh-repeat advice, never commit attribution. Drift suppresses tolerance advice; no command edits policy.

Residual: scalar history cannot localize timestamp/font defects. Real browser nuisance-alert and capture-fix evaluation remains pilot work. Producer declarations do not independently prove capture independence.
