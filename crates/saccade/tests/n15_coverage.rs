//! Generated cross-domain coverage proofs at the executable boundary (CC0).
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use image::{Rgb, RgbImage};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::Path, process::Command};
fn run(root: &Path, args: &[&str], code: i32) -> Value {
    let result = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(code), "{args:?}: {result:?}");
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    if args[0] == "manifest" {
        let schema: Value = serde_json::from_str(
            saccade_core::schema_catalog::get(value["schema"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&value)
            .unwrap();
    }
    value
}
fn file(root: &Path, name: &str) -> Value {
    json!({"path":name,"sha256":format!("{:x}",Sha256::digest(std::fs::read(root.join(name)).unwrap()))})
}
#[test]
fn web_viewports_asset_sizes_languages_and_scene_tiers_keep_absences_and_separate_drift() {
    for (family, axes, variants) in [
        (
            "pages",
            json!({"page":["start","details"],"viewport":["narrow","wide"]}),
            vec![
                json!({"page":"start","viewport":"narrow"}),
                json!({"page":"start","viewport":"wide"}),
                json!({"page":"details","viewport":"wide"}),
            ],
        ),
        (
            "assets",
            json!({"size":["small","large"],"language":["en","fr"]}),
            vec![
                json!({"size":"small","language":"en"}),
                json!({"size":"large","language":"en"}),
                json!({"size":"large","language":"fr"}),
            ],
        ),
        (
            "scenes",
            json!({"scene":["interior","outdoor"],"tier":["low","high"]}),
            vec![
                json!({"scene":"interior","tier":"low"}),
                json!({"scene":"interior","tier":"high"}),
                json!({"scene":"outdoor","tier":"high"}),
            ],
        ),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        RgbImage::from_pixel(32, 32, Rgb([255; 3]))
            .save(root.join("current.png"))
            .unwrap();
        RgbImage::from_pixel(32, 32, Rgb([0; 3]))
            .save(root.join("anchor.png"))
            .unwrap();
        let latest = run(
            root,
            &[
                "compare",
                "current.png",
                "current.png",
                "--out",
                "latest",
                "--json",
            ],
            0,
        );
        let drift = run(
            root,
            &[
                "compare",
                "anchor.png",
                "current.png",
                "--out",
                "drift",
                "--json",
            ],
            1,
        );
        assert!(latest.is_object() && drift.is_object());
        let load_report = |name: &str| -> Value {
            serde_json::from_slice(
                &std::fs::read(root.join(name).join("saccade-report.v1.json")).unwrap(),
            )
            .unwrap()
        };
        let latest = load_report("latest");
        let drift = load_report("drift");
        let reference = |report: &Value| json!({"report_id":report["report_id"],"entry":report["entries"][0]["name"]});
        let case = json!({"case_id":format!("{family}-measured"),"variants":variants[0],"required":true,"baseline":file(root,"current.png"),"capture":file(root,"current.png"),"approved_anchor":file(root,"anchor.png"),"approved_at_unix":100,"last_good":file(root,"current.png"),"latest":reference(&latest),"anchor_comparison":reference(&drift),"last_good_comparison":reference(&latest),"refusal":null});
        let mut absent = case.clone();
        absent["case_id"] = format!("{family}-absent-both").into();
        absent["variants"] = variants[1].clone();
        for key in [
            "baseline",
            "capture",
            "latest",
            "approved_anchor",
            "approved_at_unix",
            "last_good",
            "anchor_comparison",
            "last_good_comparison",
        ] {
            absent[key] = Value::Null;
        }
        let mut refused = absent.clone();
        refused["case_id"] = format!("{family}-refused").into();
        refused["variants"] = variants[2].clone();
        refused["refusal"] = "producer refused acquisition".into();
        std::fs::write(
            root.join("cases.json"),
            serde_json::to_vec(
                &json!({"schema":"saccade-cases.v1","axes":axes,"cases":[case,absent,refused]}),
            )
            .unwrap(),
        )
        .unwrap();
        run(
            root,
            &["manifest", "build", ".", "--cases", "cases.json", "--json"],
            0,
        );
        run(
            root,
            &[
                "manifest",
                "views",
                ".",
                "--out",
                "views",
                "--now-unix",
                "2000000000",
                "--max-age-seconds",
                "2000000000",
                "--json",
            ],
            1,
        );
        let report: Value =
            serde_json::from_slice(&std::fs::read(root.join("views/coverage.json")).unwrap())
                .unwrap();
        assert_eq!(report["counts"]["expected"], 3);
        assert_eq!(report["counts"]["measured"], 1);
        assert_eq!(report["counts"]["refused"], 1);
        assert_eq!(report["groups"].as_array().unwrap().len(), 3);
        let measured = report["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["case"]["case_id"] == format!("{family}-measured"))
            .unwrap();
        assert_eq!(measured["approved_anchor"]["verdict"], "fail");
        assert_eq!(measured["last_good"]["verdict"], "pass");
        let html = std::fs::read_to_string(root.join("views/index.html")).unwrap();
        assert!(
            html.contains(&format!("{family}-absent-both"))
                && html.contains("never_approved")
                && html.contains("Baseline health")
        );
        for (id, value) in [
            (
                "saccade-manifest.v2",
                serde_json::from_slice::<Value>(
                    &std::fs::read(root.join("saccade-manifest.json")).unwrap(),
                )
                .unwrap(),
            ),
            ("saccade-coverage.v1", report),
        ] {
            let schema: Value =
                serde_json::from_str(saccade_core::schema_catalog::get(id).unwrap()).unwrap();
            jsonschema::validator_for(&schema)
                .unwrap()
                .validate(&value)
                .unwrap();
        }
    }
}
#[test]
fn missing_manifest_is_a_typed_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let missing = run(
        root,
        &[
            "manifest",
            "views",
            "missing.json",
            "--out",
            "views",
            "--json",
        ],
        2,
    );
    assert!(missing["errors"][0]["code"].is_string());
}

#[test]
fn foreign_output_is_preserved_and_labels_are_escaped() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let declaration: saccade_core::coverage::Declaration = serde_json::from_value(json!({
        "schema":"saccade-cases.v1", "axes":{}, "cases":[{
            "case_id":"<script>alert(1)</script>","variants":{},"required":true
        }]
    }))
    .unwrap();
    saccade_core::coverage::write_manifest(
        root,
        saccade_core::manifest::Anchors::default(),
        &declaration,
    )
    .unwrap();
    std::fs::create_dir(root.join("foreign")).unwrap();
    std::fs::write(root.join("foreign/index.html"), b"owned elsewhere").unwrap();
    run(
        root,
        &["manifest", "views", ".", "--out", "foreign", "--json"],
        2,
    );
    assert_eq!(
        std::fs::read(root.join("foreign/index.html")).unwrap(),
        b"owned elsewhere"
    );
    run(
        root,
        &["manifest", "views", ".", "--out", "views", "--json"],
        1,
    );
    let html = std::fs::read_to_string(root.join("views/index.html")).unwrap();
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!html.contains("<script>alert(1)</script>"));
}
