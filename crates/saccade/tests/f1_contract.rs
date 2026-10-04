//! Generated-fixture acceptance checks for the F1 release spec.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

#[path = "support/approval.rs"]
mod approval_support;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const PYTHON: &str = if cfg!(windows) { "python" } else { "python3" };

fn image(dir: &Path, name: &str, delta: u8) {
    std::fs::create_dir_all(dir).unwrap();
    let img = image::RgbImage::from_fn(24, 24, |x, y| {
        let v = 40 + ((x * 7 + y * 11) % 140) as u8;
        image::Rgb([v.saturating_add(delta), v, v])
    });
    img.save(dir.join(name)).unwrap();
}
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn value(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap()
}
fn schema(name: &str, v: &Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas")
        .join(format!(
            "saccade-{}.{}.schema.json",
            if name == "error" { "result" } else { name },
            if name == "error" { "v2" } else { "v1" }
        ));
    let document: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&document).unwrap();
    assert!(validator.is_valid(v), "schema {name}: {v}");
}
fn fixture(root: &Path) {
    for (name, delta) in [("a.png", 0), ("b.png", 35), ("c.png", 20)] {
        image(&root.join("baseline"), name, 0);
        image(&root.join("capture"), name, delta);
    }
}
fn report(root: &Path) {
    let out = run(
        root,
        &[
            "compare", "baseline", "capture", "--out", "report", "--json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn no_prefix(v: &Value, prefix: &str) {
    let normalized = saccade_core::paths::canonicalize(prefix).unwrap();
    let prefixes = [
        prefix.to_owned(),
        prefix.replace('\\', "/"),
        normalized.display().to_string(),
        saccade_core::paths::portable(&normalized),
    ];
    check_no_prefix(v, &prefixes);
}
fn check_no_prefix(v: &Value, prefixes: &[String]) {
    match v {
        Value::String(s) => assert!(
            prefixes.iter().all(|prefix| !s.starts_with(prefix)),
            "leaked path {s}"
        ),
        Value::Object(m) => m.values().for_each(|v| check_no_prefix(v, prefixes)),
        Value::Array(a) => a.iter().for_each(|v| check_no_prefix(v, prefixes)),
        _ => {}
    }
}
#[test]
fn no_absolute_paths_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    report(root);
    let file = root.join("report/saccade-report.v1.json");
    let full: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    no_prefix(&full, root.to_str().unwrap());
    let output = run(
        root,
        &[
            "compare", "baseline", "capture", "--out", "report", "--json",
        ],
    );
    let mut summary = value(&output);
    for action in summary["next_actions"].as_array_mut().unwrap() {
        assert_eq!(action["cwd"], root.to_str().unwrap());
        action.as_object_mut().unwrap().remove("cwd");
    }
    no_prefix(&summary, root.to_str().unwrap());
    let output = run(
        root,
        &[
            "compare",
            "baseline",
            "capture",
            "--out",
            "report",
            "--record-absolute-paths",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let full: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert!(Path::new(full["capture_dir"].as_str().unwrap()).is_absolute());
}

#[test]
fn approve_report_resolves_and_binds_inputs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    report(root);
    let decision = approval_support::draft(
        &root.join("report/saccade-report.v1.json"),
        &root.join("plan"),
        &[],
        false,
    );
    let other = root.join("elsewhere");
    std::fs::create_dir(&other).unwrap();
    let out = run(
        &other,
        &[
            "approve",
            "--report",
            "../report/saccade-report.v1.json",
            "--decisions",
            "../plan/decision.json",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(value(&out)["data"]["copied"].as_array().unwrap().len(), 2);
    no_prefix(&value(&out), root.to_str().unwrap());
    assert_eq!(
        std::fs::read(root.join("baseline/b.png")).unwrap(),
        std::fs::read(root.join("capture/b.png")).unwrap()
    );
    image(&root.join("capture"), "b.png", 46);
    let out = run(
        &other,
        &[
            "approve",
            "--report",
            "../report/saccade-report.v1.json",
            "--decisions",
            decision.to_str().unwrap(),
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(2));
    schema("error", &value(&out));
}

#[test]
fn init_templates_parse_and_refuse_overwrite() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    for t in ["renderer", "ui", "identity", "ml"] {
        let out = run(root, &["init", "--template", t, "--dir", t]);
        assert_eq!(out.status.code(), Some(0));
        let cfg =
            saccade_core::config::RunConfig::from_toml_file(&root.join(t).join("saccade.toml"))
                .unwrap();
        if t == "renderer" {
            assert!(cfg.meta.required);
            assert!(cfg.hotspot_fail.is_some());
        }
        let out = run(root, &["init", "--template", t, "--dir", t]);
        assert_eq!(out.status.code(), Some(2));
    }
}
#[test]
fn config_explain_identifies_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("saccade.toml"),"threshold=0.03\n[[override]]\nglob=\"ui/*\"\nmetric=\"max\"\n[[region]]\nname=\"text\"\nrect=[0,0,1,0.5]\n[[mask]]\nrect=[0,0,0.1,0.1]\n").unwrap();
    let out = run(
        root,
        &["inspect", "config", "--entry", "ui/a.png", "--json"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v = value(&out)["data"].clone();
    assert_eq!(v["reason"], "auto-loaded ./saccade.toml");
    assert_eq!(v["image"]["threshold"]["value"], 0.03);
    assert_eq!(v["image"]["metric"]["value"], "max");
    assert!(
        v["image"]["metric"]["source"]
            .as_str()
            .unwrap()
            .contains("override[0]")
    );
    assert_eq!(v["image"]["masks"].as_array().unwrap().len(), 1);
}
#[test]
fn file_pairs_use_capture_name_and_hashes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    image(root, "parent.png", 0);
    image(root, "capture.png", 20);
    for cmd in ["compare", "identity"] {
        let out = run(
            root,
            &[cmd, "parent.png", "capture.png", "--out", cmd, "--json"],
        );
        assert_eq!(
            out.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v: Value = serde_json::from_slice(
            &std::fs::read(root.join(cmd).join("saccade-report.v1.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(v["entries"][0]["name"], "capture.png");
        assert_eq!(v["totals"]["total"], 1);
        assert_eq!(
            v["entries"][0]["capture_sha256"].as_str().unwrap().len(),
            64
        );
    }
    std::fs::write(root.join("parent.saccade-meta.json"), r#"{"mode":"old"}"#).unwrap();
    std::fs::write(root.join("capture.saccade-meta.json"), r#"{"mode":"new"}"#).unwrap();
    let out = run(
        root,
        &[
            "compare",
            "parent.png",
            "capture.png",
            "--out",
            "compare",
            "--require-matching-meta",
            "--json",
        ],
    );
    assert_eq!(value(&out)["failing"][0]["status"], "error");
}

#[test]
fn noise_threshold_is_largest_observed_times_margin() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    image(&root.join("one"), "a.png", 0);
    image(&root.join("two"), "a.png", 2);
    image(&root.join("three"), "a.png", 3);
    let out = run(
        root,
        &[
            "noise",
            "one",
            "two",
            "three",
            "--margin",
            "1.5",
            "--metric",
            "p95",
            "--out",
            "noise.toml",
            "--json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value =
        serde_json::from_slice(&std::fs::read(root.join("noise.json")).unwrap()).unwrap();
    schema("noise", &v);
    let e = &v["entries"][0];
    assert_eq!(e["pairs"], 3);
    assert!(
        (e["suggested_threshold"].as_f64().unwrap() - e["p95"].as_f64().unwrap() * 1.5).abs()
            < 1e-12
    );
    let cfg = saccade_core::config::RunConfig::from_toml_file(&root.join("noise.toml")).unwrap();
    assert_eq!(cfg.overrides.len(), 1);
}
#[test]
fn junit_is_valid_xml_and_preserves_verdicts() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    image(&root.join("baseline"), "missing.png", 0);
    image(&root.join("capture"), "new.png", 0);
    std::fs::write(root.join("capture/b.png"), b"not an image").unwrap();
    let out = run(
        root,
        &[
            "compare",
            "baseline",
            "capture",
            "--out",
            "report",
            "--junit",
            "results.xml",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    let out=Command::new(PYTHON).args(["-c","import sys,xml.etree.ElementTree as E; r=E.parse(sys.argv[1]).getroot(); assert len(r.findall('testcase'))==5; assert len(r.findall('.//failure'))==4; assert len(r.findall('.//skipped'))==0",root.join("results.xml").to_str().unwrap()]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for cmd in ["identity", "rank"] {
        let mut args = if cmd == "rank" {
            vec!["experiment", "rank"]
        } else {
            vec!["identity"]
        };
        args.extend([
            "baseline",
            "capture",
            "--out",
            cmd,
            "--junit",
            "results.xml",
        ]);
        let out = run(root, &args);
        assert_eq!(out.status.code(), Some(if cmd == "rank" { 2 } else { 1 }));
        let parsed=Command::new(PYTHON).args(["-c","import sys,xml.etree.ElementTree as E; assert len(E.parse(sys.argv[1]).getroot().findall('testcase'))==5",root.join("results.xml").to_str().unwrap()]).output().unwrap();
        assert!(parsed.status.success());
    }
    image(&root.join("seq-base"), "frame_000.png", 0);
    image(&root.join("seq-base"), "frame_001.png", 0);
    image(&root.join("seq-cap"), "frame_000.png", 0);
    image(&root.join("seq-cap"), "frame_001.png", 20);
    let out = run(
        root,
        &[
            "experiment",
            "sequence",
            "seq-base",
            "seq-cap",
            "--out",
            "sequence",
            "--junit",
            "results.xml",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    let parsed=Command::new(PYTHON).args(["-c","import sys,xml.etree.ElementTree as E; assert len(E.parse(sys.argv[1]).getroot().findall('testcase'))==2",root.join("results.xml").to_str().unwrap()]).output().unwrap();
    assert!(parsed.status.success());
}
