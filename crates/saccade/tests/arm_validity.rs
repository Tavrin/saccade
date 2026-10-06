//! Generated-fixture arm identity, refusal and transport contracts.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
fn metadata() -> Value {
    json!({"fingerprint":{"schema":saccade_core::arms::FINGERPRINT_SCHEMA,"producer":{"binary":"sha256:binary","build":{"profile":"debug","features":[]}},"inputs":{"identity":"sha256:content"},"run":{"mode":"fixed","env":{},"session":"one","readiness":[{"criterion":{"name":"settled","parameters":{}},"reached":true,"observed":0.001}]}}})
}
fn write(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn captures(tmp: &Path) -> (PathBuf, PathBuf) {
    let a = tmp.join("a");
    let b = tmp.join("b");
    for p in [&a, &b] {
        std::fs::create_dir_all(p).unwrap();
        image::RgbImage::from_pixel(8, 8, image::Rgb([100; 3]))
            .save(p.join("frame.png"))
            .unwrap();
        write(&p.join("saccade-meta.json"), &metadata());
    }
    (a, b)
}
fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn run(args: &[&str], code: i32) -> Value {
    let o = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(code), "{o:?}");
    serde_json::from_slice(&o.stdout).unwrap()
}
#[test]
fn standalone_declared_variable_and_missing_identity_exit_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    let mut m = metadata();
    m["fingerprint"]["run"]["mode"] = json!("alternate");
    write(&b.join("saccade-meta.json"), &m);
    let v = run(
        &[
            "arms",
            "check",
            path(&a),
            path(&b),
            "--vary",
            "mode",
            "--json",
        ],
        0,
    );
    assert_eq!(v["result"], "valid_comparison");
    let v = run(&["arms", "check", path(&a), path(&b), "--json"], 3);
    assert_eq!(v["result"], "invalid_comparison");
    assert!(v.get("verdict").is_none());
    m["fingerprint"]["inputs"]
        .as_object_mut()
        .unwrap()
        .remove("identity");
    write(&b.join("saccade-meta.json"), &m);
    let v = run(
        &[
            "arms",
            "check",
            path(&a),
            path(&b),
            "--vary",
            "mode",
            "--json",
        ],
        4,
    );
    assert!(
        v["offending"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["key"].as_str().unwrap().ends_with("inputs.identity"))
    );
}
#[test]
fn standalone_comma_split_vary_and_ignore_match_repeated_flags() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    let mut ma = metadata();
    ma["fingerprint"]["run"]["env"] = json!({"setting":"old"});
    ma["timestamp"] = json!("before");
    ma["note"] = json!("before");
    write(&a.join("saccade-meta.json"), &ma);
    let mut mb = ma;
    mb["fingerprint"]["producer"]["binary"] = json!("sha256:other");
    mb["fingerprint"]["run"]["mode"] = json!("alternate");
    mb["fingerprint"]["run"]["env"] = json!({"setting":"new"});
    mb["timestamp"] = json!("after");
    mb["note"] = json!("after");
    write(&b.join("saccade-meta.json"), &mb);
    let repeated = run(
        &[
            "arms",
            "check",
            path(&a),
            path(&b),
            "--vary",
            "binary",
            "--vary",
            "mode",
            "--vary",
            "run.env",
            "--ignore",
            "timestamp",
            "--ignore",
            "note",
            "--ignore",
            "unused",
            "--json",
        ],
        0,
    );
    let comma = run(
        &[
            "arms",
            "check",
            path(&a),
            path(&b),
            "--vary",
            "binary,mode",
            "--vary",
            "run.env",
            "--ignore",
            "timestamp,note",
            "--ignore",
            "unused",
            "--json",
        ],
        0,
    );
    assert_eq!(comma, repeated);
    assert_eq!(comma["vary"], json!(["binary", "mode", "run.env"]));
    assert_eq!(comma["ignore"], json!(["timestamp", "note", "unused"]));
    assert_eq!(comma["ignored"].as_array().unwrap().len(), 2);
}
#[test]
fn compare_default_warns_and_strict_refuses_without_touching_previous_output() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    let mut m = metadata();
    m["fingerprint"]["producer"]["binary"] = json!("sha256:other");
    write(&b.join("saccade-meta.json"), &m);
    let out = tmp.path().join("out");
    let v = run(
        &["compare", path(&a), path(&b), "--out", path(&out), "--json"],
        0,
    );
    assert_eq!(v["verdict"], "pass");
    let report = out.join("saccade-report.v1.json");
    let before = std::fs::read(&report).unwrap();
    let v = run(
        &[
            "compare",
            path(&a),
            path(&b),
            "--require-valid-arms",
            "--out",
            path(&out),
            "--json",
        ],
        3,
    );
    assert_eq!(v["result"], "invalid_comparison");
    assert_eq!(before, std::fs::read(&report).unwrap());
    assert!(v.get("verdict").is_none());
    let report: Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(
        report["entries"][0]["meta_diff"][0]["key"],
        "producer.binary"
    );
}
#[test]
fn strict_config_declared_variable_and_ignores_are_echoed_on_success() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    let mut ma = metadata();
    ma["timestamp"] = json!("before");
    write(&a.join("saccade-meta.json"), &ma);
    let mut mb = metadata();
    mb["timestamp"] = json!("after");
    mb["fingerprint"]["run"]["mode"] = json!("alternate");
    write(&b.join("saccade-meta.json"), &mb);
    let cfg = tmp.path().join("config.toml");
    std::fs::write(&cfg,"require_valid_arms = true\nintended_variables = [\"mode\"]\narm_ignore = [\"timestamp\"]\n").unwrap();
    let out = tmp.path().join("out");
    let v = run(
        &[
            "compare",
            path(&a),
            path(&b),
            "--config",
            path(&cfg),
            "--out",
            path(&out),
            "--json",
        ],
        0,
    );
    assert_eq!(v["data"]["arm_validation"]["ignore"], json!(["timestamp"]));
    let v = run(
        &[
            "arms",
            "check",
            path(&a),
            path(&b),
            "--vary",
            "mode",
            "--ignore",
            "timestamp",
            "--ignore",
            "unused",
            "--json",
        ],
        0,
    );
    assert_eq!(v["ignore"], json!(["timestamp", "unused"]));
    assert_eq!(v["ignored"].as_array().unwrap().len(), 1);
}
#[test]
fn generic_toml_and_json_mapping_and_absent_content_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/arm-validity");
    let a = root.join("baseline/capture.json");
    let b = root.join("candidate/capture.json");
    let map = root.join("fingerprint-map.toml");
    let v = run(
        &[
            "arms",
            "check",
            path(&a),
            path(&b),
            "--fingerprint-map",
            path(&map),
            "--vary",
            "mode",
            "--json",
        ],
        0,
    );
    assert_eq!(v["result"], "valid_comparison");
    let tmp = tempfile::tempdir().unwrap();
    let json_map = tmp.path().join("map.json");
    let mut m = saccade_core::arms::FingerprintMap::read(&map).unwrap();
    m.fields.get_mut("inputs.identity").unwrap().path = "unrecorded_hash".into();
    write(&json_map, &serde_json::to_value(m).unwrap());
    let v = run(
        &[
            "arms",
            "check",
            path(&a),
            path(&b),
            "--fingerprint-map",
            path(&json_map),
            "--vary",
            "mode",
            "--json",
        ],
        4,
    );
    assert!(
        v["offending"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["key"] == "inputs.identity" && f["reason"] == "missing")
    );
}
#[cfg(feature = "graphics")]
#[test]
fn ablate_checks_every_repeat_and_render_verdict_commands_refuse() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    let mut m = metadata();
    m["fingerprint"]["run"]["readiness"][0]["reached"] = json!(false);
    write(&b.join("saccade-meta.json"), &m);
    let out = tmp.path().join("out");
    for args in [
        vec![
            "experiment",
            "ablate",
            path(&a),
            path(&b),
            "--require-valid-arms",
            "--out",
            path(&out),
            "--json",
        ],
        vec![
            "experiment",
            "reference",
            path(&b.join("frame.png")),
            path(&a.join("frame.png")),
            "--require-valid-arms",
            "--json",
        ],
    ] {
        let v = run(&args, 3);
        assert_eq!(v["result"], "invalid_comparison");
        assert!(!out.exists());
    }
    let v = run(
        &[
            "experiment",
            "ablate",
            "--base",
            path(&a),
            path(&b),
            "--arm",
            &format!("variant={}", a.display()),
            "--require-valid-arms",
            "--out",
            path(&out),
            "--json",
        ],
        3,
    );
    assert_eq!(v["result"], "invalid_comparison");
    assert!(!out.exists());
    let v = run(
        &[
            "experiment",
            "temporal",
            path(&a),
            path(&b),
            "--fps",
            "60",
            "--require-valid-arms",
            "--out",
            path(&out),
            "--json",
        ],
        3,
    );
    assert_eq!(v["result"], "invalid_comparison");
    let v = run(
        &[
            "localized-check",
            path(&a.join("frame.png")),
            path(&b.join("frame.png")),
            "--box",
            "0,0,1,1",
            "--require-valid-arms",
            "--out",
            path(&out),
            "--json",
        ],
        3,
    );
    assert_eq!(v["result"], "invalid_comparison");
    assert!(!out.exists());
}
#[cfg(feature = "mcp")]
#[test]
fn mcp_mirrors_standalone_and_compare_refusal() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    let mut m = metadata();
    m["fingerprint"]["run"]["session"] = json!("two");
    write(&b.join("saccade-meta.json"), &m);
    let map = a.join("map.json");
    write(
        &map,
        &json!({"fields":{"run.mode":{"path":"fingerprint.run.mode"}}}),
    );
    let out = tmp.path().join("out");
    std::fs::create_dir(&out).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args([
            "mcp",
            "--root",
            path(&a),
            "--root",
            path(&b),
            "--out-root",
            path(&out),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for (id, args) in [
        (1, json!({"operation":"arms_check","a":a,"b":b})),
        (
            2,
            json!({"operation":"compare","baseline_dir":a,"capture_dir":b,"out_dir":out.join("report"),"require_valid_arms":true}),
        ),
        (
            3,
            json!({"operation":"arms_check","a":a,"b":b,"fingerprint_map":map,"compare":"mapped_only"}),
        ),
    ] {
        writeln!(stdin,"{}",json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"saccade_measure","arguments":args}})).unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let lines = String::from_utf8(output.stdout).unwrap();
    let values = lines
        .lines()
        .map(|s| serde_json::from_str::<Value>(s).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 3);
    for v in values {
        let result = &v["result"]["structuredContent"];
        if v["id"] == 3 {
            assert_eq!(result["compare"], "mapped_only");
            assert_eq!(result["exit_code"], 0);
            assert_eq!(result["result"], "valid_comparison");
            continue;
        }
        assert_eq!(result["result"], "invalid_comparison", "{v}");
        assert_eq!(result["exit_code"], 3);
        assert!(result.get("verdict").is_none());
    }
    assert!(!out.join("report").exists());
}

