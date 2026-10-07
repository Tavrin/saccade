//! Bounded local batch intake and report orchestration.
use super::{CommandError as CliError, signed_approval::Policy};
use crate::batch::{self, Options};
use std::collections::BTreeMap;
use std::path::Path;
/// Batch command inputs; executable is explicit rather than discovered globally.
pub struct BatchRun<'a> {
    /// Folder or versioned intake manifest.
    pub source: &'a Path,
    /// Optional paired reference directory.
    pub reference_dir: Option<&'a Path>,
    /// Dedicated resumable output directory.
    pub out: &'a Path,
    /// Validated section/concurrency/deadline options.
    pub options: &'a Options,
    /// Installed executable for the existing subprocess transport.
    pub executable: &'a Path,
    /// Explicit approval policy, refusing unsupported subprocess propagation.
    pub policy: &'a Policy,
}
/// Batch completion and wire summary; item failures remain visible.
pub struct BatchResult {
    /// Exact status counts.
    pub counts: BTreeMap<String, usize>,
    /// Number of input receipts.
    pub rows: usize,
    /// Whether any row is refused, partial or failed.
    pub failed: bool,
    /// Existing versioned batch result.
    pub value: serde_json::Value,
}
/// Intake and execute a batch, write receipts and manifest, and summarize all rows.
pub fn run_batch(opts: &BatchRun<'_>) -> Result<BatchResult, CliError> {
    if opts.policy.require_signed_approval {
        return Err(CliError::new(
            "approval_consumer_unsupported",
            "signed policy refuses subprocess measurement workflows without policy propagation",
        ));
    }
    if opts.source.is_dir() {
        let source =
            crate::paths::canonicalize(opts.source).map_err(|e| CliError::io(e.to_string()))?;
        if crate::run::normalise_path(opts.out).starts_with(source) {
            return Err(CliError::usage("batch output is inside an input directory"));
        }
    }
    let inputs = batch::intake(opts.source, opts.reference_dir)?;
    let rows = batch::run(opts.executable, &inputs, opts.options, opts.out)?;
    let failed = rows.iter().any(|r| {
        !matches!(
            r["status"].as_str(),
            Some("ok" | "duplicate-basename" | "skipped")
        )
    });
    let mut counts = BTreeMap::new();
    for row in &rows {
        *counts
            .entry(row["status"].as_str().unwrap_or("partial").to_owned())
            .or_insert(0usize) += 1;
    }
    let value = serde_json::json!({"schema":batch::RESULT_SCHEMA,"rows":rows.len(),"counts":counts,"out":opts.out,"manifest":opts.out.join(crate::manifest::MANIFEST_FILE)});
    Ok(BatchResult {
        counts,
        rows: rows.len(),
        failed,
        value,
    })
}

/// Core-only batch inputs for the existing cooperative-deadline transport.
pub struct InProcessBatchRun<'a> {
    /// Folder or versioned input manifest.
    pub source: &'a Path,
    /// Optional paired reference directory.
    pub reference_dir: Option<&'a Path>,
    /// Dedicated resumable output directory.
    pub out: &'a Path,
    /// Sections, concurrency and whole-item deadline.
    pub options: &'a Options,
    /// Exact loaded application/library file used to bind resumable worker identity.
    pub library: &'a Path,
    /// Explicit approval policy; signed comparisons require snapshot-aware execution.
    pub policy: &'a Policy,
}

/// Run supported core sections without spawning a CLI, retaining partial/refused rows.
/// Deadlines are cooperative; unsupported sections/options never become successful rows.
pub fn run_batch_in_process(opts: &InProcessBatchRun<'_>) -> Result<BatchResult, CliError> {
    if opts.policy.require_signed_approval {
        return Err(CliError::new(
            "approval_consumer_unsupported",
            "signed policy refuses in-process batch measurement without snapshot propagation",
        ));
    }
    if opts.source.is_dir() {
        let source =
            crate::paths::canonicalize(opts.source).map_err(|e| CliError::io(e.to_string()))?;
        if crate::run::normalise_path(opts.out).starts_with(source) {
            return Err(CliError::usage("batch output is inside an input directory"));
        }
    }
    let inputs = batch::intake(opts.source, opts.reference_dir)?;
    let rows = batch::run_in_process(opts.library, &inputs, opts.options, opts.out)?;
    let failed = rows.iter().any(|r| {
        !matches!(
            r["status"].as_str(),
            Some("ok" | "duplicate-basename" | "skipped")
        )
    });
    let mut counts = BTreeMap::new();
    for row in &rows {
        *counts
            .entry(row["status"].as_str().unwrap_or("partial").to_owned())
            .or_insert(0usize) += 1;
    }
    let value = serde_json::json!({"schema":batch::RESULT_SCHEMA,"rows":rows.len(),"counts":counts,"out":opts.out,"manifest":opts.out.join(crate::manifest::MANIFEST_FILE)});
    Ok(BatchResult {
        counts,
        rows: rows.len(),
        failed,
        value,
    })
}
