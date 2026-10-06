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
    /// The actionable repair, printed under the message on stderr.
    pub hint: String,
    /// Complete typed arm refusal when applicable.
    pub arm_check: Option<Box<saccade_core::arms::Check>>,
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
                "invalid_comparison" => {
                    "recapture with complete matching identity, or declare the intended experiment variables"
                }
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
                "stale_link" | "link_missing" => {
                    "rebuild the manifest with `saccade manifest build` and re-link, or restore the recorded file"
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
            arm_check: None,
            hint: fix.to_owned(),
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
        if let Some(check) = &self.arm_check {
            return serde_json::json!(check);
        }
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
        if e.to_string().starts_with("unknown field ") {
            Self::new(
                "version_skew",
                format!(
                    "written by a newer producer; installed saccade does not support this field, upgrade: {e}"
                ),
            )
        } else {
            Self::io(format!("JSON error: {e}"))
        }
    }
}

impl From<saccade_core::Error> for CliError {
    fn from(e: saccade_core::Error) -> Self {
        use saccade_core::Error;
        if let Error::InvalidComparison(check) = e {
            let mut error = Self::new(
                "invalid_comparison",
                "arm identity validation refused a verdict",
            );
            error.arm_check = Some(check);
            return error;
        }
        let code = match e {
            Error::TrialPlanChanged => "trial_plan_changed",
            Error::Config(_) => "config",
            Error::VersionSkew { .. } => "version_skew",
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

/// Measured failures by severity relative to their own thresholds, then
/// unresolved entries in stable status/name order.
pub fn failing_entries(report: &Report) -> Vec<&Entry> {
    let ratio = |e: &Entry| {
        e.value.map(|v| {
            if e.threshold > 0.0 {
                v / e.threshold
            } else {
                f64::INFINITY
            }
        })
    };
    let mut v: Vec<&Entry> = report
        .entries
        .iter()
        .filter(|e| is_failing(report, e))
        .collect();
    v.sort_by(|a, b| {
        ratio(b)
            .is_some()
            .cmp(&ratio(a).is_some())
            .then_with(|| ratio(b).unwrap_or(0.0).total_cmp(&ratio(a).unwrap_or(0.0)))
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

/// Bounded v2 measurement envelope with stable operation fields.
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
    value["report_id"] = json!(report.report_id);
    value["source_refs"] = json!(report.source_refs);
    if let Some(perf) = &report.perf_diff {
        value["performance"] = json!({"verdict":perf.verdict(),"comparability":perf.comparability,"repeat_qualification":perf.noise_comparability,"summary":perf.summary(3)});
        if !report.is_regression()
            && perf.comparability == saccade_core::perf::Comparability::Rejected
        {
            value["overall"] = json!("performance_rejected");
            value["verdict"] = json!("performance_rejected");
        }
    }
    if report.perf_diff.is_some() || !report.perf_errors.is_empty() {
        value["performance_action"] =
            json!("saccade experiment ablate BASE CAPTURE --out DIR --json");
    }
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
    let missing = report
        .entries
        .iter()
        .flat_map(|e| {
            ["baseline", "capture"].into_iter().flat_map(move |side| {
                ["binary_sha256", "source_head"]
                    .into_iter()
                    .filter(move |field| {
                        !e.capture_provenance
                            .contains_key(&format!("{side}.{field}"))
                    })
                    .map(move |field| format!("{side}.{field}"))
            })
        })
        .collect::<std::collections::BTreeSet<_>>();
    if !missing.is_empty() {
        value["validity_missing"] = json!({"keys":missing,"source":format!("--meta-name {} (--meta-name cost-card.json; binary.sha and build.commit)",report.config.meta.name)});
    }
    let undeclared = report
        .entries
        .iter()
        .flat_map(|e| e.meta_diff.iter().map(|d| d.key.as_str()))
        .filter(|key| !report.config.meta.declared.iter().any(|d| d == key))
        .collect::<std::collections::BTreeSet<_>>();
    if validity.status == saccade_core::meta::Validity::Unknown && !undeclared.is_empty() {
        value["validity_guidance"] = json!({"undeclared_keys":undeclared.into_iter().take(8).collect::<Vec<_>>(),"action":"use --declare KEY for intentional differences, or --require-matching-meta to reject them"});
    }
    value["counts"]["validity_reasons"] = json!(validity.reasons.len());
    value["scope"] = json!({"entries":report.config.entries,"ignore":report.config.ignore});
    let scopes = report
        .entries
        .iter()
        .filter_map(|e| e.field_evidence.as_ref().map(|f| f.scope.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    value["scope"]["kind"] = json!(if scopes.len() == 1 {
        scopes.first().copied().unwrap_or("unknown")
    } else {
        "mixed"
    });
    value["counts"] = json!({"total":report.totals.total,"pass":report.totals.pass,"fail":report.totals.fail,"error":report.totals.error,"missing":report.totals.missing,"new":report.totals.new});
    value["counts"]["validity_reasons"] = json!(validity.reasons.len());
    let local_changes = report
        .entries
        .iter()
        .filter(|e| e.pass_with_local_change)
        .collect::<Vec<_>>();
    if !local_changes.is_empty() {
        value["data"] = json!({"pass_with_local_change":local_changes.len(),"local_changes":local_changes.iter().take(3).map(|e|json!({"entry":e.name,"note":e.local_hotspot_note()})).collect::<Vec<_>>()});
    }
    if report.config.meta.require_valid_arms {
        if !value["data"].is_object() {
            value["data"] = json!({});
        }
        value["data"]["arm_validation"] = json!({"result":"valid_comparison","ignore":report.config.meta.arm_ignore,"vary":report.config.meta.intended,"covered_by_derivation":report.entries.iter().filter(|e| !e.covered_by_derivation.is_empty()).map(|e|json!({"entry":e.name,"fields":e.covered_by_derivation})).collect::<Vec<_>>()});
    }
    if let Some(check) = &report.config.meta.arm_validation {
        value["data"]["arm_validation"]["compare"] = json!(check.compare);
        value["data"]["arm_validation"]["unmapped"] = json!(check.unmapped);
        value["data"]["arm_validation"]["outcomes"] = json!(check.outcomes);
        value["data"]["arm_validation"]["allowed_unreached"] = json!(check.allowed_unreached);
        value["data"]["arm_validation"]["ignored"] =
            json!(check.ignored.iter().take(8).collect::<Vec<_>>());
    }
    let failing = failing_entries(report);
    let summaries=failing.iter().take(top.min(5)).map(|e|json!({"entry_id":e.name,"measurement":if e.status==Status::Fail{"regression"}else{"unknown"},"error":e.error.as_ref().map(|s|crate::local_cmd::short(s,256))})).collect::<Vec<_>>();
    value["entries"] = json!(summaries);
    value["failing"]=json!(failing.iter().take(top.min(5)).map(|e|json!({"name":e.name,"status":status_str(e.status),"value":e.value,"threshold":e.threshold,"error":e.error.as_ref().map(|s|crate::local_cmd::short(s,256))})).collect::<Vec<_>>());
    value["worst"] = failing.iter().find(|e| e.value.is_some()).map_or(
        Value::Null,
        |e| json!({"entry":e.name,"metric":e.metric_used,"value":e.value,"threshold":e.threshold}),
    );
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
        && (!failing.is_empty() || !local_changes.is_empty())
    {
        value["next_actions"] = json!([{
            "id":"inspect-evidence","kind":"inspect_evidence","reason_code":if !local_changes.is_empty() {"pass_with_local_change"} else {"measured_change_or_missing_evidence"},"priority":1,"requires":[],"tool":"saccade_inspect",
            "arguments":{"operation":"summary","artifact":reference},
            "cli_argv":["saccade","inspect",report_path,"--json","--expected-case-id",case.case_id],"cwd":std::env::current_dir().ok().map(|p|saccade_core::paths::portable(&p)),"expected_case_id":case.case_id
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
