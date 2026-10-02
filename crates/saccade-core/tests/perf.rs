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
    for opts in [
        PerfOptions {
            resolution_ms: Some(0.0),
            ..Default::default()
        },
        PerfOptions {
            resolution_ms: Some(f64::NAN),
            ..Default::default()
        },
        PerfOptions {
            resolution_ticks: Some(0),
            ..Default::default()
        },
        PerfOptions {
            min_delta_ms: Some(-1.0),
            ..Default::default()
        },
        PerfOptions {
            min_delta_pct: Some(f64::INFINITY),
            ..Default::default()
        },
    ] {
        assert!(opts.validate().is_err());
    }
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
        resolution_ms: Some(0.0001),
        terms: [("render".into(), 0.2), ("render/detail".into(), 0.5)].into(),
        ..Default::default()
    };
    let d = PerfDiff::between(&parse(&b), &parse(&a), Some(&floor), 3.0);
    assert_eq!(d.unattributed_before, None);
    assert_eq!(d.unattributed_after, None);
    assert_eq!(d.warnings.len(), 0);
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
    assert_eq!(d.flags(true), (false, false));
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
            ..Default::default()
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
        None
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
    assert_eq!(
        warnings
            .iter()
            .filter(|w| w.contains("not comparable across all repeats"))
            .count(),
        2
    );
    std::fs::write(
        tmp.path().join("floor.json"),
        json!({"perf_noise":f}).to_string(),
    )
    .unwrap();
    assert_eq!(PerfNoise::read(&tmp.path().join("floor.json")).unwrap(), f);
    std::fs::write(
        tmp.path().join("full-floor.toml"),
        toml::to_string(&json!({"perf_noise": f})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        PerfNoise::read(&tmp.path().join("full-floor.toml")).unwrap(),
        f
    );
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
fn quantised_zero_spread_and_one_tick_do_not_make_performance_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    let quantum = 0.001024;
    let b = capture(16.0 * quantum, 10.0 * quantum);
    let dirs: Vec<_> = (0..3).map(|i| tmp.path().join(i.to_string())).collect();
    for (i, dir) in dirs.iter().enumerate() {
        let mut v = b.clone();
        // Decimal/floating jitter below the estimator tolerance is not a tick.
        v["terms"][2]["value"] = json!(5.0 * quantum + i as f64 * 1e-9);
        write(dir, &v);
    }
    let opts = PerfOptions {
        min_delta_ms: Some(0.0),
        min_delta_pct: Some(0.0),
        ..Default::default()
    };
    let (floor, _) = saccade_core::perf::noise_with_options(&dirs, &opts).unwrap();
    let floor = floor.unwrap();
    assert!((floor.resolution_ms.unwrap() - quantum).abs() < 1e-8);
    assert_eq!(floor.terms["render"], 0.0);
    let a = capture(16.0 * quantum, 11.0 * quantum);
    let d = PerfDiff::between(&parse(&b), &parse(&a), Some(&floor), 3.0);
    assert_eq!(d.flags(true), (false, false));
    let render = d.terms.iter().find(|t| t.id == "render").unwrap();
    assert_eq!(render.change.beyond_noise, Some(false));
    assert!((render.change.noise_threshold.unwrap() - 2.0 * quantum).abs() < 1e-8);

    // Unmatched terms carry durations, not paired changes, even when large.
    let mut unpaired = b.clone();
    unpaired["terms"][0]["id"] = json!("other-render");
    unpaired["terms"][0]["value"] = json!(9.0 * quantum);
    unpaired["terms"][2]["parent"] = json!("other-render");
    unpaired["terms"][2]["id"] = json!("other-detail");
    unpaired["counters"] = json!({});
    let d = PerfDiff::between(&parse(&b), &parse(&unpaired), Some(&floor), 3.0);
    assert_eq!(d.flags(true), (false, false));
    assert!(d.verdict().contains("terms differ"));
    assert!(d.verdict().contains("disappeared"));
    assert!(d.summary(1).contains("above minimum delta"));
    assert_eq!(d.not_comparable(1)[0].change.status, "disappeared");
}

