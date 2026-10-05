//! Generated colour sequences and known candidate degradations.
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use std::path::{Path, PathBuf};

use image::{Rgb, RgbImage};
use saccade_core::config::RunConfig;
use saccade_core::report::{Metric, Status};

fn save(root: &Path, name: &str, shade: u8, square_x: u32) {
    std::fs::create_dir_all(root).unwrap();
    let mut img = RgbImage::from_pixel(32, 32, Rgb([shade, shade, shade]));
    for y in 8..16 {
        for x in square_x..square_x + 8 {
            img.put_pixel(x, y, Rgb([128, 128, 128]));
        }
    }
    img.save(root.join(name)).unwrap();
}

#[test]
fn moving_square_pairs_by_numeric_index_and_added_flicker_is_positive() {
    let tmp = tempfile::tempdir().unwrap();
    let (b, c) = (tmp.path().join("base"), tmp.path().join("cap"));
    for (i, n) in [2, 10, 11].into_iter().enumerate() {
        save(&b, &format!("frame_{n}.png"), 64, 5 + i as u32);
        save(
            &c,
            &format!("other_{}.png", n + 100),
            if i == 1 { 220 } else { 64 },
            5 + i as u32,
        );
    }
    let cfg = RunConfig::default();
    let identical =
        saccade_core::sequence::run_sequence(&b, &b, &tmp.path().join("same"), "*.png", &cfg)
            .unwrap();
    assert_eq!(identical.temporal_instability, Some(0.0));
    assert!(!identical.is_regression());
    let r = saccade_core::sequence::run_sequence(&b, &c, &tmp.path().join("seq"), "*.png", &cfg)
        .unwrap();
    assert!(r.temporal_instability.unwrap() > 0.0);
    assert_eq!(
        r.frames
            .iter()
            .map(|f| f.baseline_number.unwrap())
            .collect::<Vec<_>>(),
        [2, 10, 11]
    );
    assert_eq!(r.frames[0].capture_number, Some(102));
    assert_eq!(r.worst_frame.as_ref().unwrap().index, 1);
    assert_eq!(r.frames_over_threshold, 1);
    assert!(r.is_regression());
    let html = std::fs::read_to_string(&r.index_html).unwrap();
    assert!(html.contains("<svg") && html.contains("<polyline"));
    assert!(Path::new(&r.frames_report_json).is_file());
    assert!(r.lean().frames.is_empty() && r.lean().mean_flip_curve.is_empty());
}

#[test]
fn sequence_reports_missing_frames_and_refuses_ambiguous_numbers_or_outputs() {
    let tmp = tempfile::tempdir().unwrap();
    let (b, c) = (tmp.path().join("base"), tmp.path().join("cap"));
    save(&b, "f_1.png", 64, 5);
    save(&b, "f_2.png", 64, 6);
    save(&c, "f_101.png", 64, 5);
    let cfg = RunConfig::default();
    let r =
        saccade_core::sequence::run_sequence(&b, &c, &tmp.path().join("seq"), "*", &cfg).unwrap();
    assert_eq!(r.totals.missing, 1);
    assert_eq!(r.frames[1].entry.status, Status::Missing);
    assert!(r.temporal_instability.is_none());
    assert!(r.is_regression());
    assert!(saccade_core::sequence::run_sequence(&b, &c, &b.join("out"), "*", &cfg).is_err());
    save(&b, "duplicate_01.png", 64, 5);
    assert!(
        saccade_core::sequence::run_sequence(&b, &c, &tmp.path().join("duplicate"), "*", &cfg)
            .is_err()
    );
    std::fs::remove_file(b.join("duplicate_01.png")).unwrap();
    std::fs::write(c.join("f_102.png"), b"not an image").unwrap();
    let r =
        saccade_core::sequence::run_sequence(&b, &c, &tmp.path().join("bad"), "*", &cfg).unwrap();
    assert_eq!(r.totals.error, 1);
    assert!(!r.temporal_errors.is_empty());
    assert!(r.temporal_instability.is_none());
}

