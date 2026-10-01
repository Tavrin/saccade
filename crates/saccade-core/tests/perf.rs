//! Generated run-level attribution evidence and its failure boundaries.
#![allow(clippy::unwrap_used, missing_docs)]
use saccade_core::perf::{CapturePerf, PerfDiff, PerfNoise, PerfOptions};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn capture(frame: f64, pass: f64) -> Value {
    json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":frame,"samples":4,"stat":"p50"},
    "terms":[{"id":"render","kind":"pass","value":pass},{"id":"idle","kind":"gap","value":frame-pass},
    {"id":"render/detail","parent":"render","kind":"scope","value":pass/2.0}],
    "counters":{"render":{"draws":100,"triangles":1000}}})
}
fn parse(v: &Value) -> CapturePerf {
    CapturePerf::parse(&v.to_string(), "generated.json").unwrap()
}
fn write(dir: &Path, v: &Value) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("saccade-perf.json"), v.to_string()).unwrap();
}

#[test]
fn strict_validation_returns_typed_errors_for_malformed_evidence() {
    let good = capture(10.0, 8.0);
    let mut invalid = Vec::new();
    for (pointer, value) in [
        ("/schema", json!("other")),
        ("/unit", json!("s")),
        ("/frame/value", json!(-1)),
        ("/frame/samples", json!(0)),
        ("/frame/stat", json!("unknown")),
        ("/terms/0/value", json!(-1)),
        ("/terms/2/parent", json!("missing")),
        ("/terms/1/id", json!("render")),
    ] {
        let mut v = good.clone();
        *v.pointer_mut(pointer).unwrap() = value;
        invalid.push(v);
    }
    let duplicate_counter = good
        .to_string()
        .replace("\"draws\":100", "\"draws\":100,\"draws\":200");
    assert!(CapturePerf::parse(&duplicate_counter, "x").is_err());
    let mut v = good.clone();
    v["unknown"] = json!(1);
    invalid.push(v);
    let mut v = good.clone();
    v["terms"][0]["spread"] = json!({"min":9,"max":10});
    invalid.push(v);
    let mut v = good.clone();
    v["terms"][0]["kind"] = json!("scope");
    v["terms"][0]["parent"] = json!("render/detail");
    invalid.push(v);
    let mut v = good.clone();
    v["counters"]["missing"] = json!({"n":1});
    invalid.push(v);
    let mut v = good.clone();
    v["terms"][0]["value"] = json!(1.7e308);
    v["terms"][1]["value"] = json!(1.7e308);
    invalid.push(v);
    for v in invalid {
        let e = CapturePerf::parse(&v.to_string(), "generated.json").unwrap_err();
        assert_eq!(e.path, "generated.json");
        assert!(!e.message.is_empty());
    }
    for raw in [
        "{}",
        "null",
        "{",
        "{\"schema\":\"saccade-perf.v1\",\"schema\":\"other\"}",
    ] {
        assert!(CapturePerf::parse(raw, "x").is_err());
    }
    assert!(
        PerfOptions {
            name: "../x".into(),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        PerfOptions {
            k: f64::NAN,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn exact_ids_scopes_zero_denominators_and_counter_moves_are_preserved() {
    let mut b = capture(10.0, 8.0);
    let mut a = capture(12.0, 9.0);
    b["terms"][1]["value"] = json!(1.0);
    a["terms"][1]["id"] = json!("idle-new");
    a["counters"]["render"] = json!({"draws":90,"triangles":2000,"new-count":42});
    b["counters"]["render"]["old-count"] = json!(7);
    b["counters"]["render"]["zero"] = json!(0);
    a["counters"]["render"]["zero"] = json!(1);
    let floor = PerfNoise {
        frame: 1.0,
        terms: [("render".into(), 0.2), ("render/detail".into(), 0.5)].into(),
    };
    let d = PerfDiff::between(&parse(&b), &parse(&a), Some(&floor), 3.0);
    assert_eq!(d.unattributed_before, 1.0);
    assert_eq!(d.unattributed_after, 0.0);
    assert_eq!(d.warnings.len(), 1);
    let render = d.terms.iter().find(|t| t.id == "render").unwrap();
    assert_eq!(render.change.beyond_noise, Some(true));
    assert_eq!(render.share_before, Some(0.8));
    assert_eq!(render.counters[0].name, "triangles");
    assert_eq!(render.counters[0].change.delta_pct, Some(100.0));
    for (id, status) in [("idle", "disappeared"), ("idle-new", "appeared")] {
        let t = d.terms.iter().find(|t| t.id == id).unwrap();
        assert_eq!(t.change.status, status);
        assert_eq!(t.change.delta, None);
    }
    for (id, status) in [("old-count", "disappeared"), ("new-count", "appeared")] {
        let c = render.counters.iter().find(|c| c.name == id).unwrap();
        assert_eq!(c.change.status, status);
        assert_eq!(c.change.delta, None);
    }
    assert_eq!(
        render
            .counters
            .iter()
            .find(|c| c.name == "zero")
            .unwrap()
            .change
            .delta_pct,
        None
    );
    assert_eq!(d.flags(true), (false, true));
    let none = PerfDiff::between(&parse(&b), &parse(&b), None, 3.0);
    assert_eq!(none.flags(true), (false, false));
    let mut changed = a.clone();
    changed["terms"][0]["kind"] = json!("scope");
    changed["terms"][0]["parent"] = json!("idle-new");
    changed["terms"][1]["kind"] = json!("pass");
    let mismatch = PerfDiff::between(&parse(&a), &parse(&changed), Some(&floor), 3.0);
    assert_eq!(
        mismatch
            .terms
            .iter()
            .find(|t| t.id == "render")
            .unwrap()
            .change
            .status,
        "not_comparable"
    );
    let boundary = PerfDiff::between(
        &parse(&capture(10.0, 8.0)),
        &parse(&capture(11.0, 9.0)),
        Some(&PerfNoise {
            frame: 1.0,
            terms: [("render".into(), 0.5)].into(),
        }),
        2.0,
    );
    assert_eq!(
        boundary
            .terms
            .iter()
            .find(|t| t.id == "render")
            .unwrap()
            .change
            .beyond_noise,
        Some(false)
    );
}

#[test]
fn repeat_noise_ranges_include_scopes_and_exclude_incomplete_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let dirs: Vec<PathBuf> = (0..3).map(|i| tmp.path().join(i.to_string())).collect();
    for (i, d) in dirs.iter().enumerate() {
        let mut v = capture(10.0 + i as f64, 8.0 + i as f64);
        if i == 2 {
            v["terms"][1]["id"] = json!("different-id");
        }
        write(d, &v);
    }
    let (floor, warnings) = saccade_core::perf::noise(&dirs, "saccade-perf.json").unwrap();
    let f = floor.unwrap();
    assert_eq!(f.frame, 2.0);
    assert_eq!(f.terms["render"], 2.0);
    assert_eq!(f.terms["render/detail"], 1.0);
    assert!(!f.terms.contains_key("idle"));
    assert_eq!(warnings.len(), 2);
    std::fs::write(
        tmp.path().join("floor.json"),
        json!({"perf_noise":f}).to_string(),
    )
    .unwrap();
    assert_eq!(PerfNoise::read(&tmp.path().join("floor.json")).unwrap(), f);
    let text = format!(
        "[perf_noise]\nframe = {}\n[perf_noise.terms]\nrender = {}\n",
        f.frame, f.terms["render"]
    );
    std::fs::write(tmp.path().join("floor.toml"), &text).unwrap();
    assert_eq!(
        PerfNoise::read(&tmp.path().join("floor.toml"))
            .unwrap()
            .frame,
        2.0
    );
    let cfg = saccade_core::config::RunConfig::from_toml_str(&text).unwrap();
    assert_eq!(cfg.perf.floor.unwrap().frame, 2.0);
    std::fs::write(
        tmp.path().join("saccade.toml"),
        "perf_noise = 'floor.toml'\nperf_name = 'custom.json'\nperf_noise_k = 4\n",
    )
    .unwrap();
    let cfg =
        saccade_core::config::RunConfig::from_toml_file(&tmp.path().join("saccade.toml")).unwrap();
    assert_eq!(cfg.perf.resolved_floor().unwrap().unwrap().frame, 2.0);
    std::fs::remove_file(dirs[2].join("saccade-perf.json")).unwrap();
    assert!(saccade_core::perf::noise(&dirs, "saccade-perf.json").is_err());
}

#[test]
fn malformed_sidecar_becomes_a_run_error_entry_without_duplicating_image_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let b = tmp.path().join("b");
    let a = tmp.path().join("a");
    write(&b, &capture(10.0, 8.0));
    write(&a, &json!({"bad":true}));
    let image = image::RgbaImage::from_pixel(16, 16, image::Rgba([80, 90, 100, 255]));
    for d in [&b, &a] {
        image.save(d.join("frame.png")).unwrap();
    }
    let r =
        saccade_core::run::run(&b, &a, &tmp.path().join("report"), &Default::default()).unwrap();
    assert_eq!(r.perf_errors.len(), 1);
    assert!(r.perf_diff.is_none());
    assert_eq!(r.totals.error, 1);
    assert_eq!(r.entries.len(), 2);
    assert!(r.is_regression());
    assert_eq!(r.entries[0].bit_identical, Some(true));
    let valid = capture(10.0, 8.0);
    write(&a, &valid);
    let r =
        saccade_core::run::run(&b, &a, &tmp.path().join("report"), &Default::default()).unwrap();
    assert_eq!(r.entries.len(), 1);
    assert!(r.perf_diff.is_some());
    assert!(
        r.combined_verdict
            .unwrap()
            .starts_with("image bit-identical")
    );
}