#[test]
fn meaningful_term_or_frame_changes_above_the_floor_are_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let quantum = 0.001024;
    let b = capture(1001.0 * quantum, 500.0 * quantum);
    let dirs: Vec<_> = (0..2).map(|i| tmp.path().join(i.to_string())).collect();
    for dir in &dirs {
        write(dir, &b);
    }
    let floor = saccade_core::perf::noise(&dirs, "saccade-perf.json")
        .unwrap()
        .0
        .unwrap();
    assert!((floor.resolution_ms.unwrap() - quantum).abs() < 1e-10);
    assert_eq!(floor.min_delta_ms, 0.05);
    assert_eq!(floor.min_delta_pct, 0.5);
    let a = capture(1001.0 * quantum, 600.0 * quantum);
    let d = PerfDiff::between(&parse(&b), &parse(&a), Some(&floor), 3.0);
    assert_eq!(d.frame.beyond_noise, Some(false));
    assert_eq!(d.flags(true), (false, false));
    assert_eq!(d.top(1, true)[0].change.beyond_noise, Some(true));

    let mut a = b.clone();
    a["frame"]["value"] = json!(b["frame"]["value"].as_f64().unwrap() + 0.2);
    let d = PerfDiff::between(&parse(&b), &parse(&a), Some(&floor), 3.0);
    assert_eq!(d.frame.beyond_noise, Some(true));
    assert!(d.top(5, true).is_empty());
    assert_eq!(d.flags(true), (false, false));

    // The same term change is below 0.5% of a 100 ms baseline frame.
    let b = capture(100.0, 0.512);
    let a = capture(100.2, 0.612);
    let d = PerfDiff::between(&parse(&b), &parse(&a), Some(&floor), 3.0);
    assert_eq!(d.minimum_delta_ms, 0.5);
    assert_eq!(d.frame.beyond_noise, Some(false));
    assert_eq!(d.flags(true), (false, false));
    let opts = PerfOptions {
        min_delta_pct: Some(0.0),
        ..Default::default()
    };
    let d = PerfDiff::between_with_options(&parse(&b), &parse(&a), Some(&floor), &opts);
    assert_eq!(d.flags(true), (false, false));
    let opts = PerfOptions {
        resolution_ms: Some(1.0),
        ..Default::default()
    };
    let d = PerfDiff::between_with_options(&parse(&b), &parse(&a), Some(&floor), &opts);
    assert_eq!(d.flags(true), (false, false));
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

fn qualified(frame: f64, terms: Value) -> CapturePerf {
    parse(
        &json!({"schema":"saccade-perf.v2", "kind":"measurement", "unit":"ms",
        "frame":{"value":frame,"samples":8,"stat":"mean"}, "terms":terms, "counters":{},
        "context":serde_json::from_str::<Value>(include_str!("fixtures/perf/context.json")).unwrap()}),
    )
}
fn known_floor() -> PerfNoise {
    PerfNoise {
        frame: 0.0,
        terms: [("main".into(), 0.0)].into(),
        resolution_ms: Some(0.0001),
        comparability: saccade_core::perf::Comparability::Qualified,
        timer: Some("fixture-timer".into()),
        context_identity: unchanged().comparison_identity(),
        ..Default::default()
    }
}
fn unchanged() -> CapturePerf {
    qualified(10.0, json!([{"id":"main","kind":"pass","value":10.0}]))
}