#[test]
fn mapped_directory_compare_resolves_config_mapping_and_reports_canonical_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/arm-validity");
    for (destination, source) in [(&a, "baseline"), (&b, "candidate")] {
        for name in [
            "capture.json",
            "prepared.json",
            "settings.json",
            "readiness.json",
        ] {
            std::fs::copy(root.join(source).join(name), destination.join(name)).unwrap();
        }
    }
    let map = tmp.path().join("map.toml");
    std::fs::copy(root.join("fingerprint-map.toml"), &map).unwrap();
    let cfg = tmp.path().join("saccade.toml");
    std::fs::write(&cfg,"require_valid_arms = true\nfingerprint_map = \"map.toml\"\nmeta_name = \"capture.json\"\nintended_variables = [\"mode\"]\n").unwrap();
    let out = tmp.path().join("out");
    let v = run(
        &[
            "compare",
            path(&a),
            path(&b),
            "--config",
            path(&cfg),
            "--out",
            path(&out),
            "--json",
        ],
        0,
    );
    assert_eq!(v["data"]["arm_validation"]["result"], "valid_comparison");
    let report: Value =
        serde_json::from_slice(&std::fs::read(out.join("saccade-report.v1.json")).unwrap())
            .unwrap();
    assert!(
        report["entries"][0]["intended_variables"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["key"] == "run.mode")
    );
    assert_eq!(
        report["entries"][0]["meta_diff"].as_array().unwrap().len(),
        0
    );
}
#[test]
fn predicate_order_and_declared_observations_match_the_measurement_path() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    let mut ma = metadata();
    let mut second = ma["fingerprint"]["run"]["readiness"][0].clone();
    second["criterion"]["name"] = json!("pipeline_ready");
    ma["fingerprint"]["run"]["readiness"]
        .as_array_mut()
        .unwrap()
        .push(second);
    ma["capture.timestamp"] = json!("before");
    write(&a.join("saccade-meta.json"), &ma);
    let mut mb = ma.clone();
    mb["fingerprint"]["run"]["readiness"]
        .as_array_mut()
        .unwrap()
        .reverse();
    mb["fingerprint"]["run"]["readiness"][1]["observed"] = json!(0.002);
    mb["capture.timestamp"] = json!("after");
    write(&b.join("saccade-meta.json"), &mb);
    let out = tmp.path().join("out");
    let v = run(
        &[
            "compare",
            path(&a),
            path(&b),
            "--require-valid-arms",
            "--intended-variable",
            "run.readiness.settled.observed",
            "--arm-ignore",
            "timestamp",
            "--out",
            path(&out),
            "--json",
        ],
        0,
    );
    assert_eq!(v["data"]["arm_validation"]["ignore"], json!(["timestamp"]));
}
#[test]
fn standalone_result_obeys_its_schema_and_invalid_has_no_pixel_verdict() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    let mut m = metadata();
    m["fingerprint"]["producer"]["binary"] = json!("sha256:other");
    write(&b.join("saccade-meta.json"), &m);
    let v = run(&["arms", "check", path(&a), path(&b), "--json"], 3);
    let schema: Value = serde_json::from_slice(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../saccade-core/schemas/saccade-arms-check.v1.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&v));
    let mut invalid = v;
    invalid["result"] = json!("pass");
    assert!(!validator.is_valid(&invalid));
}

