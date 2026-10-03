//! Tests for the HTML report.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::PathBuf;

use saccade_core::render::render_html;
use saccade_core::report::Report;

fn sample() -> Report {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/sample-report/saccade-report.v1.json");
    let text = std::fs::read_to_string(path).expect("sample report readable");
    serde_json::from_str(&text).expect("sample report parses")
}

fn render(report: &Report, tag: &str) -> String {
    let dir =
        std::env::temp_dir().join(format!("saccade-render-html-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = render_html(report, &dir).expect("render ok");
    assert_eq!(
        saccade_core::paths::canonicalize(&path).expect("canonical report path"),
        saccade_core::paths::canonicalize(&dir)
            .expect("canonical temp dir")
            .join("index.html")
    );
    let html = std::fs::read_to_string(&path).expect("index.html written");
    let _ = std::fs::remove_dir_all(&dir);
    html
}

fn embedded_json(html: &str) -> &str {
    let open = "<script type=\"application/json\" id=\"saccade-data\">";
    let start = html.find(open).expect("data script present") + open.len();
    let end = html[start..].find("</script>").expect("data script closed");
    &html[start..start + end]
}

#[test]
fn html_is_self_contained() {
    let html = render(&sample(), "selfcontained");
    for needle in [
        "http://",
        "https://",
        "src=\"//",
        "href=\"//",
        "@import",
        "url(//",
    ] {
        assert!(!html.contains(needle), "found external reference: {needle}");
    }
    assert!(!html.contains("/*__SACCADE"), "unfilled placeholder");
    assert!(!html.contains("__SACCADE_SUMMARY__"));
    assert!(!html.contains("__SACCADE_DATA__"));
    assert!(html.contains("<style>") && html.contains("id=\"saccade-data\""));
}

#[test]
fn embedded_json_round_trips_and_escapes_script_breakouts() {
    let mut report = sample();
    report.entries[0].name =
        "</script><script>alert(1)</script><!-- x __SACCADE_DATA__ __SACCADE_SUMMARY__".to_string();
    report.entries[0].error = Some("a </b> & \"quoted\"".to_string());
    report.config.labels.baseline =
        "baseline __SACCADE_DATA__ __SACCADE_SUMMARY__ /*__SACCADE_JS__*/".into();
    let html = render(&report, "roundtrip");
    let summary = html
        .split_once("<section class=\"review-summary\"")
        .expect("review summary")
        .1
        .split_once("</section>")
        .expect("summary closes")
        .0;
    assert!(summary.contains(&report.config.labels.baseline));
    let data = embedded_json(&html);
    assert!(!data.contains("</"), "raw `</` leaked into the data script");
    assert!(
        !data.contains("<!--"),
        "raw `<!--` leaked into the data script"
    );
    assert!(data.contains("<\\/script>"));
    let parsed: Report = serde_json::from_str(data).expect("embedded JSON parses");
    assert_eq!(parsed, report);
}

#[test]
fn identity_header_names_native_proof_and_selected_scope_instead_of_metric_settings() {
    let mut report = sample();
    report.config.mode = saccade_core::report::Mode::Identity;
    report.config.entries = vec!["scene*.png".into()];
    report.config.ignore = vec!["excluded.png".into()];
    let html = render(&report, "identity-scope");
    let meta = html
        .split_once("<dl class=\"meta\" id=\"meta\">")
        .unwrap()
        .1
        .split_once("</dl>")
        .unwrap()
        .0;
    assert!(meta.contains("Exact native decoded samples"));
    assert!(meta.contains("scene*.png") && meta.contains("excluded.png"));
    assert!(meta.contains(&report.entries[0].name));
    for key in ["threshold", "metric", "ppd"] {
        assert!(!meta.contains(&format!("<dt>{key}</dt>")));
    }
    let summary = html
        .split_once("<section class=\"review-summary\"")
        .unwrap()
        .1
        .split_once("</section>")
        .unwrap()
        .0;
    assert!(!summary.contains("pixels per degree"));
    assert!(summary.contains("No tolerance or mask establishes identity"));
    report.config.mode = saccade_core::report::Mode::Regression;
    let html = render(&report, "regression-header");
    assert!(html.contains("<dt>threshold</dt>") && html.contains("<dt>ppd</dt>"));
}
