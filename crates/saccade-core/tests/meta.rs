//! Measurement chronology survives sidecar normalization.
#![allow(clippy::expect_used, missing_docs)]
#[test]
fn capture_evidence_preserves_measurement_timestamp() {
    let dir = tempfile::tempdir().expect("tmp");
    for timestamp in [serde_json::json!(123), serde_json::json!("123")] {
        std::fs::write(
            dir.path().join("saccade-meta.json"),
            serde_json::json!({"capture.timestamp": timestamp}).to_string(),
        )
        .expect("metadata");
        let evidence =
            saccade_core::meta::capture_evidence(dir.path(), "frame.png", "saccade-meta.json");
        assert_eq!(evidence.get("timestamp").map(String::as_str), Some("123"));
    }
}
