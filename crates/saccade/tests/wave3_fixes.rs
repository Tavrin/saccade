//! Wave 3 contract discovery, nested reader and published-schema regressions.
#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::{path::Path, process::Command};
const BIN: &str = env!("CARGO_BIN_EXE_saccade");
const IDS: &[&str] = &[
    "saccade-brand-source.v1",
    "saccade-brand-review.v1",
    "saccade-ui-source.v1",
    "saccade-ui-review.v1",
    "saccade-tesseract.v1",
    "saccade-perf-plan.v1",
    "saccade-perf-pairs.v1",
    "saccade-motion-review.v1",
    "saccade-motion-vectors.v1",
    "saccade-vector-buffer.v1",
    "saccade-asset-views.v1",
    "saccade-asset-view-report.v1",
];
#[test]
fn w3_f11_doctor_schema_inventory() {
    let output = Command::new(BIN)
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    let ids: Vec<_> = v["schemas"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|v| v.as_array().unwrap())
        .collect();
    for id in IDS {
        assert!(ids.iter().any(|v| *v == id), "missing {id}");
    }
}
#[test]
fn w3_f10_schema_ids_are_constants() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../saccade-core/schemas");
    for id in IDS {
        let s: Value =
            serde_json::from_slice(&std::fs::read(dir.join(format!("{id}.schema.json"))).unwrap())
                .unwrap();
        let validator = jsonschema::validator_for(&s).unwrap();
        // Validate an otherwise well-shaped UI source as in the review reproduction.
        if *id == "saccade-ui-source.v1" {
            let mut body = json!({"schema":id,"capture_sha256":"a".repeat(64),"dimensions":[16,16],"kind":"dom","producer":{},"nodes":[],"complete":true});
            validator.validate(&body).unwrap();
            body["schema"] = json!("saccade-ui-source.v99");
            assert!(!validator.is_valid(&body), "v99 validated against v1");
        }
        assert_eq!(s["properties"]["schema"]["const"], *id, "{id}");
        let id_validator = jsonschema::validator_for(&s["properties"]["schema"]).unwrap();
        id_validator.validate(&json!(id)).unwrap();
        assert!(!id_validator.is_valid(&json!(id.replace(".v1", ".v99"))));
        fn check_defs(v: &Value) {
            if let Some(props) = v.get("properties")
                && props.get("schema").is_some_and(|s| {
                    s.get("description")
                        .and_then(Value::as_str)
                        .is_some_and(|description| IDS.iter().any(|id| description.contains(id)))
                })
            {
                assert!(
                    props["schema"]["const"].is_string(),
                    "unconstrained nested schema: {v}"
                );
            }
            if let Some(defs) = v.get("$defs").and_then(Value::as_object) {
                for def in defs.values() {
                    check_defs(def);
                }
            }
        }
        check_defs(&s);
    }
}
fn paired() -> saccade_core::paired_stats::Samples {
    use saccade_core::{
        evidence::canonical::{self, Digest},
        paired_stats::*,
    };
    let plan = Plan {
        schema: "saccade-perf-plan.v1".into(),
        independent_unit: "run".into(),
        stopping_rule: "fixed_count".into(),
        pairs: (0..6)
            .map(|i| Assignment {
                id: i.to_string(),
                order: if i % 2 == 0 { "ab" } else { "ba" }.into(),
            })
            .collect(),
        confidence: 0.95,
        resamples: 1024,
        seed: 1,
    };
    let observations = plan
        .pairs
        .iter()
        .map(|p| Observation {
            pair_id: p.id.clone(),
            order: p.order.clone(),
            before_ms: 10.,
            after_ms: 9.,
        })
        .collect();
    Samples {
        schema: "saccade-perf-pairs.v1".into(),
        before_perf_sha256: Digest::of_bytes(b"b"),
        after_perf_sha256: Digest::of_bytes(b"a"),
        plan_sha256: canonical::digest(&plan).unwrap(),
        plan,
        observations,
    }
}
#[cfg(feature = "graphics")]
#[test]
fn w3_f08_nested_reader_versions_and_fields() {
    use saccade_core::perf::{CapturePerf, PerfDiff};
    let t = tempfile::tempdir().unwrap();
    for name in ["b", "a"] {
        let dir = t.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        image::RgbImage::from_pixel(2, 2, image::Rgb([40, 50, 60]))
            .save(dir.join("scene.png"))
            .unwrap();
    }
    let out = Command::new(BIN)
        .current_dir(t.path())
        .args(["compare", "b", "a", "--out", "report"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let path = t.path().join("report/saccade-report.v1.json");
    let mut report: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let perf = CapturePerf::parse(&json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":10.,"samples":6,"stat":"p50"},"terms":[],"counters":{}}).to_string(), "fixture").unwrap();
    let mut diff = PerfDiff::between(&perf, &perf, None, 3.);
    diff.attach_robust(saccade_core::paired_stats::estimate(&paired()).unwrap());
    report["perf_diff"] = serde_json::to_value(diff).unwrap();
    std::fs::write(&path, report.to_string()).unwrap();
    let out = Command::new(BIN)
        .args(["inspect", "exclusions"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    for pointer in [
        "/perf_diff/robust_effect/samples",
        "/perf_diff/robust_effect/samples/plan",
    ] {
        let mut v = report.clone();
        let nested = v.pointer_mut(pointer).unwrap();
        let schema = nested["schema"].as_str().unwrap().replace(".v1", ".v2");
        nested["schema"] = json!(schema);
        std::fs::write(&path, v.to_string()).unwrap();
        let out = Command::new(BIN)
            .args(["inspect", "exclusions"])
            .arg(&path)
            .args(["--json"])
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.status.code(), Some(2));
        assert!(
            text.contains("version_skew") && text.contains("upgrade"),
            "{text}"
        );
        let mut v = report.clone();
        v.pointer_mut(pointer).unwrap()["future_field"] = json!(true);
        std::fs::write(&path, v.to_string()).unwrap();
        let out = Command::new(BIN)
            .args(["inspect", "exclusions"])
            .arg(&path)
            .args(["--json"])
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            text.contains("version_skew") && text.contains("upgrade"),
            "{text}"
        );
    }
}