#[test]
fn unknown_noise_timer_and_historical_qualification_remain_unknown() {
    use saccade_core::perf::{Comparability, FrameChange};
    let b = unchanged();
    let missing_noise = PerfDiff::between(&b, &b, None, 3.0);
    assert_eq!(missing_noise.frame.beyond_noise, None);
    assert_eq!(missing_noise.frame_change, FrameChange::Unknown);
    assert_eq!(missing_noise.flags(true), (false, false));
    let legacy = parse(&capture(10.0, 8.0));
    let d = PerfDiff::between(&legacy, &legacy, Some(&known_floor()), 3.0);
    assert_eq!(d.comparability, Comparability::Unknown);
    assert_eq!(d.flags(true), (false, false));
    let mut no_timer = b.clone();
    no_timer.context.as_mut().unwrap().quantum_ms = None;
    let floor = PerfNoise {
        resolution_ms: None,
        ..known_floor()
    };
    let d = PerfDiff::between(&no_timer, &no_timer, Some(&floor), 3.0);
    assert_eq!(d.frame.beyond_noise, None);
    assert_eq!(d.materiality.floor_ms, None);
    assert_eq!(d.flags(true), (false, false));
    let mut legacy_floor = known_floor();
    legacy_floor.comparability = Comparability::Unknown;
    assert_eq!(
        PerfDiff::between(&b, &b, Some(&legacy_floor), 3.0).flags(true),
        (false, false)
    );
}

#[test]
fn failed_or_missing_qualification_checks_cannot_qualify_a_pair() {
    use saccade_core::perf::Comparability;
    let mut b = unchanged();
    for (check, expected) in [
        (Some(false), Comparability::Rejected),
        (None, Comparability::Unknown),
    ] {
        b.context
            .as_mut()
            .unwrap()
            .qualification
            .checks
            .insert("clock_qualified".into(), check);
        let d = PerfDiff::between(&b, &b, Some(&known_floor()), 3.0);
        assert_eq!(d.comparability, expected);
        assert_eq!(d.flags(true), (false, false));
    }
    let b = unchanged();
    let mut a = b.clone();
    a.context
        .as_mut()
        .unwrap()
        .hardware
        .insert("driver".into(), "different".into());
    assert_eq!(
        PerfDiff::between(&b, &a, Some(&known_floor()), 3.0).comparability,
        Comparability::Rejected
    );
}

#[test]
fn a_pass_change_keeps_the_frame_within_noise_and_names_the_scope() {
    use saccade_core::perf::FrameChange;
    let b = qualified(
        10.0,
        json!([{"id":"main","kind":"pass","value":8.0},{"id":"idle","kind":"gap","value":2.0}]),
    );
    let a = qualified(
        10.0,
        json!([{"id":"main","kind":"pass","value":7.0},{"id":"idle","kind":"gap","value":3.0}]),
    );
    let mut floor = known_floor();
    floor.terms.insert("idle".into(), 0.0);
    let d = PerfDiff::between(&b, &a, Some(&floor), 3.0);
    assert_eq!(d.frame_change, FrameChange::WithinMeasuredNoise);
    assert_eq!(d.frame.delta, Some(0.0));
    assert_eq!(d.flags(true), (false, true));
    assert!(d.top(2, true).iter().any(|t| t.id == "main"));
    let faster = qualified(9.0, json!([{"id":"main","kind":"pass","value":9.0}]));
    assert_eq!(
        PerfDiff::between(&unchanged(), &faster, Some(&known_floor()), 3.0).frame_change,
        FrameChange::Faster
    );
}

fn renamed_terms(values: &[f64]) -> (CapturePerf, CapturePerf) {
    let rest = 10.0 - values.iter().sum::<f64>();
    let side = |prefix: &str| {
        let mut terms = vec![json!({"id":"main","kind":"pass","value":rest})];
        terms.extend(
            values
                .iter()
                .enumerate()
                .map(|(i, v)| json!({"id":format!("{prefix}{i}"),"kind":"pass","value":v})),
        );
        qualified(10.0, json!(terms))
    };
    (side("old"), side("new"))
}

#[test]
fn materiality_just_below_equal_and_above_the_floor_gate_no_effect() {
    for (bound, allowed) in [(0.049, true), (0.05, false), (0.051, false)] {
        let (b, a) = renamed_terms(&[bound]);
        let d = PerfDiff::between(&b, &a, Some(&known_floor()), 3.0);
        assert_eq!(d.materiality.floor_ms, Some(0.05));
        assert!((d.materiality.unexplained_upper_bound_ms.unwrap() - bound).abs() < 1e-12);
        assert_eq!(d.flags(true), (allowed, false));
    }
}

