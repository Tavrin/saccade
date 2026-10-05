//! Mathematical reference values, fixed-plan boundaries and constructed coverage.
#![allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]
use saccade_core::{
    evidence::canonical::{self, Digest},
    paired_stats::*,
};
fn samples(before: &[f64], after: &[f64], seed: u64) -> Samples {
    let plan = Plan {
        schema: "saccade-perf-plan.v1".into(),
        independent_unit: "run".into(),
        stopping_rule: "fixed_count".into(),
        pairs: (0..before.len())
            .map(|i| Assignment {
                id: format!("pair-{i}"),
                order: if i % 2 == 0 { "ab" } else { "ba" }.into(),
            })
            .collect(),
        confidence: 0.95,
        resamples: 2048,
        seed,
    };
    let observations = before
        .iter()
        .zip(after)
        .enumerate()
        .map(|(i, (&before_ms, &after_ms))| Observation {
            pair_id: plan.pairs[i].id.clone(),
            order: plan.pairs[i].order.clone(),
            before_ms,
            after_ms,
        })
        .collect();
    Samples {
        schema: "saccade-perf-pairs.v1".into(),
        before_perf_sha256: Digest::of_bytes(b"before"),
        after_perf_sha256: Digest::of_bytes(b"after"),
        plan_sha256: canonical::digest(&plan).unwrap(),
        plan,
        observations,
    }
}
#[test]
fn walsh_reference_constants_ties_outlier_and_exhaustive_oracle() {
    for (input, expected) in [
        (vec![1., 2., 3., 4.], 2.5),
        (vec![1., 1., 3.], 1.5),
        (vec![0., 0., 0., 0., 100.], 0.),
        (vec![-2.; 6], -2.),
    ] {
        assert_eq!(hodges_lehmann(&input).unwrap(), expected);
    }
    let values = [-3., 0.5, 4., 1., 17., -1.];
    let mut oracle = Vec::new();
    for i in 0..values.len() {
        for j in i..values.len() {
            oracle.push((values[i] + values[j]) / 2.);
        }
    }
    oracle.sort_unstable_by(f64::total_cmp);
    assert_eq!(hodges_lehmann(&values).unwrap(), oracle[oracle.len() / 2]);
    assert_eq!(quantile(&[1., 2., 3., 4.], 0.25).unwrap(), 1.75);
}
#[test]
fn paired_bootstrap_matches_independent_numpy_reference_and_log_ratios() {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/wave3/paired-reference.json")).unwrap();
    let b: Vec<_> = v["before_ms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let a: Vec<_> = v["after_ms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let data = samples(&b, &a, 42);
    let effect = estimate(&data).unwrap();
    assert_eq!(effect, estimate(&data).unwrap());
    assert!((effect.delta_ms - v["delta_ms"].as_f64().unwrap()).abs() < 1e-12);
    assert!((effect.delta_pct - v["delta_pct"].as_f64().unwrap()).abs() < 1e-10);
    for i in 0..2 {
        assert!((effect.interval_ms[i] - v["interval_ms"][i].as_f64().unwrap()).abs() < 1e-12);
        assert!((effect.interval_pct[i] - v["interval_pct"][i].as_f64().unwrap()).abs() < 1e-10);
    }
    let a: Vec<_> = b.iter().map(|v| v * 1.25).collect();
    let e = estimate(&samples(&b, &a, 17)).unwrap();
    assert!((e.delta_pct - 25.).abs() < 1e-10);
    assert!(e.interval_pct.iter().all(|v| (v - 25.).abs() < 1e-10));
}
#[test]
fn fixed_plan_rejects_stopping_frames_duplicates_order_and_stale_pins() {
    let b = [10.; 6];
    let a = [11.; 6];
    let good = samples(&b, &a, 7);
    let e = estimate(&good).unwrap();
    assert_eq!(e.interval_ms, [1., 1.]);
    assert!(e.degenerate);
    let mut x = good.clone();
    x.observations.pop();
    assert!(estimate(&x).is_err());
    let mut x = good.clone();
    x.plan.stopping_rule = "stop_when_significant".into();
    x.plan_sha256 = canonical::digest(&x.plan).unwrap();
    assert!(estimate(&x).is_err());
    let mut x = good.clone();
    x.plan.independent_unit = "frame".into();
    x.plan_sha256 = canonical::digest(&x.plan).unwrap();
    assert!(estimate(&x).is_err());
    let mut x = good.clone();
    x.observations[1].pair_id = x.observations[0].pair_id.clone();
    assert!(estimate(&x).is_err());
    let mut x = good.clone();
    x.observations.swap(0, 1);
    assert!(estimate(&x).is_err());
    let mut x = good.clone();
    x.plan.seed += 1;
    assert!(estimate(&x).is_err());
    let mut x = good;
    x.observations[0].before_ms = 0.;
    assert!(estimate(&x).is_err());
}
#[test]
fn seeded_symmetric_null_and_shift_have_constructed_coverage() {
    let mut generator = Generator::new(734);
    let mut covered = 0;
    let mut alternatives = 0;
    for campaign in 0..48 {
        let b = vec![100.; 24];
        let noise: Vec<_> = (0..24)
            .map(|_| {
                (0..12)
                    .map(|_| (generator.index(1 << 20).unwrap() as f64 + 0.5) / (1 << 20) as f64)
                    .sum::<f64>()
                    - 6.
            })
            .collect();
        let a: Vec<_> = noise.iter().map(|n| 100. + n).collect();
        let mut data = samples(&b, &a, campaign);
        data.plan.resamples = 1024;
        data.plan_sha256 = canonical::digest(&data.plan).unwrap();
        let e = estimate(&data).unwrap();
        covered += usize::from(e.interval_ms[0] <= 0. && e.interval_ms[1] >= 0.);
        for row in &mut data.observations {
            row.after_ms += 2.;
        }
        let e = estimate(&data).unwrap();
        alternatives += usize::from(e.interval_ms[0] > 0.);
    }
    // A broad deterministic sanity bound, not universal 95% coverage qualification.
    assert!(
        (39..=48).contains(&covered),
        "constructed coverage {covered}/48"
    );
    assert!(alternatives >= 46);
}
#[cfg(feature = "graphics")]
#[test]
fn performance_verdict_requires_qualified_resolved_nondegenerate_interval() {
    use saccade_core::perf::{CapturePerf, Comparability, FrameChange, PerfDiff};
    let parse = |value| {
        CapturePerf::parse(&serde_json::json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":value,"samples":6,"stat":"p50"},"terms":[],"counters":{}}).to_string(),"synthetic").unwrap()
    };
    let mut d = PerfDiff::between(&parse(10.), &parse(9.), None, 3.);
    d.frame.noise_threshold = Some(0.1);
    let b = [10.; 6];
    let e = estimate(&samples(&b, &[8.8, 8.9, 9., 9.1, 9.2, 9.], 11)).unwrap();
    d.attach_robust(e.clone());
    assert_eq!(d.frame_change, FrameChange::Unknown);
    d.comparability = Comparability::Qualified;
    d.noise_comparability = Comparability::Qualified;
    d.attach_robust(e);
    assert_eq!(d.frame_change, FrameChange::Faster);
    assert!(d.verdict().contains("paired HL"));
    d.attach_robust(estimate(&samples(&b, &[9., 11., 9., 11., 9., 11.], 12)).unwrap());
    assert_eq!(d.frame_change, FrameChange::Unknown);
    assert_eq!(d.frame.beyond_noise, None);
    d.attach_robust(estimate(&samples(&b, &[9.; 6], 12)).unwrap());
    assert_eq!(d.frame_change, FrameChange::Unknown);
}
#[cfg(feature = "graphics")]
#[test]
fn paired_sidecar_is_integrated_and_stale_or_partial_series_rejects_qualification() {
    use saccade_core::perf::{self, Comparability, PerfOptions};
    let t = tempfile::tempdir().unwrap();
    let before = t.path().join("b");
    let after = t.path().join("a");
    std::fs::create_dir(&before).unwrap();
    std::fs::create_dir(&after).unwrap();
    let b=serde_json::json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":10.,"samples":6,"stat":"p50"},"terms":[],"counters":{}}).to_string();
    let a = b.replace("10.0", "9.0");
    std::fs::write(before.join("saccade-perf.json"), &b).unwrap();
    std::fs::write(after.join("saccade-perf.json"), &a).unwrap();
    let mut s = samples(&[10.; 6], &[8.8, 8.9, 9., 9.1, 9.2, 9.], 11);
    s.before_perf_sha256 = Digest::of_bytes(b.as_bytes());
    s.after_perf_sha256 = Digest::of_bytes(a.as_bytes());
    let path = after.join("saccade-perf-pairs.json");
    std::fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();
    let opts = PerfOptions {
        gpu_clocks_not_applicable: true,
        ..Default::default()
    };
    let (d, errors) = perf::pair(&before, &after, &opts).unwrap();
    assert!(errors.is_empty());
    assert!(d.unwrap().robust_effect.is_some());
    s.observations.pop();
    std::fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();
    let d = perf::pair(&before, &after, &opts).unwrap().0.unwrap();
    assert_eq!(d.comparability, Comparability::Rejected);
    assert!(
        d.qualification_reasons
            .iter()
            .any(|s| s.contains("fixed-plan"))
    );
    s.observations.push(Observation {
        pair_id: "pair-5".into(),
        order: "ba".into(),
        before_ms: 10.,
        after_ms: 9.,
    });
    s.after_perf_sha256 = Digest::of_bytes(b"stale");
    std::fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();
    assert_eq!(
        perf::pair(&before, &after, &opts)
            .unwrap()
            .0
            .unwrap()
            .comparability,
        Comparability::Rejected
    );
    #[cfg(unix)]
    {
        let outside = t.path().join("outside.json");
        std::fs::write(&outside, serde_json::to_vec(&s).unwrap()).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&outside, &path).unwrap();
        let d = perf::pair(&before, &after, &opts).unwrap().0.unwrap();
        assert_eq!(d.comparability, Comparability::Rejected);
        assert!(
            d.qualification_reasons
                .iter()
                .any(|s| s.contains("escapes"))
        );
    }
}

#[test]
fn w3_f12_quantile_extreme_finite() {
    assert_eq!(quantile(&[-f64::MAX, f64::MAX], 0.5).unwrap(), 0.);
    assert_eq!(quantile(&[-f64::MAX, f64::MAX], 0.).unwrap(), -f64::MAX);
    assert_eq!(quantile(&[-f64::MAX, f64::MAX], 1.).unwrap(), f64::MAX);
    assert!(
        quantile(&[f64::MAX / 2., f64::MAX], 0.75)
            .unwrap()
            .is_finite()
    );
}
#[test]
fn w3_f13_empty_generator_does_not_panic() {
    assert!(
        std::panic::catch_unwind(|| Generator::new(1).index(0))
            .unwrap()
            .is_err()
    );
    let mut a = Generator::new(1);
    let mut b = Generator::new(1);
    assert!(a.index(0).is_err());
    for length in [1, 2, 17, usize::MAX] {
        let i = a.index(length).unwrap();
        assert!(i < length);
        assert_eq!(i, b.index(length).unwrap());
    }
}
#[cfg(all(unix, feature = "graphics"))]
#[test]
fn w3_f02_dangling_pairs_rejected() {
    use saccade_core::perf::{self, Comparability, PerfOptions};
    let t = tempfile::tempdir().unwrap();
    let before = t.path().join("b");
    let after = t.path().join("a");
    std::fs::create_dir(&before).unwrap();
    std::fs::create_dir(&after).unwrap();
    let b = serde_json::json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":10.,"samples":6,"stat":"p50"},"terms":[],"counters":{}});
    for dir in [&before, &after] {
        std::fs::write(dir.join("saccade-perf.json"), b.to_string()).unwrap();
    }
    let opts = PerfOptions {
        gpu_clocks_not_applicable: true,
        ..Default::default()
    };
    let historical = perf::pair(&before, &after, &opts).unwrap().0.unwrap();
    std::os::unix::fs::symlink(
        after.join("missing.json"),
        after.join("saccade-perf-pairs.json"),
    )
    .unwrap();
    let d = perf::pair(&before, &after, &opts).unwrap().0.unwrap();
    assert_eq!(d.comparability, Comparability::Rejected);
    assert!(d.qualification_reasons.iter().any(|s| s.contains("paired")));
    assert_ne!(d.qualification_reasons, historical.qualification_reasons);
}
