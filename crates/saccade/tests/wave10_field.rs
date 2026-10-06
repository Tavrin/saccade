//! Coordinator-run field feedback proofs with generated fixtures only.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]
use serde_json::{Value, json};
use std::{path::Path, process::Command};
fn text(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn call(args: &[&str], code: i32) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_saccade"))
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(code),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn write(p: &Path, v: &Value) {
    std::fs::write(p, serde_json::to_vec(v).unwrap()).unwrap();
}
fn arm() -> Value {
    json!({"fingerprint":{"schema":"saccade-fingerprint.v1","producer":{"binary":"sha256:b","build":{"profile":"debug"}},"inputs":{"identity":"sha256:i"},"run":{"mode":"fixed","env":{},"session":"one","readiness":[{"criterion":{"name":"warmup","parameters":{"limit":100}},"reached":true,"observed":12}]}},"content":{"preparer":null}})
}
#[test]
#[ignore = "heavy: wave10-cli"]
fn schema_discovery_and_perf_validator_use_exact_shipped_documents() {
    let temp = tempfile::tempdir().unwrap();
    let listed = call(&["schema", "list", "--json"], 0);
    for id in listed["ids"].as_array().unwrap() {
        let id = id.as_str().unwrap();
        let got = call(&["schema", "get", id], 0);
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../saccade-core/schemas/{id}.schema.json"));
        assert_eq!(
            got,
            serde_json::from_slice::<Value>(&std::fs::read(file).unwrap()).unwrap()
        );
    }
    let perf = temp.path().join("perf.json");
    let mut v = json!({"schema":"saccade-perf.v2","kind":"measurement","unit":"ms","frame":{"value":1,"samples":8,"stat":"mean"},"terms":[],"counters":{}});
    write(&perf, &v);
    assert_eq!(
        call(&["perf", "validate", text(&perf), "--json"], 0)["valid"],
        true
    );
    v["kind"] = json!("invalid");
    write(&perf, &v);
    assert_eq!(
        call(&["perf", "validate", text(&perf), "--json"], 1)["valid"],
        false
    );
}
#[test]
#[ignore = "heavy: wave10-cli"]
fn arm_null_ignore_readiness_and_directory_record_policies() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a.json");
    let b = temp.path().join("b.json");
    let va = arm();
    let mut vb = va.clone();
    write(&a, &va);
    write(&b, &vb);
    call(&["arms", "check", text(&a), text(&b), "--json"], 0);
    vb["content"]["preparer"] = json!({"commit":"one","dirty":false});
    write(&b, &vb);
    call(&["arms", "check", text(&a), text(&b), "--json"], 3);
    let ignored = call(
        &[
            "arms",
            "check",
            text(&a),
            text(&b),
            "--ignore",
            "preparer",
            "--json",
        ],
        0,
    );
    assert_eq!(ignored["ignored"][0]["baseline_state"], "null");
    vb["content"].as_object_mut().unwrap().remove("preparer");
    write(&b, &vb);
    call(&["arms", "check", text(&a), text(&b), "--json"], 4);
    let ignored = call(
        &[
            "arms",
            "check",
            text(&a),
            text(&b),
            "--ignore",
            "preparer",
            "--json",
        ],
        0,
    );
    assert_eq!(ignored["ignored"][0]["capture_state"], "missing");
    let mut va = arm();
    va["fingerprint"]["run"]["readiness"][0]["reached"] = json!(false);
    write(&a, &va);
    write(&b, &va);
    let matched = call(
        &[
            "arms",
            "check",
            text(&a),
            text(&b),
            "--allow-unreached",
            "warmup",
            "--json",
        ],
        0,
    );
    assert_eq!(matched["allowed_unreached"][0]["baseline_observed"], 12);
    assert_eq!(matched["allowed_unreached"][0]["capture_observed"], 12);
    va["fingerprint"]["run"]["readiness"][0]["observed"] = json!(13);
    write(&b, &va);
    call(
        &[
            "arms",
            "check",
            text(&a),
            text(&b),
            "--allow-unreached",
            "warmup",
            "--json",
        ],
        3,
    );
    va["fingerprint"]["run"]["readiness"][0]["observed"] = json!(12);
    va["fingerprint"]["run"]["readiness"][0]["reached"] = json!(true);
    write(&b, &va);
    call(
        &[
            "arms",
            "check",
            text(&a),
            text(&b),
            "--allow-unreached",
            "warmup",
            "--json",
        ],
        3,
    );
    let da = temp.path().join("run-a");
    let db = temp.path().join("run-b");
    for dir in [&da, &db] {
        std::fs::create_dir(dir).unwrap();
        std::fs::write(dir.join("frame.png"), b"not decoded by arms").unwrap();
        write(&dir.join("capture.json"), &arm());
    }
    let missing = call(&["arms", "check", text(&da), text(&db), "--json"], 4);
    assert!(
        missing["diagnostics"][0]
            .as_str()
            .unwrap()
            .contains("record_files")
    );
    let map = temp.path().join("map.toml");
    std::fs::write(&map, "record_files = ['capture.json']\n").unwrap();
    call(
        &[
            "arms",
            "check",
            text(&da),
            text(&db),
            "--fingerprint-map",
            text(&map),
            "--json",
        ],
        0,
    );
}
#[test]
#[ignore = "heavy: wave10-cli"]
fn repeat_noise_scope_and_float_map_artifacts() {
    let tmp = tempfile::tempdir().unwrap();
    let image = |name: &str, v: u8| {
        let p = tmp.path().join(name);
        image::RgbaImage::from_pixel(32, 32, image::Rgba([v, v, v, 255]))
            .save(&p)
            .unwrap();
        p
    };
    let a = image("a.png", 100);
    let b = image("b.png", 101);
    let shifted = image("shift.png", 110);
    let r1 = image("r1.png", 99);
    let r2 = image("r2.png", 101);
    let noise = tmp.path().join("noise.json");
    let built = call(
        &[
            "noise",
            "build",
            text(&r1),
            text(&r2),
            "--out",
            text(&noise),
            "--json",
        ],
        0,
    );
    assert_eq!(built["method"], "max_pairwise_rgb_luminance_envelope_v1");
    for (candidate, code, class) in [
        (&b, 0, "texture_noise_only"),
        (&shifted, 1, "systematic_shift"),
    ] {
        let out = tmp.path().join(class);
        let result = call(
            &[
                "render-evidence",
                text(&a),
                text(candidate),
                "--out",
                text(&out),
                "--noise-from",
                text(&r1),
                text(&r2),
                "--export-maps",
                "--json",
            ],
            code,
        );
        assert_eq!(result["scope"], "whole_frame_unmasked");
        assert_eq!(result["entries"][0]["class"], class);
        let report: Value =
            serde_json::from_slice(&std::fs::read(out.join("saccade-report.v1.json")).unwrap())
                .unwrap();
        let index: Value = serde_json::from_slice(
            &std::fs::read(
                out.join(
                    report["entries"][0]["field_evidence"]["maps"]
                        .as_str()
                        .unwrap(),
                ),
            )
            .unwrap(),
        )
        .unwrap();
        let maps = &index["maps"];
        assert!(maps.as_array().unwrap().len() > 1);
        for m in maps.as_array().unwrap() {
            assert!(out.join(m["npy"].as_str().unwrap()).is_file());
            assert!(out.join(m["exr"].as_str().unwrap()).is_file());
        }
        assert!(
            std::fs::read_to_string(out.join("index.html"))
                .unwrap()
                .contains("whole_frame_unmasked")
        );
    }
    let out = tmp.path().join("scoped");
    call(
        &[
            "render-evidence",
            text(&a),
            text(&a),
            "--require-scope",
            "--out",
            text(&out),
            "--json",
        ],
        1,
    );
}
