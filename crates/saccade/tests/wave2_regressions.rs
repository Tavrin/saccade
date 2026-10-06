#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};
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
fn localized() -> Value {
    json!({"schema":"saccade-localized.v1","region":{"schema":"saccade-frozen-region.v1","region_id":"intended","semantics":"include","reference_sha256":"a".repeat(64),"dimensions":[2,2],"inclusion":[1,0,0,0],"mask_sha256":saccade_core::localized::digest(&[1,0,0,0]),"provenance":{"method":"box"}},"candidate_sha256":"b".repeat(64),"pixels_per_degree":67.0,"inside":{"pixels":1,"changed_pixels":1,"mean_flip":0.1,"max_flip":0.1},"outside":{"pixels":3,"changed_pixels":0,"mean_flip":0.0,"max_flip":0.0},"boundary":{"pixels":3,"changed_pixels":1,"mean_flip":0.1,"max_flip":0.1},"intended_change_detected":true,"exact_outside":true,"maximum_outside_flip":0.01,"collateral":"preserved","limits":[]})
}
#[test]
fn grounded_masked_region_exposes_exclusion_scope() {
    let mut r = report();
    r["entries"][0]["pixel_exclusions"] = json!({"dimensions":[16,16],"runs":[[0,16]],"mask_sha256":"a".repeat(64),"pixels":16,"error_sum":16.0,"error_mean":1.0,"error_max":1.0,"full_frame_value":0.0625,"without_masks":"fail"});
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("report.json"), r.to_string()).unwrap();
    let p = cli(
        t.path(),
        &[
            "explain-grounded",
            "--report",
            "report.json",
            "--out",
            "out.json",
        ],
    );
    assert!(p.status.success(), "{p:?}");
    let e: Value =
        serde_json::from_slice(&std::fs::read(t.path().join("out.json")).unwrap()).unwrap();
    assert_eq!(
        e["catalog"]["regions"][0]["measurement_scope"],
        "included_pixels"
    );
    assert_eq!(
        e["catalog"]["regions"][0]["pixel_exclusions"],
        r["entries"][0]["pixel_exclusions"]
    );
}
#[test]
fn localized_grounded_quantity_matches_source_json_exactly() {
    let t = tempfile::tempdir().unwrap();
    let r = localized();
    std::fs::write(t.path().join("report.json"), r.to_string()).unwrap();
    let p = cli(
        t.path(),
        &[
            "explain-grounded",
            "--report",
            "report.json",
            "--out",
            "out.json",
        ],
    );
    assert!(p.status.success(), "{p:?}");
    let e: Value =
        serde_json::from_slice(&std::fs::read(t.path().join("out.json")).unwrap()).unwrap();
    for f in e["catalog"]["facts"].as_array().unwrap() {
        assert_eq!(
            f["value"].as_f64().unwrap(),
            r.pointer(f["source_pointer"].as_str().unwrap())
                .unwrap()
                .as_f64()
                .unwrap()
        );
    }
}
#[cfg(unix)]
#[test]
fn report_replacement_cannot_split_facts_and_digest() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("report.json");
    std::fs::write(&path, serde_json::to_vec(&report()).unwrap()).unwrap();
    let script = include_str!("support/retained_read.py");
    let p = Command::new("python3")
        .args(["-c", script])
        .arg(env!("CARGO_BIN_EXE_saccade"))
        .arg(path)
        .output()
        .unwrap();
    assert!(p.status.success(), "{p:?}");
}