#[test]
fn mapped_binary_derivation_is_visible_and_requires_a_declaration() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    for (root, binary, cache) in [
        (&a, "sha256:before", "cache-before"),
        (&b, "sha256:after", "cache-after"),
    ] {
        let mut m = metadata();
        m["fingerprint"]["producer"]["binary"] = json!(binary);
        m["cache_key"] = json!(cache);
        write(&root.join("saccade-meta.json"), &m);
    }
    let map = tmp.path().join("map.toml");
    for derives in [true, false] {
        std::fs::write(
            &map,
            format!(
                "[fields.\"producer.binary\"]\npath = \"fingerprint.producer.binary\"\n{}",
                if derives {
                    "derives = [\"cache_key\"]\n"
                } else {
                    ""
                }
            ),
        )
        .unwrap();
        let code = if derives { 0 } else { 3 };
        for token in ["producer.binary", "binary", "producer.*"] {
            let v = run(
                &[
                    "arms",
                    "check",
                    path(&a),
                    path(&b),
                    "--fingerprint-map",
                    path(&map),
                    "--vary",
                    token,
                    "--json",
                ],
                code,
            );
            if derives {
                assert_eq!(v["offending"], json!([]));
                assert_eq!(v["ignored"], json!([]));
                assert_eq!(
                    v["covered_by_derivation"],
                    json!([{
                        "key": "frame.png:cache_key",
                        "baseline": "cache-before",
                        "baseline_state": "value",
                        "capture": "cache-after",
                        "capture_state": "value",
                        "reason": "covered_by_derivation"
                    }])
                );
            } else {
                assert_eq!(v["offending"][0]["key"], "frame.png:cache_key");
                assert_eq!(v["covered_by_derivation"], json!([]));
            }
        }
        let cfg = tmp.path().join("config.toml");
        std::fs::write(&cfg, "require_valid_arms = true\nfingerprint_map = \"map.toml\"\nintended_variables = [\"binary\"]\n").unwrap();
        let out = tmp
            .path()
            .join(if derives { "covered" } else { "undeclared" });
        run(
            &[
                "compare",
                path(&a),
                path(&b),
                "--config",
                path(&cfg),
                "--out",
                path(&out),
                "--json",
            ],
            code,
        );
        if derives {
            let report: Value =
                serde_json::from_slice(&std::fs::read(out.join("saccade-report.v1.json")).unwrap())
                    .unwrap();
            assert_eq!(
                report["entries"][0]["covered_by_derivation"],
                json!([{"key":"cache_key","baseline":"cache-before","capture":"cache-after"}])
            );
            assert_eq!(
                report["entries"][0]["meta_diff"]
                    .as_array()
                    .map(Vec::len)
                    .unwrap_or(0),
                0
            );
        } else {
            assert!(!out.exists());
        }
    }
}

