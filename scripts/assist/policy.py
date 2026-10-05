"""Frozen constructed-domain policy. No human labels and no historical evaluator changes."""
SCHEMA = "saccade-constructed-truth.v1"
ARMS = ["rules", "oracle_jev", "single_gemini", "two_gemini", "two_gemini_jev", "cascade", "cascade_jev_route"]
WORKLOADS = ["explain", "audit_mask", "check_ui", "routing"]
POLICY = {
    "version": "constructed-assist/2",
    "optional_jev_routing": "separate cascade_jev_route arm versus cascade; off by default",
    "truth": "rendered_constructed_oracle_only",
    "target_per_workload": 1000,
    "classes": {"challenge": 600, "control": 200, "unavailable": 200},
    "arms": ARMS,
    "workloads": WORKLOADS,
    "availability_min": .95,
    "coverage_min": .60,
    "assertion_precision_lower95_min": .95,
    "challenge_recall_lower95_min": .90,
    "false_reassurance_upper99_max": .01,
    "order_disagreement_max": .05,
    "necessary_evidence_skip_upper99_max": .01,
    "routing_cost_reduction_min": .20,
    "routing_matched_coverage_recall_loss_max": .02,
    "jev_unsupported_reduction_min": .30,
    "jev_coverage_loss_max": .05,
    "minimum_heldout_families": 20,
    "family_point_precision_min": .95,
    "family_point_recall_min": .90,
    "family_coverage_min": .60,
    "cluster_bootstrap_repeats": 1000,
    "cluster_bootstrap_seed": 4406,
    "matched_coverage": "fixed root SHA-256 rank; downsample committed roots to the smaller count",
    "descendants": "same root and split; never additional independent confirmations",
    "admissibility": "oracle pixel/color/text/geometry witnesses must hold before provider execution",
    "unknown_assertion": "unsupported; never silently dropped",
    "unknown_cost": "unknown; never zero and cannot pass a monetary value gate",
    "synthetic_domain_only": True,
}
GATES = ["fmt", "check", "clippy", "core-tests", "cli-tests", "wave4-heavy-core", "wave4-heavy-cli", "constructed-python", "docs", "shell", "wave4-batch-routing", "qualification-preflight"]

ORACLE_SCHEMA = "saccade-constructed-oracle.v1"