#[test]
fn frozen_region_requires_method_specific_provenance() {
    let t = tempfile::tempdir().unwrap();
    image::RgbImage::from_pixel(2, 2, image::Rgb([50, 60, 70]))
        .save(t.path().join("image.png"))
        .unwrap();
    let mut r = localized()["region"].clone();
    r["reference_sha256"] =
        saccade_core::localized::digest(&std::fs::read(t.path().join("image.png")).unwrap()).into();
    for (i, provenance) in [
        json!({}),
        json!({"method":"mask_import","source_mask_sha256":"wrong"}),
        json!({"method":"dom_selector","selector":""}),
    ]
    .into_iter()
    .enumerate()
    {
        r["provenance"] = provenance;
        std::fs::write(t.path().join("region.json"), r.to_string()).unwrap();
        let p = cli(
            t.path(),
            &[
                "localized-check",
                "image.png",
                "image.png",
                "--region",
                "region.json",
                "--out",
                &format!("out{i}"),
            ],
        );
        assert_eq!(p.status.code(), Some(2), "{p:?}");
    }
}
#[test]
fn new_readers_diagnose_newer_producers_and_malformed_separately() {
    let t = tempfile::tempdir().unwrap();
    for (i, mut r) in [localized(), localized(), localized(), localized()]
        .into_iter()
        .enumerate()
    {
        if i == 0 {
            r["inside"]["newer_field"] = json!(true);
        } else if i == 1 {
            r["schema"] = "saccade-localized.v99".into();
        } else if i == 2 {
            r["region"]["schema"] = "saccade-frozen-region.v2".into();
        } else {
            r["inside"]["pixels"] = "unknown field".into();
        }
        std::fs::write(t.path().join("report.json"), r.to_string()).unwrap();
        let p = cli(
            t.path(),
            &[
                "explain-grounded",
                "--report",
                "report.json",
                "--out",
                &format!("out{i}.json"),
                "--json",
            ],
        );
        assert_eq!(p.status.code(), Some(2), "{p:?}");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&p.stdout),
            String::from_utf8_lossy(&p.stderr)
        );
        if i < 3 {
            assert!(
                text.contains("newer producer")
                    || text.contains("written by saccade-localized.v99")
                    || text.contains("written by saccade-frozen-region.v2"),
                "{text}"
            );
            assert!(text.contains("upgrade"), "{text}");
        } else {
            assert!(!text.contains("upgrade"), "{text}");
        }
    }
    std::fs::write(
        t.path().join("sweep.json"),
        json!({"schema":"saccade-quality-sweep.v2"}).to_string(),
    )
    .unwrap();
    let p = cli(
        t.path(),
        &[
            "quality-sweep",
            "sweep.json",
            "--out",
            "quality.json",
            "--json",
        ],
    );
    assert!(
        String::from_utf8_lossy(&p.stderr).contains("upgrade")
            || String::from_utf8_lossy(&p.stdout).contains("upgrade"),
        "{p:?}"
    );
}
#[test]
fn doctor_advertises_wave2_contracts() {
    let t = tempfile::tempdir().unwrap();
    let p = cli(t.path(), &["doctor", "--json"]);
    assert!(p.status.success());
    let d: Value = serde_json::from_slice(&p.stdout).unwrap();
    let schemas = d["schemas"].to_string();
    for schema in [
        "saccade-inventory.v1",
        "saccade-inventory-report.v1",
        "saccade-localized.v1",
        "saccade-grounded.v1",
        "saccade-frozen-region.v1",
        "saccade-dom-regions.v1",
        "saccade-region-models.v1",
        "saccade-quality-sweep.v1",
        "saccade-quality-report.v1",
        "saccade-renderdoc-extract.v1",
        "saccade-renderdoc-localization.v1",
    ] {
        assert!(schemas.contains(schema), "missing {schema}: {d}");
    }
}

#[test]
fn legacy_report_schema_remains_readable() {
    let t = tempfile::tempdir().unwrap();
    let mut r = report();
    r["schema"] = "flipdiff-report.v1".into();
    let bytes = serde_json::to_vec(&r).unwrap();
    std::fs::write(t.path().join("historical.json"), &bytes).unwrap();
    let p = cli(
        t.path(),
        &["inspect", "exclusions", "historical.json", "--json"],
    );
    assert!(p.status.success(), "{p:?}");
    assert_eq!(
        std::fs::read(t.path().join("historical.json")).unwrap(),
        bytes
    );
}

#[test]
fn nested_legacy_metrics_unknown_fields_require_upgrade() {
    let t = tempfile::tempdir().unwrap();
    let mut r = report();
    r["entries"][0]["metrics"]["newer_producer_field"] = json!(true);
    std::fs::write(t.path().join("report.json"), r.to_string()).unwrap();
    let p = cli(
        t.path(),
        &[
            "explain-grounded",
            "--report",
            "report.json",
            "--out",
            "out.json",
            "--json",
        ],
    );
    assert_eq!(p.status.code(), Some(2), "{p:?}");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&p.stdout),
        String::from_utf8_lossy(&p.stderr)
    );
    assert!(
        text.contains("version_skew") && text.contains("upgrade"),
        "{text}"
    );
}
