# Wave 2 review fixes

Binding brief: `SPEC-wave2-fixes.md`; reviewed starting revision
`f2004f465e29113a52b622b57d7a1025bc60d5b1`. All sixteen findings are accepted.
No subagents, push, merge or rebase were used. Commits use Tavrin's requested
identity. The work is grouped by capture accounting, evidence readers,
RenderDoc/file boundaries, history and compression qualification.

Regression receipts are retained in `/tmp/saccade-wave2-fixes/`. Each regression
was added and observed failing against the relevant original implementation
before the fix. The initially incomplete masked-report fixture was corrected and
rerun against the original grounded implementation (`red-mask-scope.log`); the
payload FIFO regression uses absolute extraction paths so it reproduces the
blocking FIFO rather than the separate bare-parent defect.

| Finding | Regression | Failing receipt |
| --- | --- | --- |
| 1 | snapshot_names_and_duplicate_sources_cannot_create_complete_pairs | red-playwright.log |
| 2 | test_capture_fifo_rejected_before_open; test_native_worker_deadline_terminates_stall; renderdoc_payload_fifo_is_rejected_without_blocking | red-worker.log; red-payload.log |
| 3 | test_clear_never_reads_unrelated_graphics_bindings | red-worker.log |
| 4 | history_advice_uses_verified_objects_and_rejects_missing_witnesses | red-history.log |
| 5 | repeated_and_settled_anchor_drift_remains_detected | red-drift.log |
| 6 | grounded_masked_region_exposes_exclusion_scope | red-mask-scope.log |
| 7 | compression_metrics_match_official_nonidentical_references | red-reference.log |
| 8 | localized_grounded_quantity_matches_source_json_exactly | red-contracts.log |
| 9 | report_replacement_cannot_split_facts_and_digest | red-retained-read.log |
| 10 | bare_filenames_work_for_quality_and_renderdoc | red-bare.log |
| 11 | frozen_region_requires_method_specific_provenance | red-contracts.log |
| 12 | contradictory_snapshot_case_id_is_rejected_before_comparison | red-playwright.log |
| 13 | new_readers_diagnose_newer_producers_and_malformed_separately; nested_legacy_metrics_unknown_fields_require_upgrade | red-contracts.log; red-nested-fields.log |
| 14 | missing_attachment_preserves_suite_accounting_and_usable_case | red-playwright.log |
| 15 | missing_resource_format_or_identity_cannot_be_complete | red-format.log |
| 16 | doctor_advertises_wave2_contracts | red-contracts.log |

Finding 9 uses a controlled POSIX channel: its first read returns valid report
A, while a second open returns valid replacement B. Against the original reader
implementations, the regression observed grounded facts from B with A's digest
and inventory facts from A with B's digest. Both assertions pass with the
retained-byte readers. This replaces the early source-only assertion; it is a
controlled replacement witness, not a concurrent regular-file stress campaign.
RenderDoc clear extraction abstains explicitly, with
empty resource evidence and an extraction limit: graphics targets cannot stand
in for a clear destination. The process supervisor applies a terminating
external deadline to hashing, import, capture opening, replay and readback.
Rust uses nonblocking Unix opens, pre/post-open regular-file checks and hard
per-file/aggregate budgets. No live Vulkan replay is claimed.

Frozen provenance is validated by method. External source-mask/authoring claims
remain producer declarations rather than authenticated history. An explicitly
`unverified_import` method requires a nonempty reason and stays visibly
unverified in measurement limits. Ordinary empty provenance is rejected.

Compression uses committed project-authored images and independently executed
[official reference tools](../../crates/saccade-core/tests/fixtures/compression-reference/README.md).
The missing supplementary Butteraugli evidence produced the reference test's
original failure; the new test additionally rejects constant identity scores on
nonidentical images. The numerical tolerances qualify this small fixture set,
not arbitrary images, display conditions, or human judgments. New dependencies
retain the declared Rust 1.88 floor; a Rust 1.88 execution is not claimed.

The shared reader detects ignored fields throughout typed documents, including
legacy nested metrics/exclusion types that do not declare deny_unknown_fields.
The nested-metrics negative test reproduced silent acceptance before this final
reader correction. Newer nested schema versions also produce upgrade diagnostics;
malformed values remain ordinary parsing errors.