#[test]
fn mapped_selection_outcomes_override_and_json_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.json");
    let b = tmp.path().join("b.json");
    let map = tmp.path().join("map.json");
    // Native identity sources keep all-mode requirements independently satisfied.
    let full_a = metadata();
    let full_b = metadata();
    let mut av = full_a;
    av["timing"] = json!({"gpu":1});
    let mut bv = full_b;
    bv["timing"] = json!({"gpu":9});
    write(&a, &av);
    write(&b, &bv);
    let mut policy =
        json!({"compare":"mapped_only","fields":{"run.mode":{"path":"fingerprint.run.mode"}}});
    write(&map, &policy);
    let args = [
        "arms",
        "check",
        path(&a),
        path(&b),
        "--fingerprint-map",
        path(&map),
        "--json",
    ];
    let v = run(&args, 0);
    assert_eq!(v["compare"], "mapped_only");
    assert!(
        v["unmapped"]["keys"]
            .as_array()
            .unwrap()
            .contains(&json!("timing.gpu"))
    );
    let schema: Value = serde_json::from_str(include_str!(
        "../../saccade-core/schemas/saccade-arms-check.v1.schema.json"
    ))
    .unwrap();
    assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&v));
    let mut all_args = args.to_vec();
    all_args.extend(["--compare", "all"]);
    let v = run(&all_args, 3);
    assert_eq!(v["compare"], "all");
    assert!(
        v["offending"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["key"] == "timing.gpu")
    );
    policy["compare"] = json!("all");
    policy["outcomes"] = json!(["timing.*"]);
    write(&map, &policy);
    let v = run(&args, 0);
    assert_eq!(v["outcomes"]["keys"], json!(["timing.gpu"]));
    bv["fingerprint"]["run"]["mode"] = json!("alternate");
    write(&b, &bv);
    let mut selected = args.to_vec();
    selected.extend(["--compare", "mapped-only"]);
    let v = run(&selected, 3);
    assert_eq!(v["offending"][0]["key"], "run.mode");
    bv["fingerprint"]["run"]
        .as_object_mut()
        .unwrap()
        .remove("mode");
    write(&b, &bv);
    let v = run(&selected, 4);
    assert_eq!(v["offending"][0]["capture_state"], "missing");
    let error = run(
        &[
            "arms",
            "check",
            path(&a),
            path(&b),
            "--compare",
            "mapped-only",
            "--json",
        ],
        2,
    );
    assert!(error.to_string().contains("requires a fingerprint map"));
}