#[test]
fn rank_orders_known_degradations_handles_ties_and_incomplete_candidates() {
    let tmp = tempfile::tempdir().unwrap();
    let reference = tmp.path().join("ref");
    save(&reference, "a.png", 64, 5);
    save(&reference, "b.png", 64, 6);
    let dirs: Vec<PathBuf> = ["low", "medium", "high"]
        .iter()
        .map(|n| tmp.path().join(n))
        .collect();
    for (dir, shade) in dirs.iter().zip([64, 80, 160]) {
        save(dir, "a.png", shade, 5);
        save(dir, "b.png", shade, 6);
    }
    let cfg = RunConfig::default();
    for metric in [Metric::Mean, Metric::P95, Metric::P99, Metric::Max] {
        let r = saccade_core::rank::run_rank(
            &reference,
            &dirs,
            None,
            metric,
            &tmp.path().join(format!("rank_{metric:?}")),
            &cfg,
        )
        .unwrap();
        assert_eq!(
            r.overall
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>(),
            ["low", "medium", "high"]
        );
        assert_eq!(
            r.overall.iter().map(|c| c.rank).collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3)]
        );
        assert_eq!(r.overall[0].bit_identical, 2);
        assert_eq!(r.common_images, 2);
        assert!(Path::new(&r.overall[0].report_json).is_file());
        assert!(
            std::fs::read_to_string(&r.index_html)
                .unwrap()
                .contains("low/index.html")
        );
        assert!(r.markdown().contains("| Image |"));
    }
    let labels = ["first".into(), "tied".into(), "last".into()];
    let tied_dirs = [dirs[0].clone(), dirs[0].clone(), dirs[2].clone()];
    let tied = saccade_core::rank::run_rank(
        &reference,
        &tied_dirs,
        Some(&labels),
        Metric::Mean,
        &tmp.path().join("ties"),
        &cfg,
    )
    .unwrap();
    assert_eq!(
        tied.overall.iter().map(|c| c.rank).collect::<Vec<_>>(),
        [Some(1), Some(1), Some(3)]
    );
    std::fs::remove_file(dirs[0].join("a.png")).unwrap();
    let incomplete = saccade_core::rank::run_rank(
        &reference,
        &dirs,
        None,
        Metric::Mean,
        &tmp.path().join("missing"),
        &cfg,
    )
    .unwrap();
    assert_eq!(incomplete.common_images, 1);
    assert!(
        incomplete
            .overall
            .iter()
            .all(|c| c.rank.is_none() && !c.complete)
    );
    assert!(incomplete.is_regression());
    assert!(
        saccade_core::rank::run_rank(
            &reference,
            &dirs,
            Some(&["../bad".into(), "b".into(), "c".into()]),
            Metric::Mean,
            &tmp.path().join("unsafe"),
            &cfg
        )
        .is_err()
    );
}

#[test]
fn passing_sequences_carry_exclusions_and_threshold_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let (b, c) = (tmp.path().join("base"), tmp.path().join("cap"));
    for root in [&b, &c] {
        for name in ["frame_1.png", "frame_2.png", "frame_3.png", "other_4.png"] {
            save(root, name, 64, 5);
        }
    }
    let cfg = RunConfig {
        ignore: vec!["frame_3.png".into()],
        ..Default::default()
    };
    let out = tmp.path().join("out");
    let sequence = saccade_core::sequence::run_sequence(&b, &c, &out, "frame_*.png", &cfg).unwrap();
    assert!(!sequence.is_regression());
    let report: saccade_core::Report =
        serde_json::from_slice(&std::fs::read(out.join("saccade-report.v1.json")).unwrap())
            .unwrap();
    let audit = report
        .exclusion_audit
        .as_ref()
        .expect("sequence exclusion audit")
        .evidence
        .as_ref()
        .unwrap();
    assert_eq!(audit.excluded_captures, ["frame_3.png", "other_4.png"]);
    assert_eq!(audit.selection, ["frame_*.png"]);
    assert_eq!(audit.ignore, cfg.ignore);
    assert_eq!(audit.entries.len(), 2);
    assert_eq!(
        audit.entries[0].thresholds[0].headroom,
        Some(cfg.default_threshold)
    );
    assert!(matches!(
        audit.performance,
        saccade_core::evidence::analysis::Capability::Unknown { .. }
    ));
    let json = serde_json::to_value(&sequence).unwrap();
    assert_eq!(
        json["exclusion_audit"]["evidence"]["excluded_captures"],
        serde_json::json!(["frame_3.png", "other_4.png"])
    );
    assert!(
        std::fs::read_to_string(out.join("index.html"))
            .unwrap()
            .contains("frame_3.png")
    );
}
