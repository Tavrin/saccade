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
    assert!(html.contains("<style>") && html.contains("id=\"saccade-data\""));
}

#[test]
fn embedded_json_round_trips_and_escapes_script_breakouts() {
    let mut report = sample();
    report.entries[0].name = "</script><script>alert(1)</script><!-- x".to_string();
    report.entries[0].error = Some("a </b> & \"quoted\"".to_string());
    let html = render(&report, "roundtrip");
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
