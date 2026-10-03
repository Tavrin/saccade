//! Bounded measurement results and errors shared by CLI and local MCP.

use std::path::Path;

use saccade_core::Entry;
use saccade_core::report::{Report, Status};
use serde_json::{Value, json};

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
                "feature_unavailable" => {
                    "rebuild with the required feature; inspect enabled modules with `saccade inspect capabilities`"
                }
                "config" => {
                    "check the named config setting or glob, correct its value, and rerun `saccade inspect config --entry NAME`"
                }
                "unsafe_path" => "choose a path inside the allowed root without symlink escapes",
                "not_empty_out_dir" => {
                    "choose an empty --out directory or an existing saccade report directory"
                }
                "approve_mismatch" => {
                    "rerun the comparison, prepare --dry-run --out PLAN, review the exact decision, then apply --decisions PLAN/decision.json under explicit human authorization"
                }
                "nothing_compared" => {
                    "check the input paths and --entry filters; bootstrap with `saccade approve --report REPORT_JSON --all-failing`"
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

    /// The shared result envelope carrying an execution error.
    pub fn value(&self) -> Value {
        {
            let mut value = crate::local_cmd::base_result("error");
            value["execution"] = json!("error");
            value["errors"] = json!([{"code":self.code,"message":self.message.chars().take(512).collect::<String>(),"required_feature":if self.code=="feature_unavailable" {["graphics","evaluation","workbench","prechecks","mcp","ai"].iter().find(|name|self.message.contains(**name)).copied()}else{None}}]);
            value
        }
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
            Error::WrongNoiseKind { .. } => "wrong_noise_kind",
            Error::FeatureUnavailable { .. } => "feature_unavailable",
            Error::NotEmptyOutDir(_) => "not_empty_out_dir",
            _ => "io",
        };
        Self::new(code, e.to_string())
    }
}

/// How many failing entries a summary lists by default.
pub const DEFAULT_TOP_FAILING: usize = 5;

/// Hotspots listed per failing entry.
fn status_str(s: Status) -> &'static str {
    match s {
        Status::Pass => "pass",
        Status::Fail => "fail",
        Status::New => "new",
        Status::Missing => "missing",
        Status::Error => "error",
    }
}
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

/// Bounded v2 measurement envelope with the named Moss fields retained.
pub fn result_value(
    report: &Report,
    report_json: &Path,
    top: usize,
    _explain_written: bool,
) -> Value {
    let mut value = crate::local_cmd::base_result(
        if report.config.mode == saccade_core::report::Mode::Identity {
            "identity"
        } else {
            "compare"
        },
    );
    let reference = crate::local_cmd::reference(report_json).ok();
    value["artifact"] = reference.clone().unwrap_or(Value::Null);
    value["mode"] = json!(report.config.mode);
    value["verdict"] = json!(if report.is_regression() {
        "regression"
    } else {
        "pass"
    });
    value["totals"] = json!(report.totals);
    let equality = report.sample_equality();
    value["sample_equality"] = json!(equality);
    value["measurement"] = json!(
        if report.config.mode == saccade_core::report::Mode::Identity {
            match equality {
                Some(true) => "identical",
                Some(false) => "different",
                _ => "unknown",
            }
        } else if report.is_regression() {
            "regression"
        } else {
            "pass"
        }
    );
    let validity = report.capture_validity();
    value["validity"] = json!(validity.status);
    // Group identical invariant failures; the full report binds all affected entries.
    let reasons = validity
        .reasons
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    value["validity_reasons"] = crate::local_cmd::validity_summary(&reasons);
    value["capture_validity"] = json!({"status":validity.status,"reasons":crate::local_cmd::validity_summary(&validity.reasons)});
    value["counts"]["validity_reasons"] = json!(validity.reasons.len());
    value["scope"] = json!({"entries":report.config.entries,"ignore":report.config.ignore});
    value["counts"] = json!({"total":report.totals.total,"pass":report.totals.pass,"fail":report.totals.fail,"error":report.totals.error,"missing":report.totals.missing,"new":report.totals.new});
    value["counts"]["validity_reasons"] = json!(validity.reasons.len());
    let failing = failing_entries(report);
    let summaries=failing.iter().take(top.min(5)).map(|e|json!({"entry_id":e.name,"measurement":if e.status==Status::Fail{"regression"}else{"unknown"},"error":e.error.as_ref().map(|s|crate::local_cmd::short(s,256))})).collect::<Vec<_>>();
    value["entries"] = json!(summaries);
    value["failing"]=json!(failing.iter().take(top.min(5)).map(|e|json!({"name":e.name,"status":status_str(e.status),"value":e.value,"threshold":e.threshold,"error":e.error.as_ref().map(|s|crate::local_cmd::short(s,256))})).collect::<Vec<_>>());
    let report_path = saccade_core::paths::cwd(
        report_json,
        report
            .baseline_dir
            .as_deref()
            .is_some_and(|p| Path::new(p).is_absolute()),
    );
    value["paths"] = json!({"report_json":report_path,"index_html":saccade_core::paths::cwd(&report_json.with_file_name("index.html"),report.baseline_dir.as_deref().is_some_and(|p|Path::new(p).is_absolute()))});
    let omitted = failing.len().saturating_sub(top.min(5));
    value["page"] = json!({"omitted":omitted,"next_cursor":crate::local_cmd::failing_cursor(report_json,top.min(5),failing.len()).ok().flatten()});
    if let Ok(case) = crate::local_cmd::case_for_result(report, report_json)
        && !failing.is_empty()
    {
        value["next_actions"] = json!([{
            "id":"inspect-evidence","kind":"inspect_evidence","reason_code":"measured_change_or_missing_evidence","priority":1,"requires":[],"tool":"saccade_inspect",
            "arguments":{"operation":"summary","artifact":reference},
            "cli_argv":["saccade","inspect",report_path,"--json","--expected-case-id",case.case_id],"expected_case_id":case.case_id
        }]);
    }
    if report.is_empty_run() {
        value["limits"] = json!(["Empty comparisons are not evidence."]);
    }
    round_floats(&mut value);
    match crate::local_cmd::bounded(value, 4096) {
        Ok(mut value) => {
            let shown = value["entries"].as_array().map_or(0, Vec::len);
            value["page"]["next_cursor"] = json!(
                crate::local_cmd::failing_cursor(report_json, shown, failing.len())
                    .ok()
                    .flatten()
            );
            value
        }
        Err(error) => error.value(),
    }
}
