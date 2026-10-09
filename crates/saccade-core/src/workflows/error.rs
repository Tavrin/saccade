//! Stable workflow failures shared with CLI adapters.
use serde_json::{Value, json};
/// A failed command: a stable machine-readable `code` and a message.
///
/// Codes: `usage` (bad arguments), `io` (a file cannot be read, written or
/// decoded), `config` (a config file or setting is invalid), `unsafe_path`
/// (a path escapes its root or goes through a symlink), `not_empty_out_dir`,
/// `nothing_compared` and `approve_mismatch`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    /// Stable error code.
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    /// The actionable repair, printed under the message on stderr.
    pub hint: String,
    /// Complete typed arm refusal when applicable.
    pub arm_check: Option<Box<crate::arms::Check>>,
}

impl CommandError {
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
                "vision_unavailable" => {
                    "run `saccade doctor` for the build's features and missing models, then follow the fix named in the message"
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
            let mut value = super::support::base_result("error");
            value["execution"] = json!("error");
            value["errors"] = json!([{"code":self.code,"message":self.message.chars().take(512).collect::<String>(),"required_feature":if self.code=="feature_unavailable" {["graphics","evaluation","workbench","prechecks","mcp","ai"].iter().find(|name|self.message.contains(**name)).copied()}else{None}}]);
            value
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::usage(message)
    }
}

impl From<serde_json::Error> for CommandError {
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

impl From<crate::Error> for CommandError {
    fn from(e: crate::Error) -> Self {
        use crate::Error;
        if let Error::ApprovalRefused { code, message } = e {
            return Self::new(code, message);
        }
        if let Error::Document { code } = &e {
            return Self::new(code, e.to_string());
        }
        if let Error::InvalidComparison(check) = e {
            let mut error = Self::new(
                "invalid_comparison",
                "arm identity validation refused a verdict",
            );
            error.arm_check = Some(check);
            return error;
        }
        let code = match e {
            Error::ApprovalContentMismatch => "approval_content_mismatch",
            Error::ReviewBoard { code, .. } => code,
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

impl std::error::Error for CommandError {}
impl From<crate::evidence::ContractError> for CommandError {
    fn from(e: crate::evidence::ContractError) -> Self {
        Self::new("invalid_evidence", e.to_string())
    }
}
