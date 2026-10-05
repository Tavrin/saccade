#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::process::{Command, Output};
#[test]
fn twenty_runs_have_bounded_preview_full_witness_and_recent_anchor_drift() {
    let temp = tempfile::tempdir().unwrap();
    let store = temp.path().join("store");
    std::fs::create_dir(&store).unwrap();
    std::fs::create_dir(store.join("objects")).unwrap();
    let mut index = String::new();
    for i in 0..20 {
        let mut source = report();
        source["generated_at_unix"] = json!(i);
        source["entries"][0]["name"] = "article.png".into();
        source["entries"][0]["value"] = json!(if i < 10 {
            0.0
        } else {
            (i - 9) as f64 * 0.000001
        });
        source["entries"][0]["baseline_sha256"] = "retained-anchor".into();
        source["entries"][0]["capture_sha256"] = json!(if i < 10 {
            "same".into()
        } else {
            format!("image{i}")
        });
        source["entries"][0]["threshold"] = json!(0.5);
        let bytes = serde_json::to_vec(&source).unwrap();
        let digest = saccade_core::localized::digest(&bytes);
        std::fs::write(store.join("objects").join(format!("{digest}.json")), bytes).unwrap();
        index.push_str(&json!({"schema":"saccade-history.v1","report_sha256":digest,"config_sha256":"config","generated_at_unix":20-i,
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

use std::path::Path;
fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn report() -> Value {
    json!({"schema":"saccade-report.v1","tool_version":"fixture","generated_at_unix":0,"config":{"default_threshold":0.01,"default_metric":"mean","pixels_per_degree":67.0,"fail_on_new":false},"totals":{"total":1,"pass":1,"fail":0,"error":0,"missing":0,"new":0},"entries":[{"name":"image.png","status":"pass","metric_used":"mean","threshold":0.01,"value":0.0,"paths":{},"baseline_sha256":"anchor","capture_sha256":"capture","metrics":{"width":16,"height":16,"mean":0.0,"max":0.0,"p50":0.0,"p95":0.0,"p99":0.0,"frac_above_0_1":0.0,"frac_above_0_5":0.0}}]})
}

#[test]
fn history_advice_uses_verified_objects_and_rejects_missing_witnesses() {
    let t = tempfile::tempdir().unwrap();
    for i in 0..10 {
        let mut r = report();
        r["generated_at_unix"] = json!(i);
        std::fs::write(t.path().join("report.json"), r.to_string()).unwrap();
        let p = cli(
            t.path(),
            &[
                "history",
                "record",
                "report.json",
                "--store",
                "store",
                "--run-id",
                &format!("run{i}"),
                "--environment-id",
                "env",
                "--unchanged-build",
            ],
        );
        assert!(p.status.success(), "{p:?}");
    }
    let path = t.path().join("store/index.jsonl");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut rows: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for row in &mut rows {
        row["samples"][0]["value"] = json!(0.9);
    }
    std::fs::write(
        &path,
        rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
    let p = cli(
        t.path(),
        &[
            "history",
            "analyze",
            "--store",
            "store",
            "--drift",
            "--out",
            "full.json",
            "--json",
        ],
    );
    assert!(p.status.success(), "{p:?}");
    let full: Value =
        serde_json::from_slice(&std::fs::read(t.path().join("full.json")).unwrap()).unwrap();
    assert_eq!(
        full["run_analysis"]["entries"][0]["evidence"][0]["value"],
        0.0
    );
    std::fs::remove_file(t.path().join(format!(
        "store/objects/{}.json",
        rows[0]["report_sha256"].as_str().unwrap()
    )))
    .unwrap();
    let p = cli(
        t.path(),
        &[
            "history", "analyze", "--store", "store", "--drift", "--json",
        ],
    );
    assert_eq!(p.status.code(), Some(2), "{p:?}");
}
