//! Generated-fixture regressions for field command friction.
#![cfg(feature = "graphics")]
#![allow(clippy::unwrap_used, missing_docs)]
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
};
const BIN: &str = env!("CARGO_BIN_EXE_saccade");
fn capture(root: &Path, label: &str, ms: f64) -> std::path::PathBuf {
    let dir = root.join(label);
    std::fs::create_dir(&dir).unwrap();
    image::RgbImage::from_pixel(8, 8, image::Rgb([80, 90, 100]))
        .save(dir.join("frame.png"))
        .unwrap();
    std::fs::write(dir.join("saccade-meta.json"), "{\"mode\":\"fixed\"}").unwrap();
    std::fs::write(dir.join("saccade-perf.json"), json!({"schema":"saccade-perf.v1","unit":"ms","frame":{"value":ms,"samples":2,"stat":"mean"},"terms":[],"counters":{}}).to_string()).unwrap();
    dir
}
fn ablate(root: &Path, suffix: &str, extra: &[&str]) -> Output {
    Command::new(BIN)
        .args(["experiment", "ablate"])
        .arg(root.join("base"))
        .arg(root.join("arm"))
        .arg("--out")
        .arg(root.join(suffix))
        .args(extra)
        .output()
        .unwrap()
}
#[test]
fn arms_check_out_writes_same_json_and_preserves_verdict() {
    let tmp = tempfile::tempdir().unwrap();
    let a = capture(tmp.path(), "base", 10.);
    let b = capture(tmp.path(), "arm", 9.);
    let out = tmp.path().join("reports/arms.json");
    let run = Command::new(BIN)
        .args(["arms", "check"])
        .arg(&a)
        .arg(&b)
        .args(["--json", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    let stdout: Value = serde_json::from_slice(&run.stdout).unwrap();
    let written: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(saccade_core::report_links::legacy_view(&written), stdout);
    assert_eq!(
        run.status.code().unwrap(),
        stdout["exit_code"].as_i64().unwrap() as i32
    );
}
#[test]
fn ablation_rank_and_ci_render_plain_values() {
    let tmp = tempfile::tempdir().unwrap();
    capture(tmp.path(), "base", 10.);
    capture(tmp.path(), "arm", 9.);
    let run = ablate(tmp.path(), "plain", &["--gpu-clocks-not-applicable"]);
    assert!(run.status.success(), "{run:?}");
    for name in ["ablation.txt", "ablation.md", "index.html"] {
        let text = std::fs::read_to_string(tmp.path().join("plain").join(name)).unwrap();
        assert!(!text.contains("Some("), "{text}");
        assert!(!text.contains("None"), "{text}");
        assert!(text.contains("1"));
    }
    // Multiple repeats render an actual numeric interval too.
    capture(tmp.path(), "base2", 10.2);
    capture(tmp.path(), "arm2", 9.2);
    let run = Command::new(BIN)
        .args(["experiment", "ablate", "--base"])
        .arg(tmp.path().join("base"))
        .arg(tmp.path().join("base2"))
        .arg("--arm")
        .arg(format!("variant={}", tmp.path().join("arm").display()))
        .arg("--arm")
        .arg(format!("variant={}", tmp.path().join("arm2").display()))
        .args(["--gpu-clocks-not-applicable", "--out"])
        .arg(tmp.path().join("multi"))
        .output()
        .unwrap();
    assert!(run.status.success(), "{run:?}");
    let text = std::fs::read_to_string(tmp.path().join("multi/ablation.md")).unwrap();
    assert!(text.contains("[-1.200000, -0.800000]"), "{text}");
}
#[test]
fn single_repeat_table_shows_n_and_unavailable_spread() {
    let tmp = tempfile::tempdir().unwrap();
    capture(tmp.path(), "base", 10.);
    capture(tmp.path(), "arm", 9.);
    let run = ablate(tmp.path(), "single", &["--gpu-clocks-not-applicable"]);
    assert!(run.status.success(), "{run:?}");
    let md = std::fs::read_to_string(tmp.path().join("single/ablation.md")).unwrap();
    assert!(md.contains("| Rank | n |"));
    assert!(
        md.contains("| arm | 1 | 1 | 9.000000 | - | -1.000000 | - |"),
        "{md}"
    );
    assert!(md.contains("IQR is - for n < 2"));
    let txt = std::fs::read_to_string(tmp.path().join("single/ablation.txt")).unwrap();
    assert!(txt.contains("arm\t1\t1\t9.000000\t-\t-1.000000\t-"));
}
#[test]
fn legacy_clock_warning_once_per_command_with_repeated_reads() {
    let tmp = tempfile::tempdir().unwrap();
    // Get the existing compatibility identifier from its reader; the fixture carries no producer names.
    let source = include_str!("fixtures/package/gpu-clock-source.txt");
    let schema = source
        .lines()
        .find(|l| l.contains("gpu-clock.v2\" =>"))
        .unwrap()
        .split('"')
        .nth(1)
        .unwrap();
    for (label, ms) in [("base", 10.), ("base2", 10.2), ("arm", 9.), ("arm2", 9.2)] {
        let dir = capture(tmp.path(), label, ms);
        std::fs::write(
            dir.join("gpu_clock.json"),
            json!({"schema":schema,"device":{"uuid":["fixture-device"]},"sample_windows":[]})
                .to_string(),
        )
        .unwrap();
    }
    for suffix in ["first", "second"] {
        let run = Command::new(BIN)
            .args(["experiment", "ablate", "--base"])
            .arg(tmp.path().join("base"))
            .arg(tmp.path().join("base2"))
            .arg("--arm")
            .arg(format!("variant={}", tmp.path().join("arm").display()))
            .arg("--arm")
            .arg(format!("variant={}", tmp.path().join("arm2").display()))
            .arg("--out")
            .arg(tmp.path().join(suffix))
            .output()
            .unwrap();
        assert!(run.status.success(), "{run:?}");
        assert_eq!(
            String::from_utf8_lossy(&run.stderr)
                .matches("warning: legacy GPU clock schema is deprecated")
                .count(),
            1,
            "{run:?}"
        );
        assert!(
            !String::from_utf8_lossy(&run.stdout)
                .contains("warning: legacy GPU clock schema is deprecated")
        );
    }
}
#[test]
fn image_free_arm_is_excluded_and_strict_exit_keeps_table() {
    let tmp = tempfile::tempdir().unwrap();
    capture(tmp.path(), "base", 10.);
    let arm = capture(tmp.path(), "arm", 9.);
    let empty = tmp.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    std::fs::write(empty.join("capture.json"), "{}").unwrap();
    for (suffix, strict, code) in [("partial", false, 0), ("strict", true, 1)] {
        let mut cmd = Command::new(BIN);
        cmd.args(["experiment", "ablate"])
            .arg(tmp.path().join("base"))
            .arg(&arm)
            .arg(&empty)
            .args(["--gpu-clocks-not-applicable", "--out"])
            .arg(tmp.path().join(suffix));
        if strict {
            cmd.arg("--require-all-arms");
        }
        let run = cmd.output().unwrap();
        assert_eq!(run.status.code(), Some(code), "{run:?}");
        let md = std::fs::read_to_string(tmp.path().join(suffix).join("ablation.md")).unwrap();
        assert!(md.contains("excluded: no images"));
        assert!(md.contains("| arm |"));
        let v: Value = serde_json::from_slice(
            &std::fs::read(tmp.path().join(suffix).join("saccade-ablate.v1.json")).unwrap(),
        )
        .unwrap();
        let excluded = v["arms"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["label"] == "empty")
            .unwrap();
        assert_eq!(excluded["flag"], "EXCLUDED");
        assert_eq!(excluded["timing_rank"], Value::Null);
        assert_eq!(excluded["no_effect"], false);
    }
}
#[test]
#[cfg(unix)]
fn noise_discovery_reports_symlinks_empty_and_over_budget() {
    use saccade_core::evidence_quality::repeat_noise::build_runs;
    let tmp = tempfile::tempdir().unwrap();
    let dirs = [tmp.path().join("one"), tmp.path().join("two")];
    let target = tmp.path().join("pixels.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([80, 90, 100]))
        .save(&target)
        .unwrap();
    for dir in &dirs {
        std::fs::create_dir(dir).unwrap();
        std::os::unix::fs::symlink(&target, dir.join("linked.png")).unwrap();
    }
    let run = Command::new(BIN)
        .args(["noise", "build"])
        .args(&dirs)
        .arg("--out")
        .arg(tmp.path().join("noise.json"))
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(2), "{run:?}");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("noise skipped symlinks: 1"), "{run:?}");
    assert!(stderr.contains("linked.png"));
    let error = build_runs(&dirs, 8).unwrap_err().to_string();
    assert!(error.contains("capture set empty"));
    assert!(error.contains("skipped symlinks: 1"));
    assert!(error.contains("linked.png"));
    assert!(!error.contains("over budget"));
    for dir in &dirs {
        std::fs::copy(&target, dir.join("frame.png")).unwrap();
    }
    let report = build_runs(&dirs, 8).unwrap();
    assert_eq!(report.entries.len(), 1);
    assert!(
        report
            .limits
            .iter()
            .any(|l| l.contains("skipped symlinks: 1"))
    );
    for i in 0..1024 {
        std::fs::write(dirs[0].join(format!("extra-{i}.png")), b"fixture").unwrap();
    }
    let error = build_runs(&dirs, 8).unwrap_err().to_string();
    assert!(error.contains("over budget"));
    assert!(error.contains("1025 images (maximum 1024)"));
    assert!(!error.contains("capture set empty"));
}
#[test]
fn absolute_mask_error_states_rule_and_preserves_refusal() {
    use saccade_core::evidence_quality::effect::{Selection, select};
    let tmp = tempfile::tempdir().unwrap();
    let mask = tmp.path().join("mask.png");
    image::GrayImage::from_pixel(8, 8, image::Luma([255]))
        .save(&mask)
        .unwrap();
    for path in [mask.to_str().unwrap(), "../mask.png"] {
        let error = select(
            &Selection::Mask { image: path.into() },
            &mask,
            tmp.path(),
            (8, 8),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("masks must be relative to the config file"));
        assert!(error.contains("inside its directory"));
    }
    assert!(
        select(
            &Selection::Mask {
                image: "mask.png".into()
            },
            &mask,
            tmp.path(),
            (8, 8)
        )
        .unwrap()
        .iter()
        .all(|p| *p)
    );
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        let escaped = outside.path().join("mask.png");
        std::fs::copy(&mask, &escaped).unwrap();
        std::os::unix::fs::symlink(&escaped, tmp.path().join("escape.png")).unwrap();
        assert!(
            select(
                &Selection::Mask {
                    image: "escape.png".into()
                },
                &mask,
                tmp.path(),
                (8, 8)
            )
            .unwrap_err()
            .to_string()
            .contains("inside its directory")
        );
    }
}