#[test]
fn excluded_keys_remain_bounded_across_directory_images() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = captures(tmp.path());
    for root in [&a, &b] {
        std::fs::copy(root.join("frame.png"), root.join("second.png")).unwrap();
        let mut m = metadata();
        for i in 0..100 {
            m[format!("outcome_{i:03}")] = json!(i);
        }
        write(&root.join("saccade-meta.json"), &m);
    }
    let map = tmp.path().join("map.json");
    write(
        &map,
        &json!({"compare":"mapped_only","fields":{"run.mode":{"path":"fingerprint.run.mode"}}}),
    );
    let v = run(
        &[
            "arms",
            "check",
            path(&a),
            path(&b),
            "--fingerprint-map",
            path(&map),
            "--json",
        ],
        0,
    );
    assert!(v["unmapped"]["count"].as_u64().unwrap() >= 100);
    assert_eq!(v["unmapped"]["keys"].as_array().unwrap().len(), 64);
    assert_eq!(v["unmapped"]["truncated"], true);
    let c = saccade_core::arms::check_paths(
        &a,
        &b,
        &saccade_core::meta::MetaOptions {
            fingerprint_map: Some(map),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(v["unmapped"]["count"], json!(c.unmapped.count));
}

#[test]
fn strict_reports_use_the_same_mapped_and_outcome_selection() {
    for (mode, outcomes) in [("mapped_only", json!([])), ("all", json!(["results.*"]))] {
        let tmp = tempfile::tempdir().unwrap();
        let (a, b) = captures(tmp.path());
        let mut m = metadata();
        m["results"] = json!({"score":99});
        write(&b.join("saccade-meta.json"), &m);
        let map = tmp.path().join("map.json");
        write(
            &map,
            &json!({"compare":mode,"outcomes":outcomes,"fields":{"run.mode":{"path":"fingerprint.run.mode"}}}),
        );
        let out = tmp.path().join("report");
        let v = run(
            &[
                "identity",
                path(&a),
                path(&b),
                "--require-valid-arms",
                "--fingerprint-map",
                path(&map),
                "--out",
                path(&out),
                "--json",
            ],
            0,
        );
        assert_eq!(v["verdict"], "pass");
        let report: Value =
            serde_json::from_slice(&std::fs::read(out.join("saccade-report.v1.json")).unwrap())
                .unwrap();
        let receipt = &report["config"]["meta"]["arm_validation"];
        assert_eq!(receipt["exit_code"], 0);
        assert_eq!(receipt["compare"], mode);
        assert_eq!(v["data"]["arm_validation"]["compare"], mode);
        let excluded = if mode == "all" {
            &receipt["outcomes"]
        } else {
            &receipt["unmapped"]
        };
        assert!(
            excluded["keys"]
                .as_array()
                .unwrap()
                .contains(&json!("results.score"))
        );
    }
}
