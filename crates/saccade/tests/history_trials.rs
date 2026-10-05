#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::process::Command;
#[test]
fn twenty_runs_have_bounded_preview_full_witness_and_recent_anchor_drift() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("store");
    std::fs::create_dir(&store).unwrap();
    let mut index = String::new();
    for i in 0..20 {
        index.push_str(&json!({"schema":"saccade-history.v1","report_sha256":format!("report{i}"),"config_sha256":"config","generated_at_unix":20-i,
            "trial":{"run_id":format!("capture-{i:02}"),"environment_id":"chromium-fonts-v1-1280x720-warm","unchanged_build":i<10},
            "samples":[{"entry":"article.png","baseline_sha256":"retained-anchor","capture_sha256":if i<10 {"same".into()}else{format!("image{i}")},"metric":"mean","threshold":0.5,"value":if i<10 {0.0}else{(i-9) as f64*0.000001}}]}).to_string());
        index.push('\n');
    }
    std::fs::write(store.join("index.jsonl"), index).unwrap();
    let full = temp.path().join("full.json");
    let output = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(["history", "analyze", "--store"])
        .arg(&store)
        .args(["--entry", "article.png", "--drift", "--json", "--out"])
        .arg(&full)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.len() <= 4096);
    let preview: Value = serde_json::from_slice(&output.stdout).unwrap();
    let complete: Value = serde_json::from_slice(&std::fs::read(full).unwrap()).unwrap();
    assert_eq!(
        preview["run_analysis"]["entries"][0]["independent_runs"],
        20
    );
    assert_eq!(preview["run_analysis"]["entries"][0]["drift"], "candidate");
    assert_eq!(
        preview["run_analysis"]["entries"][0]["evidence_omitted"],
        14
    );
    assert_eq!(
        complete["run_analysis"]["entries"][0]["evidence"]
            .as_array()
            .unwrap()
            .len(),
        20
    );
    assert!(complete["run_analysis"]["entries"][0]["suggested_threshold"].is_null());
    assert!(complete["entries"].as_array().unwrap().is_empty());
    assert_eq!(complete["policy_changed"], false);
}
