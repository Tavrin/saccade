//! Machine-readable output shared by the CLI and the MCP tools: the lean
//! `saccade-result.v1` of a run, the `saccade-summary.v1` of a report, and
//! the `saccade-error.v1` every failing command prints in JSON mode.

use std::path::{Path, PathBuf};

use saccade_core::Entry;
use saccade_core::report::{Metric, Report, Status};
use serde_json::{Value, json};

/// Schema identifier of the summary object.
pub const SUMMARY_SCHEMA: &str = "saccade-summary.v1";

/// Schema identifier of the lean result of `compare` / `identity`.
pub const RESULT_SCHEMA: &str = "saccade-result.v1";

/// Schema identifier of the error object.
pub const ERROR_SCHEMA: &str = "saccade-error.v1";

/// A failed command: a stable machine-readable `code` and a message.
///
/// Codes: `usage` (bad arguments), `io` (a file cannot be read, written or
/// decoded), `config` (a config file or setting is invalid), `unsafe_path`
/// (a path escapes its root or goes through a symlink), `not_empty_out_dir`,
/// `nothing_compared` and `approve_mismatch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    /// Stable error code.
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    /// Path/argument context and an actionable repair.
    pub hint: String,
}

impl CliError {
    /// An error with an explicit code.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        let message = message.into();
        let fix = if message.contains("inside an input") || message.contains("inside a noise input")
        {
            "--out is inside an input dir: choose a sibling dir like ./saccade-report"
        } else {
            match code {
                "config" => {
                    "check the named config setting or glob, correct its value, and rerun `saccade config --explain NAME`"
                }
                "unsafe_path" => "choose a path inside the allowed root without symlink escapes",
                "not_empty_out_dir" => {
                    "choose an empty --out directory or an existing saccade report directory"
                }
                "approve_mismatch" => {
                    "rerun the comparison on the current inputs and approve its report"
                }
                "nothing_compared" => {
                    "check the input paths and --entries filters; bootstrap with `saccade approve --report REPORT_JSON --all-failing`"
                }
                "io" => {
                    "check the named path exists, is readable or writable as needed, and images decode"
                }
                _ => {
                    "correct the named argument; run `saccade COMMAND --help` for its accepted values"
                }
            }
        };
        Self {
            code,
            hint: format!("{message}; {fix}"),
            message,
        }
    }

    /// Bad or missing argument.
    pub fn usage(message: impl Into<String>) -> Self {
        Self::new("usage", message)
    }

    /// A file or stream cannot be read, written or decoded.
    pub fn io(message: impl Into<String>) -> Self {
        Self::new("io", message)
    }

    /// The `saccade-error.v1` object.
    pub fn value(&self) -> Value {
        json!({"schema": ERROR_SCHEMA, "code": self.code, "message": self.message, "hint": self.hint})
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::usage(message)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        Self::io(format!("JSON error: {e}"))
    }
}

impl From<saccade_core::Error> for CliError {
    fn from(e: saccade_core::Error) -> Self {
        use saccade_core::Error;
        let code = match e {
            Error::Config(_) => "config",
            Error::NotEmptyOutDir(_) => "not_empty_out_dir",
            _ => "io",
        };
        Self::new(code, e.to_string())
    }
}

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
        Metric::P99 => "p99",
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
    saccade_core::run::normalise_path(p)
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
        "buffer": e.buffer,
        "class": e.diagnostics.as_ref().map(|d| d.class.as_str()),
        "description": e.diagnostics.as_ref().map(|d| d.description.as_str()),
    })
}

/// The `saccade-summary.v1` object for a report that lives at `report_json`.
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
    let abs_paths = report
        .baseline_dir
        .as_deref()
        .is_some_and(|p| Path::new(p).is_absolute());
    let display = |p: &Path| saccade_core::paths::cwd(p, abs_paths);
    let existing = |p: PathBuf| p.is_file().then(|| display(&p));
    let explain_dir = dir.join("explain");
    json!({
        "schema": SUMMARY_SCHEMA,
        "verdict": if report.is_regression() { "regression" } else { "pass" },
        "regression": report.is_regression(),
        "mode": serde_json::to_value(report.config.mode).unwrap_or(Value::Null),
        "labels": report.config.labels,
        "totals": report.totals,
        "perf_diff": report.perf_diff,
        "perf_errors": report.perf_errors,
        "combined_verdict": report.combined_verdict,
        "failing": shown,
        "failing_omitted": failing.len().saturating_sub(top),
        "paths": {
            "report_json": display(&report_json),
            "report_dir": display(&dir),
            "index_html": existing(dir.join("index.html")),
            "explain_dir": explain_dir.join("explain.json").is_file()
                .then(|| display(&explain_dir)),
            "explain_json": existing(explain_dir.join("explain.json")),
            "explain_md": existing(explain_dir.join("explain.md")),
        },
    })
}

