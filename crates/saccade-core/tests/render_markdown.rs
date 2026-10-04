//! Tests for the Markdown summary.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use saccade_core::render::{MarkdownOptions, render_markdown};
use saccade_core::report::{Entry, EntryPaths, Metric, Report, Status};

fn sample() -> Report {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/sample-report/saccade-report.v1.json");
    let text = std::fs::read_to_string(path).expect("sample report readable");
    serde_json::from_str(&text).expect("sample report parses")
}

fn entry(name: String, status: Status, value: Option<f64>) -> Entry {
    Entry {
        pass_with_local_change: false,
        buffer: None,
        name,
        status,
        metric_used: Metric::Mean,
        threshold: 0.01,
        value,
        metrics: None,
        properties: None,
        paths: EntryPaths::default(),
        error: None,
        regions: Vec::new(),
        masked_fraction: None,
        bit_identical: None,
        hdr: None,
        meta_diff: Vec::new(),
        meta_ignored_diff: Vec::new(),
        file_bytes_identical: None,
        capture_validity: Default::default(),
        capture_provenance: Default::default(),
        meta_declared_unchanged: Vec::new(),
        baseline_properties: None,
        warnings: Vec::new(),
        baseline_sha256: None,
        capture_sha256: None,
        hotspots: Vec::new(),
        changed_pixel_runs: Vec::new(),
        object_attribution: Vec::new(),
        diagnostics: None,
    }
}

fn synthetic(fail: usize, pass: usize) -> Report {
    let mut r = sample();
    r.entries = (0..fail)
        .map(|i| {
            entry(
                format!("fail/{i:03}.png"),
                Status::Fail,
                Some(0.02 + i as f64 * 1e-4),
            )
        })
        .chain((0..pass).map(|i| entry(format!("pass/{i:03}.png"), Status::Pass, Some(0.001))))
        .collect();
    r.totals.total = fail + pass;
    r.totals.fail = fail;
    r.totals.pass = pass;
    r.totals.new = 0;
    r.totals.missing = 0;
    r.totals.error = 0;
    r
}

#[test]
fn sample_summary_has_marker_heading_rows_and_footer() {
    let opts = MarkdownOptions {
        artifact_url: Some("https://example.invalid/run/1".into()),
        max_bytes: None,
        comment_key: None,
    };
    let md = render_markdown(&sample(), &opts);
    let lines: Vec<&str> = md.lines().collect();
    assert_eq!(lines[0], "<!-- saccade-summary -->");
    assert_eq!(
        lines[1],
        "### saccade: ❌ 3 failed · 1 errored · 1 missing · 1 new · 2 passed"
    );
    // Worst fail first, values to 4 significant digits, zeros kept.
    assert!(md.contains("| ❌ fail | `gi/cornell-box.png` | mean | 0.4412 | 0.01000 |"));
    assert!(md.contains("| ❌ fail | `terrain/heightfield.png` | p95 | 0.2113 | 0.1500 |"));
    assert!(md.contains("| 🆕 new | `ui/hud minimap #2.png` | mean | — | — |"));
    let cornell = md.find("cornell-box").expect("row");
    let shadow = md.find("shadow-shift").expect("row");
    let terrain = md.find("heightfield").expect("row");
    assert!(
        cornell < terrain && terrain < shadow,
        "fails sorted by value desc"
    );
    assert!(md.contains("<details><summary>2 passed</summary>"));
    assert!(md.contains("[Full report with heatmaps](https://example.invalid/run/1)"));
    assert!(md.trim_end().ends_with("<sub>saccade v0.1.0</sub>"));
}

#[test]
fn all_pass_heading_and_no_table() {
    let mut r = synthetic(0, 3);
    r.config.fail_on_new = false;
    let md = render_markdown(&r, &MarkdownOptions::default());
    assert!(md.lines().nth(1) == Some("### saccade: ✅ 3 passed"));
    assert!(!md.contains("…and"));
    assert_eq!(md.matches("| Status |").count(), 1, "only the pass table");
}

#[test]
fn truncation_drops_pass_rows_first_and_keeps_marker() {
    let r = synthetic(3, 300);
    let full = render_markdown(&r, &MarkdownOptions::default());
    assert!(full.len() > 5_000);
    let md = render_markdown(
        &r,
        &MarkdownOptions {
            artifact_url: None,
            max_bytes: Some(2_000),
            comment_key: None,
        },
    );
    assert!(md.len() <= 2_000, "len {}", md.len());
    for i in 0..3 {
        assert!(
            md.contains(&format!("fail/{i:03}.png")),
            "non-pass row {i} kept"
        );
    }
    assert!(md.contains("<summary>300 passed</summary>"));
    let marker = md
        .lines()
        .find(|l| l.starts_with("…and "))
        .expect("marker line");
    assert!(marker.ends_with("more (see full report)"));
    let kept_pass = md.matches("`pass/").count();
    assert_eq!(
        marker,
        format!("…and {} more (see full report)", 300 - kept_pass)
    );
}

#[test]
fn truncation_drops_non_pass_rows_when_still_too_big() {
    let r = synthetic(100, 10);
    let md = render_markdown(
        &r,
        &MarkdownOptions {
            artifact_url: None,
            max_bytes: Some(1_500),
            comment_key: None,
        },
    );
    assert!(md.len() <= 1_500, "len {}", md.len());
    assert_eq!(
        md.matches("`pass/").count(),
        0,
        "all pass rows dropped first"
    );
    let kept = md.matches("`fail/").count();
    assert!(kept > 0 && kept < 100);
    assert!(md.contains(&format!("…and {} more (see full report)", 110 - kept)));
    assert!(md.starts_with("<!-- saccade-summary -->\n"));
}

#[test]
fn comment_key_changes_the_marker() {
    let keyed = render_markdown(
        &sample(),
        &MarkdownOptions {
            comment_key: Some("linux-x64".into()),
            ..MarkdownOptions::default()
        },
    );
    assert_eq!(
        keyed.lines().next(),
        Some("<!-- saccade-summary:linux-x64 -->")
    );
    assert!(saccade_core::render::is_valid_comment_key("linux-x64"));
    assert!(!saccade_core::render::is_valid_comment_key("a -->b"));
}

#[test]
fn identity_headline_escapes_the_entry_name() {
    use saccade_core::report::{Metrics, Mode};
    let mut r = synthetic(1, 0);
    r.config.mode = Mode::Identity;
    let e = &mut r.entries[0];
    e.name = "a|`b`\n<x>.png".into();
    e.metrics = Some(Metrics {
        mean: 0.1,
        max: 0.2,
        p50: 0.1,
        p95: 0.1,
        p99: 0.1,
        frac_above_0_1: 0.0,
        frac_above_0_5: 0.0,
        width: 1,
        height: 1,
    });
    let h = saccade_core::render::identity_headline(&r).expect("headline");
    assert!(h.contains("on `a\\|ˋbˋ <x>.png`"), "{h}");
}
