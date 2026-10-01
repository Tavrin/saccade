//! Machine-readable summaries shared by `summary --format json` and the MCP
//! tools: verdict, totals, the worst entries with their hotspots, and where the
//! report files are.

use std::path::{Path, PathBuf};

use flipdiff_core::Entry;
use flipdiff_core::report::{Metric, Report, Status};
use serde_json::{Value, json};

/// Schema identifier of the summary object.
pub const SUMMARY_SCHEMA: &str = "flipdiff-summary.v1";

/// How many failing entries a summary lists by default.
pub const DEFAULT_TOP_FAILING: usize = 10;

/// Hotspots listed per failing entry.
const HOTSPOTS_PER_ENTRY: usize = 3;

fn status_str(s: Status) -> &'static str {
    match s {
        Status::Pass => "pass",
        Status::Fail => "fail",
        Status::New => "new",
        Status::Missing => "missing",
        Status::Error => "error",
    }
}

fn metric_str(m: Metric) -> &'static str {
    match m {
        Metric::Mean => "mean",
        Metric::P95 => "p95",
        Metric::Max => "max",
    }
}

/// Whether `e` counts against the verdict.
fn is_failing(report: &Report, e: &Entry) -> bool {
    match e.status {
        Status::Fail | Status::Error | Status::Missing => true,
        Status::New => report.config.fail_on_new,
        Status::Pass => false,
    }
}

/// The entries that fail the run, worst first: fail, error, missing, new; then
/// by deciding value (largest first), then by name.
pub fn failing_entries(report: &Report) -> Vec<&Entry> {
    let rank = |s: Status| match s {
        Status::Fail => 0,
        Status::Error => 1,
        Status::Missing => 2,
        _ => 3,
    };
    let mut v: Vec<&Entry> = report
        .entries
        .iter()
        .filter(|e| is_failing(report, e))
        .collect();
    v.sort_by(|a, b| {
        rank(a.status)
            .cmp(&rank(b.status))
            .then_with(|| {
                b.value
                    .unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(&a.value.unwrap_or(f64::NEG_INFINITY))
            })
            .then_with(|| a.name.cmp(&b.name))
    });
    v
}

fn absolute(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir().map_or_else(|_| p.to_path_buf(), |c| c.join(p))
        }
    })
}

fn entry_value(e: &Entry) -> Value {
    json!({
        "name": e.name,
        "status": status_str(e.status),
        "metric": metric_str(e.metric_used),
        "value": e.value,
        "threshold": e.threshold,
        "error": e.error,
        "bit_identical": e.bit_identical,
        "failing_regions": e.regions.iter()
            .filter(|r| r.status == Some(Status::Fail))
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>(),
        "config_differs": e.meta_diff.iter().map(|d| d.key.as_str()).collect::<Vec<_>>(),
        "hotspots": e.hotspots.iter().take(HOTSPOTS_PER_ENTRY).collect::<Vec<_>>(),
    })
}

/// The `flipdiff-summary.v1` object for a report that lives at `report_json`.
///
/// `paths` lists the report JSON, its directory and, when they exist, the
/// `index.html` and the explain pack in `<report dir>/explain`.
pub fn summary_value(report: &Report, report_json: &Path, top: usize) -> Value {
    let failing = failing_entries(report);
    let shown: Vec<Value> = failing.iter().take(top).map(|e| entry_value(e)).collect();
    let report_json = absolute(report_json);
    let dir = report_json
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let existing = |p: PathBuf| p.is_file().then(|| p.display().to_string());
    let explain_dir = dir.join("explain");
    json!({
        "schema": SUMMARY_SCHEMA,
        "verdict": if report.is_regression() { "regression" } else { "pass" },
        "regression": report.is_regression(),
        "mode": serde_json::to_value(report.config.mode).unwrap_or(Value::Null),
        "labels": report.config.labels,
        "totals": report.totals,
        "failing": shown,
        "failing_omitted": failing.len().saturating_sub(top),
        "paths": {
            "report_json": report_json.display().to_string(),
            "report_dir": dir.display().to_string(),
            "index_html": existing(dir.join("index.html")),
            "explain_dir": explain_dir.join("explain.json").is_file()
                .then(|| explain_dir.display().to_string()),
            "explain_json": existing(explain_dir.join("explain.json")),
            "explain_md": existing(explain_dir.join("explain.md")),
        },
    })
}

/// A few lines of plain text for a tool's text content block.
pub fn summary_text(report: &Report, value: &Value) -> String {
    let t = &report.totals;
    let mut out = format!(
        "flipdiff: {} ({} fail, {} error, {} missing, {} new, {} pass of {})",
        value["verdict"].as_str().unwrap_or("?"),
        t.fail,
        t.error,
        t.missing,
        t.new,
        t.pass,
        t.total
    );
    for e in failing_entries(report).into_iter().take(3) {
        let v = e.value.map_or(String::new(), |v| {
            format!(" {} {v:.4} > {}", metric_str(e.metric_used), e.threshold)
        });
        out.push_str(&format!("\n- {} {}{v}", status_str(e.status), e.name));
        if let Some(line) = flipdiff_core::hotspots::summary_line(&e.hotspots) {
            out.push_str(&format!("\n  {line}"));
        }
    }
    if let Some(p) = value["paths"]["index_html"].as_str() {
        out.push_str(&format!("\nreport: {p}"));
    }
    if let Some(p) = value["paths"]["explain_md"].as_str() {
        out.push_str(&format!("\nexplain: {p}"));
    }
    out
}
