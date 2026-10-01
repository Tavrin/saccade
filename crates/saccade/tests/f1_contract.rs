//! Generated-fixture acceptance checks for the F1 release spec.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

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
        .join(format!("saccade-{name}.v1.schema.json"));
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
            "compare",
            "baseline",
            "capture",
            "--out",
            "report",
            "--json=full",
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
    match v {
        Value::String(s) => assert!(!s.starts_with(prefix), "leaked path {s}"),
        Value::Object(m) => m.values().for_each(|v| no_prefix(v, prefix)),
        Value::Array(a) => a.iter().for_each(|v| no_prefix(v, prefix)),
        _ => {}
    }
}
#[test]
fn no_absolute_paths_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    let base = root.join("baseline");
    let cap = root.join("capture");
    let args = [
        "compare",
        base.to_str().unwrap(),
        cap.to_str().unwrap(),
        "--out",
        "report",
        "--json=full",
    ];
    let output = run(root, &args);
    no_prefix(&value(&output), root.to_str().unwrap());
    let mut absolute = args.to_vec();
    absolute.push("--record-absolute-paths");
    let output = run(root, &absolute);
    assert!(
        value(&output)["capture_dir"]
            .as_str()
            .unwrap()
            .starts_with(root.to_str().unwrap())
    );
    report(root);
    for args in [
        vec![
            "compare",
            base.to_str().unwrap(),
            cap.to_str().unwrap(),
            "--out",
            "report",
            "--json",
        ],
        vec!["explain", "report/saccade-report.v1.json", "--json"],
        vec![
            "view",
            base.to_str().unwrap(),
            cap.to_str().unwrap(),
            "--out",
            "view",
            "--json",
        ],
        vec![
            "runs",
            base.to_str().unwrap(),
            cap.to_str().unwrap(),
            "--json",
        ],
        vec![
            "rank",
            "baseline",
            "capture",
            "--out",
            "ranking",
            "--json=full",
        ],
    ] {
        let output = run(root, &args);
        assert!(
            output.status.code().unwrap() < 2,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        no_prefix(&value(&output), root.to_str().unwrap());
    }
    let html = std::fs::read_to_string(root.join("view/index.html")).unwrap();
    assert!(!html.contains(root.to_str().unwrap()));
    let out = run(
        root,
        &[
            "decide",
            "report/saccade-report.v1.json",
            "--entry",
            "b.png",
            "--question",
            "accept",
            "--answer",
            "accept",
            "--source",
            "human",
            "--json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let d: Value = serde_json::from_slice(
        &std::fs::read(root.join("report/saccade-decisions.v1.json")).unwrap(),
    )
    .unwrap();
    no_prefix(&d, root.to_str().unwrap());
    // A key saved elsewhere must still anchor decisions rebased by CLI unblind.
    let out = run(
        root,
        &[
            "view",
            "baseline",
            "capture",
            "--blind",
            "--out",
            "blind",
            "--key-out",
            "keys/key.json",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let key: Value =
        serde_json::from_slice(&std::fs::read(root.join("keys/key.json")).unwrap()).unwrap();
    let chosen = key["sets"]["b.png"]
        .as_array()
        .unwrap()
        .iter()
        .position(|label| label == "capture")
        .unwrap();
    let judged = json!({"schema":"saccade-decisions.v1","seed":key["seed"],"labels":["P1","P2"],"blind":true,"sets":[{"name":"b.png","decision":"accept","chosen_label":format!("P{}",chosen+1)}]});
    std::fs::write(
        root.join("blind/judged.json"),
        serde_json::to_vec(&judged).unwrap(),
    )
    .unwrap();
    std::fs::create_dir(root.join("moved")).unwrap();
    let out = run(
        root,
        &[
            "unblind",
            "blind/judged.json",
            "keys/key.json",
            "--out",
            "moved/judged.json",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let out = run(
        root,
        &["approve", "--decisions", "moved/judged.json", "--json"],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    no_prefix(&value(&out), root.to_str().unwrap());
}
#[test]
fn approve_report_resolves_and_binds_inputs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    report(root);
    let other = root.join("elsewhere");
    std::fs::create_dir(&other).unwrap();
    let out = run(
        &other,
        &[
            "approve",
            "--report",
            "../report/saccade-report.v1.json",
            "--all-failing",
            "--json",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(value(&out)["copied"].as_array().unwrap().len(), 2);
    assert_eq!(
        std::fs::read(root.join("baseline/b.png")).unwrap(),
        std::fs::read(root.join("capture/b.png")).unwrap()
    );
    assert_eq!(
        run(root, &["compare", "baseline", "capture", "--out", "report"])
            .status
            .code(),
        Some(0)
    );
    image(&root.join("capture"), "b.png", 45);
    let out = run(
        root,
        &[
            "approve",
            "--report",
            "report/saccade-report.v1.json",
            "--all-failing",
            "--json",
        ],
    );
    // The reviewed pairs now pass; use decisions to exercise adopted-image hash binding.
    assert_eq!(out.status.code(), Some(0));
    let hash = saccade_core::run::sha256_file(&root.join("capture/b.png")).unwrap();
    let d = json!({"schema":"saccade-decisions.v1","seed":0,"labels":["baseline","capture"],"dirs":["../baseline","../capture"],"sets":[{"name":"b.png","decision":"accept","sha256":[null,hash]}]});
    std::fs::write(
        root.join("report/decisions.json"),
        serde_json::to_vec(&d).unwrap(),
    )
    .unwrap();
    let out = run(
        &other,
        &[
            "approve",
            "--decisions",
            "../report/decisions.json",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    image(&root.join("capture"), "b.png", 46);
    let out = run(
        &other,
        &[
            "approve",
            "--decisions",
            "../report/decisions.json",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(2));
    schema("error", &value(&out));
}
#[test]
fn entries_pagination_filters_full_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    report(root);
    let out = run(
        root,
        &[
            "entries",
            "report/saccade-report.v1.json",
            "--status",
            "fail",
            "--name",
            "*.png",
            "--offset",
            "1",
            "--limit",
            "1",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let v = value(&out);
    schema("entries", &v);
    assert_eq!(v["total"], 2);
    assert_eq!(v["entries"][0]["name"], "c.png");
    assert!(v["entries"][0].get("diagnostics").is_some());
    assert!(v["next_cursor"].is_null());
}
#[test]
fn mcp_list_and_get_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    report(root);
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .current_dir(root)
        .args(["mcp", "--root", root.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for (id, name, args) in [
        (
            1,
            "saccade_list_entries",
            json!({"report_json":"report/saccade-report.v1.json","status":["fail"],"limit":1}),
        ),
        (
            2,
            "saccade_list_entries",
            json!({"report_json":"report/saccade-report.v1.json","status":["fail"],"cursor":"1","limit":1}),
        ),
        (
            3,
            "saccade_get_entry",
            json!({"report_json":"report/saccade-report.v1.json","name":"b.png"}),
        ),
        (
            4,
            "saccade_list_entries",
            json!({"report_json":"../outside.json"}),
        ),
    ] {
        writeln!(stdin,"{}",json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let lines: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let first = &lines[0]["result"]["structuredContent"];
    schema("entries", first);
    assert_eq!(first["next_cursor"], "1");
    assert_eq!(
        lines[1]["result"]["structuredContent"]["entries"][0]["name"],
        "c.png"
    );
    let entry = &lines[2]["result"]["structuredContent"];
    assert_eq!(entry["name"], "b.png");
    for key in ["paths", "hotspots", "diagnostics", "meta_diff"] {
        assert!(entry.get(key).is_some());
    }
    assert_eq!(lines[3]["result"]["isError"], true);
    schema("error", &lines[3]["result"]["structuredContent"]);
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
    let out = run(root, &["config", "--explain", "ui/a.png", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let v = value(&out);
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
            &[
                cmd,
                "parent.png",
                "capture.png",
                "--out",
                cmd,
                "--json=full",
            ],
        );
        assert_eq!(
            out.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v = value(&out);
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
            "--json=full",
        ],
    );
    assert_eq!(value(&out)["entries"][0]["status"], "error");
}

#[test]
fn entry_filters_apply_to_compare_identity_view_and_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fixture(root);
    for cmd in ["compare", "identity", "view", "runs"] {
        let mut args = vec![
            cmd,
            "baseline",
            "capture",
            "--entries",
            "a.*",
            "--entries",
            "c.*",
            "--out",
            cmd,
        ];
        args.push(if cmd == "compare" || cmd == "identity" {
            "--json=full"
        } else {
            "--json"
        });
        let out = run(root, &args);
        assert!(
            out.status.code().unwrap() < 2,
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v = value(&out);
        if cmd == "view" {
            assert_eq!(v["sets"], 2);
        } else if cmd == "runs" {
            assert_eq!(v["images"].as_array().unwrap().len(), 2);
        } else {
            assert_eq!(v["totals"]["total"], 2);
        }
    }
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
    let v = value(&out);
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
    let out=Command::new("python3").args(["-c","import sys,xml.etree.ElementTree as E; r=E.parse(sys.argv[1]).getroot(); assert len(r.findall('testcase'))==5; assert len(r.findall('.//failure'))==3; assert len(r.findall('.//skipped'))==1",root.join("results.xml").to_str().unwrap()]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for cmd in ["identity", "rank"] {
        let out = run(
            root,
            &[
                cmd,
                "baseline",
                "capture",
                "--out",
                cmd,
                "--junit",
                "results.xml",
            ],
        );
        assert_eq!(out.status.code(), Some(1));
        let parsed=Command::new("python3").args(["-c","import sys,xml.etree.ElementTree as E; assert len(E.parse(sys.argv[1]).getroot().findall('testcase'))==5",root.join("results.xml").to_str().unwrap()]).output().unwrap();
        assert!(parsed.status.success());
    }
    image(&root.join("seq-base"), "frame_000.png", 0);
    image(&root.join("seq-base"), "frame_001.png", 0);
    image(&root.join("seq-cap"), "frame_000.png", 0);
    image(&root.join("seq-cap"), "frame_001.png", 20);
    let out = run(
        root,
        &[
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
    let parsed=Command::new("python3").args(["-c","import sys,xml.etree.ElementTree as E; assert len(E.parse(sys.argv[1]).getroot().findall('testcase'))==2",root.join("results.xml").to_str().unwrap()]).output().unwrap();
    assert!(parsed.status.success());
}