/// A few lines of plain text for a tool's text content block.
pub fn summary_text(report: &Report, value: &Value) -> String {
    let t = &report.totals;
    let mut out = format!(
        "saccade: {} ({} fail, {} error, {} missing, {} new, {} pass of {})",
        value["verdict"].as_str().unwrap_or("?"),
        t.fail,
        t.error,
        t.missing,
        t.new,
        t.pass,
        t.total
    );
    if let Some(v) = &report.combined_verdict {
        out.push_str(&format!("\n{v}"));
    }
    for e in failing_entries(report).into_iter().take(3) {
        let v = e.value.map_or(String::new(), |v| {
            format!(" {} {v:.4} > {}", metric_str(e.metric_used), e.threshold)
        });
        out.push_str(&format!("\n- {} {}{v}", status_str(e.status), e.name));
        if let Some(d) = &e.diagnostics {
            out.push_str(&format!("\n  {}: {}", d.class.as_str(), d.description));
        }
        if let Some(line) = saccade_core::hotspots::summary_line(&e.hotspots) {
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

/// Rounds every non-integer number in `value` to 4 significant digits.
pub fn round_floats(value: &mut Value) {
    match value {
        Value::Number(n) if n.is_f64() => {
            if let Some(x) = n.as_f64() {
                let rounded = format!("{x:.3e}")
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64);
                if let Some(r) = rounded {
                    *n = r;
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(round_floats),
        Value::Object(map) => map.values_mut().for_each(round_floats),
        _ => {}
    }
}

fn lean_entry(e: &Entry) -> Value {
    let mut v = json!({
        "name": e.name,
        "status": status_str(e.status),
        "metric": metric_str(e.metric_used),
        "value": e.value,
        "threshold": e.threshold,
    });
    let Some(obj) = v.as_object_mut() else {
        return v;
    };
    if let Some(err) = &e.error {
        obj.insert("error".into(), json!(err));
    }
    if let Some(buffer) = &e.buffer {
        obj.insert("buffer".into(), json!(buffer));
    }
    let keys: Vec<&str> = e.meta_diff.iter().map(|d| d.key.as_str()).collect();
    if !keys.is_empty() {
        obj.insert("config_differs".into(), json!(keys));
    }
    let failing_regions: Vec<&str> = e
        .regions
        .iter()
        .filter(|r| r.status == Some(Status::Fail))
        .map(|r| r.name.as_str())
        .collect();
    if !failing_regions.is_empty() {
        obj.insert("failing_regions".into(), json!(failing_regions));
    }
    if let Some(d) = &e.diagnostics {
        obj.insert("class".into(), json!(d.class.as_str()));
        obj.insert("description".into(), json!(d.description));
        if let Some(perf) = saccade_core::diagnostics::perf_summary(&d.perf) {
            obj.insert("perf".into(), json!(perf));
        }
    }
    let spots: Vec<Value> = e
        .hotspots
        .iter()
        .take(HOTSPOTS_PER_ENTRY)
        .map(|h| {
            json!({
                "rect_px": h.rect_px,
                "position": h.position,
                "share_of_total_error": h.share_of_total_error,
                "mean_flip": h.mean_flip,
                "max_flip": h.max_flip,
            })
        })
        .collect();
    if !spots.is_empty() {
        obj.insert("hotspots".into(), Value::Array(spots));
    }
    v
}

/// The error for a run that compared no pair in JSON mode; exits 1.
pub fn nothing_compared(report: &Report, report_json: &Path) -> Option<CliError> {
    report.is_empty_run().then(|| {
        CliError::new(
            "nothing_compared",
            format!(
                "no image exists in both directories, so nothing was compared (report: {}); empty comparisons are not evidence",
                absolute(report_json).display()
            ),
        )
    })
}

/// What the agent should do next, as one sentence.
fn next_step(report: &Report, report_json: &Path, explain_written: bool) -> String {
    let failing = failing_entries(report);
    let rj = saccade_core::paths::cwd(report_json, false);
    if report.is_empty_run() {
        return "nothing was compared: no image exists in both directories; check the two paths and selected scope".to_string();
    }
    let approve = format!("saccade approve --report {rj} --all-failing");
    if failing.is_empty() {
        return match report.totals.new {
            0 => "no regression: nothing to do".to_string(),
            n => format!(
                "no regression; {n} new image(s) have no baseline: adopt them with `{approve}` if intended"
            ),
        };
    }
    let only_config = failing
        .iter()
        .all(|e| e.status == Status::Error && !e.meta_diff.is_empty());
    if only_config {
        return "config differs: declare the keys with --declare (and --require-matching-meta) or fix the capture setup, then rerun".to_string();
    }
    let has_fail = failing.iter().any(|e| e.status == Status::Fail);
    if has_fail {
        let look = if explain_written {
            "inspect the strips in the explain pack (paths.explain_md)".to_string()
        } else {
            format!("run `saccade explain {rj}` and inspect the strips")
        };
        return format!("{look}; approve with `{approve}` if the change is intended");
    }
    if failing.iter().any(|e| e.status == Status::Missing) {
        return format!(
            "captures are missing for some baselines: capture them, or drop the baselines with `{approve} --prune-missing`"
        );
    }
    "fix the errors listed in `failing[].error` (unreadable or mismatched images), then rerun"
        .to_string()
}

/// The lean `saccade-result.v1` printed by `compare --json` and
/// `identity --json` and returned by the MCP run tools: verdict, totals, the
/// failing entries (value, threshold, top-3 hotspots, config-diff keys), the
/// report paths and a `next_step`. Floats carry 4 significant digits; paths
/// are absolute.
pub fn result_value(
    report: &Report,
    report_json: &Path,
    top: usize,
    explain_written: bool,
) -> Value {
    let failing = failing_entries(report);
    let report_json = absolute(report_json);
    let dir = report_json
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let abs_paths = report
        .baseline_dir
        .as_deref()
        .is_some_and(|p| Path::new(p).is_absolute());
    let display = |p: &Path| saccade_core::paths::cwd(p, abs_paths);
    let existing = |p: PathBuf| p.is_file().then(|| display(&p));
    let explain_dir = dir.join("explain");
    let validity = report.capture_validity();
    let mut v = json!({
        "schema": RESULT_SCHEMA,
        "verdict": if report.is_regression() { "regression" } else { "pass" },
        "mode": serde_json::to_value(report.config.mode).unwrap_or(Value::Null),
        "totals": report.totals,
        "sample_equality": report.sample_equality(),
        "capture_validity": {"status": validity.status,
            "reasons": validity.reasons.iter().take(5).collect::<Vec<_>>(),
            "reasons_omitted": validity.reasons.len().saturating_sub(5)},
        "scope": {"entries": report.config.entries, "ignore": report.config.ignore},
        "perf_diff": report.perf_diff,
        "perf_errors": report.perf_errors,
        "combined_verdict": report.combined_verdict,
        "failing": failing.iter().take(top).map(|e| lean_entry(e)).collect::<Vec<_>>(),
        "failing_omitted": failing.len().saturating_sub(top),
        "paths": {
            "report_json": display(&report_json),
            "index_html": existing(dir.join("index.html")),
            "explain_md": existing(explain_dir.join("explain.md")),
        },
        "next_step": ({
            let mut next = next_step(report, &report_json, explain_written);
            if report.entries.len() > failing.len().min(top) {
                next.push_str(&format!("; inspect omitted entries with `saccade entries {}` or MCP saccade_list_entries / saccade_get_entry", saccade_core::paths::cwd(&report_json, false)));
            }
            next
        }),
    });
    let warnings: Vec<String> = report
        .entries
        .iter()
        .flat_map(|e| e.warnings.iter().map(move |w| format!("{}: {w}", e.name)))
        .take(5)
        .collect();
    if let (false, Some(obj)) = (warnings.is_empty(), v.as_object_mut()) {
        obj.insert("warnings".into(), json!(warnings));
    }
    round_floats(&mut v);
    v
}
