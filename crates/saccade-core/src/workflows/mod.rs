#![doc = include_str!("../../../../docs/library.md")]

//! Command-level orchestration for Rust callers.
//!
//! Options carry policy, paths, configuration and authorization explicitly.
//! These functions return reports and artifacts without writing to stdout or exiting.
//! Existing low-level measurement functions remain available with unchanged names.

pub mod approval;
#[cfg(any(unix, windows))]
pub mod batch;
mod engine_ingest;
mod error;
#[cfg(any(unix, windows))]
pub mod history;
pub mod ingest;
pub mod last_good;
pub mod manifest;
#[cfg(feature = "ai")]
pub mod review;
pub mod signed_approval;
mod support;

pub use error::CommandError;
pub use support::{
    case_for_result, case_from_report, lexical_record, persist_case, report_input,
    verify_visual_intent, visual_intent,
};

use crate::{Report, config::RunConfig};
use std::path::{Path, PathBuf};

/// A versioned wire result for operations whose documents have multiple schema shapes.
#[derive(Debug)]
pub struct WireResult {
    /// Complete versioned result, retaining the CLI serialization contract.
    pub value: serde_json::Value,
}

/// Declared text or structured visual intent and optional expected changes.
#[derive(Clone, Debug, Default)]
pub struct IntentOptions {
    /// Plain-language declaration.
    pub intent: Option<String>,
    /// Structured evidence or visual intent file.
    pub intent_file: Option<PathBuf>,
    /// JSON list of declared changes; requires an intent.
    pub changes_file: Option<PathBuf>,
}

/// Directory comparison and artifact-writing options.
pub struct CompareRun<'a> {
    /// Baseline directory or image.
    pub baseline: &'a Path,
    /// Candidate directory or image.
    pub capture: &'a Path,
    /// Dedicated report directory.
    pub out: &'a Path,
    /// Effective measurement configuration (including mode).
    pub config: &'a RunConfig,
    /// Explicit signed-approval policy.
    pub policy: &'a signed_approval::Policy,
    /// Require a signed baseline even if policy defaults off.
    pub approved: bool,
    /// Optional previously verified inventory, rechecked against snapshotted bytes.
    pub verified: Option<&'a std::collections::BTreeMap<String, String>>,
    /// Optional JUnit output file.
    pub junit: Option<&'a Path>,
    /// Declared intent persisted with the evidence case.
    pub intent: &'a IntentOptions,
}

/// Comparison report plus command-level verdict and diagnostic messages.
pub struct RunReport {
    /// Full measured report.
    pub report: Report,
    /// Deterministic intent findings rejected the declaration.
    pub intent_mismatch: bool,
    /// CLI-compatible warnings; the library never prints them.
    pub warnings: Vec<String>,
}
impl RunReport {
    /// Whether image measurement or declared intent rejects the run.
    pub fn is_regression(&self) -> bool {
        self.report.is_regression() || self.intent_mismatch
    }
}

/// Measure, write report/evidence/intent artifacts and optional JUnit output.
/// Configuration is already typed; CLI flag parsing remains in the CLI.
pub fn run_compare(opts: &CompareRun<'_>) -> Result<RunReport, CommandError> {
    guard_junit(opts.junit, &[opts.baseline, opts.capture], opts.out)?;
    let mut config = opts.config.clone();
    let visual = visual_intent(opts.intent)?;
    if let Some((declaration, source)) = &visual {
        crate::intent::apply_effects(declaration, source, &mut config)?;
    }
    let inventory = if opts.approved && opts.verified.is_none() {
        Some(opts.policy.check_baseline(opts.baseline)?)
    } else {
        None
    };
    let report = opts.policy.run(
        opts.baseline,
        opts.capture,
        opts.out,
        &config,
        opts.verified.or(inventory.as_ref()),
    )?;
    if let Some(path) = opts.junit {
        crate::ergonomics::junit(&report, path)?;
    }
    persist_case(
        &report,
        &opts.out.join(crate::report::REPORT_FILE_NAME),
        opts.intent,
    )?;
    let intent_mismatch = verify_visual_intent(&report, opts.out, visual.as_ref())?;
    let warnings = crate::run::workflow_warnings(&config, &report, opts.baseline);
    Ok(RunReport {
        report,
        intent_mismatch,
        warnings,
    })
}

/// Prove exact native identity using the same artifacts and intent verification.
/// Rejects a regression-mode configuration rather than silently weakening identity.
pub fn run_prove(opts: &CompareRun<'_>) -> Result<RunReport, CommandError> {
    if opts.config.mode != crate::report::Mode::Identity {
        return Err(CommandError::usage("identity proof requires identity mode"));
    }
    run_compare(opts)
}

/// Parse retained contract bytes, rejecting newer fields and nested schema versions.
pub fn parse_contract<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    schema: &str,
) -> Result<T, CommandError> {
    support::parse_contract(bytes, schema)
}

/// Read a report under the active root I/O scope, preserving legacy reader support.
pub fn read_report(path: &Path) -> Result<Report, CommandError> {
    let text = crate::root_policy::io::read_to_string(path)
        .map_err(|e| CommandError::io(format!("reading report {}: {e}", path.display())))?;
    parse_contract(text.as_bytes(), crate::report::REPORT_SCHEMA)
}

pub(crate) fn guard_junit(
    path: Option<&Path>,
    inputs: &[&Path],
    out: &Path,
) -> Result<(), CommandError> {
    let Some(path) = path else { return Ok(()) };
    let norm = crate::run::normalise_path(path);
    if inputs
        .iter()
        .any(|p| norm.starts_with(crate::run::normalise_path(p)))
    {
        return Err(CommandError::usage(format!(
            "--junit {} is inside an input: choose a sibling results.xml",
            path.display()
        )));
    }
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CommandError::new(
            "unsafe_path",
            format!(
                "--junit {} is a symlink: choose a regular XML file",
                path.display()
            ),
        ));
    }
    let base = crate::run::normalise_path(out);
    if norm.starts_with(base.join("images"))
        || [
            crate::report::REPORT_FILE_NAME,
            "saccade-rank.v1.json",
            "saccade-sequence.v1.json",
            "index.html",
            "ranking.md",
            ".saccade-run",
        ]
        .iter()
        .any(|name| norm == base.join(name))
    {
        return Err(CommandError::usage(format!(
            "--junit {} would replace report data: choose results.xml",
            path.display()
        )));
    }
    Ok(())
}

/// Return legacy measurement diagnostics for a command adapter to print.
pub fn run_warnings(config: &RunConfig, report: &Report, baseline: &Path) -> Vec<String> {
    crate::run::workflow_warnings(config, report, baseline)
}

/// Refuse nested contracts from newer producer versions before consuming their data.
pub fn validate_contract_versions(value: &serde_json::Value) -> Result<(), CommandError> {
    support::reject_newer_nested_schemas(value)
}