#[test]
fn disjoint_small_terms_are_material_in_aggregate() {
    let (b, a) = renamed_terms(&[0.03, 0.03]);
    let d = PerfDiff::between(&b, &a, Some(&known_floor()), 3.0);
    assert!((d.materiality.unexplained_upper_bound_ms.unwrap() - 0.06).abs() < 1e-12);
    assert_eq!(d.flags(true), (false, false));
}

#[test]
fn nested_unexplained_scopes_are_counted_once() {
    let side = |prefix: &str| {
        qualified(
            10.0,
            json!([
        {"id":"main","kind":"pass","value":10.0},
        {"id":prefix,"kind":"scope","parent":"main","value":0.04},
        {"id":format!("{prefix}/child"),"kind":"scope","parent":prefix,"value":0.03}]),
        )
    };
    let d = PerfDiff::between(&side("old"), &side("new"), Some(&known_floor()), 3.0);
    assert_eq!(d.materiality.unexplained_upper_bound_ms, Some(0.04));
    assert_eq!(d.unattributed_before, Some(0.0));
    assert_eq!(d.flags(true), (true, false));
}

#[test]
fn missing_bounds_and_material_frame_remainders_block_no_effect() {
    let b = unchanged();
    let mut a = b.clone();
    a.context
        .as_mut()
        .unwrap()
        .unavailable_terms
        .push(saccade_core::perf::UnavailableTerm {
            id: "missing".into(),
            parent: Some("main".into()),
            upper_bound_ms: None,
        });
    let d = PerfDiff::between(&b, &a, Some(&known_floor()), 3.0);
    assert_eq!(d.materiality.unexplained_upper_bound_ms, None);
    assert_eq!(d.flags(true), (false, false));
    for bound in [0.049, 0.05, 0.051] {
        let mut a = b.clone();
        a.context.as_mut().unwrap().unresolved_remainder_ms = Some(bound);
        let d = PerfDiff::between(&b, &a, Some(&known_floor()), 3.0);
        assert_eq!(d.flags(true), (bound < 0.05, false));
    }
}

#[test]
fn independent_statistics_never_become_an_additive_frame_measurement() {
    use saccade_core::perf::Attribution;
    let mut b = unchanged();
    b.terms[0].value = 12.0;
    let c = b.context.as_mut().unwrap();
    c.aggregation = "independent_statistics".into();
    c.unresolved_remainder_ms = None;
    let d = PerfDiff::between(&b, &b, Some(&known_floor()), 3.0);
    assert_eq!(d.unattributed_before, None);
    assert_eq!(d.attribution, Attribution::Unavailable);
    assert_eq!(d.flags(true), (false, false));
}

#[test]
fn wrong_noise_kind_is_targeted_and_legacy_performance_toml_stays_readable() {
    let tmp = tempfile::tempdir().unwrap();
    for text in [
        r#"{"schema":"saccade-noise.v1","kind":"image_noise","unit":"FLIP"}"#,
        "[[override]]\nglob = '*.png'\nthreshold = 0.01\n",
    ] {
        let file = tmp.path().join("noise");
        std::fs::write(&file, text).unwrap();
        let error = PerfNoise::read(&file).unwrap_err().to_string();
        assert!(error.contains("wrong_noise_kind"));
        assert!(error.contains("--kind performance"));
    }
    let file = tmp.path().join("noise");
    std::fs::write(&file, "[perf_noise]\nframe = 0.0\n").unwrap();
    assert_eq!(
        PerfNoise::read(&file).unwrap().comparability,
        saccade_core::perf::Comparability::Unknown
    );
    let record = json!({"schema":"saccade-perf.v2","kind":"performance_noise","unit":"ms",
        "perf_noise":known_floor(),"comparability":"qualified","sources":[],"reasons":[]});
    let error = saccade_core::config::RunConfig::from_toml_str(&record.to_string())
        .unwrap_err()
        .to_string();
    assert!(error.contains("wrong_noise_kind") && error.contains("--kind image"));
}
