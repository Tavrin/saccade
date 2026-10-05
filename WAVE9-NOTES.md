# Wave 9 decisions and evidence

- Scope: SPEC-wave9.md plus shared rules; branch feat/wave9, initial clean HEAD 51f5bb0. Single agent; no GPU/provider/network work. Heavy gates are written, not run locally.
- Extend existing FLIP, hotspots, localized inclusion, intent, metadata and perf contracts. New evidence policies are opt-in, reports additive. Rejected: changing historical default acceptance. Reversal cost: remove the opt-in policy and additive fields.
- Required-effect coverage means selected occupancy, independently on each requested side; changed pixels are also reported. It does not assert causal effect. Inclusion metrics use the union of the two selected footprints so disappearance remains visible. Rejected: trusting FLIP zero or candidate-only occupancy. Reversal cost: version the semantics if changed.
- Threshold calibration uses generated fixtures only. The supplied six-case manual review is held out. Include detail-energy ratio and absolute/relative shifted-tile share; c8's sparse bias is darker. No project images or crops enter the repository.

9.1 light evidence: cargo check saccade --features graphics passed; 3 targeted effect tests passed (including zero occupancy with measured FLIP 0, independent sides, disappearance). Logs: /mnt/linux-extra/moss-scratch/saccade-wave9/{check-9.1,test-9.1}.log. No new dependencies.
