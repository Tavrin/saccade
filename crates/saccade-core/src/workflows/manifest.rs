//! Manifest orchestration, coverage files and approval enforcement.
use super::{CommandError as CliError, signed_approval::Policy};
use crate::manifest::{self, Anchors};
use serde_json::Value;
use std::path::{Path, PathBuf};
/// Inputs to build a manifest, with distinct approved and last-good anchors.
pub struct BuildOptions<'a> {
    /// Report directory.
    pub dir: &'a Path,
    /// Externally approved anchor document.
    pub approved_anchor: Option<&'a Path>,
    /// Continuity reference; grants no approval.
    pub last_good: Option<&'a Path>,
    /// Optional declared cases document.
    pub cases: Option<&'a Path>,
    /// Explicit authentication policy.
    pub policy: &'a Policy,
}
/// Manifest document and the path at which it was written.
pub struct ManifestOutput {
    /// Written document path.
    pub path: PathBuf,
    /// Versioned manifest document (v1 or v2).
    pub document: Value,
}
/// Build and write a manifest after verifying declared approval anchors.
pub fn build(opts: &BuildOptions<'_>) -> Result<ManifestOutput, CliError> {
    if opts.policy.require_signed_approval
        && let Some(anchor) = opts.approved_anchor
    {
        opts.policy.check_anchor(anchor, None)?;
    }
    let anchors = Anchors {
        approved: opts.approved_anchor,
        last_good: opts.last_good,
    };
    let (path, document) = if let Some(cases) = opts.cases {
        let declaration = crate::coverage::read_declaration(cases)?;
        if opts.policy.require_signed_approval {
            opts.policy.check_case_anchors(
                &opts.dir.join(manifest::MANIFEST_FILE),
                &serde_json::to_value(&declaration.cases)?,
            )?;
        }
        crate::coverage::write_manifest(opts.dir, anchors, &declaration)?
    } else {
        manifest::write(opts.dir, anchors)?
    };
    Ok(ManifestOutput { path, document })
}
/// Reproducible coverage and reference-health view options.
pub struct ViewsOptions<'a> {
    /// Manifest file or directory.
    pub target: &'a Path,
    /// Coverage JSON and HTML output directory.
    pub out: &'a Path,
    /// Declared grouping axes, empty for all.
    pub group_by: &'a [String],
    /// Observation time supplied by the caller.
    pub now_unix: u64,
    /// Maximum age of recorded references.
    pub max_age_seconds: u64,
    /// Explicit approval policy.
    pub policy: &'a Policy,
}
/// Analyze declared variants and write linked coverage JSON and HTML.
pub fn views(opts: &ViewsOptions<'_>) -> Result<crate::coverage::Report, CliError> {
    let path = if opts.target.is_dir() {
        opts.target.join(manifest::MANIFEST_FILE)
    } else {
        opts.target.to_owned()
    };
    if opts.policy.require_signed_approval {
        opts.policy.check_manifest(&path)?;
    }
    let report =
        crate::coverage::analyze(&path, opts.group_by, opts.now_unix, opts.max_age_seconds)?;
    crate::run::guard_output_dir(opts.out, &[&path], &["coverage.json"])?;
    std::fs::create_dir_all(opts.out).map_err(|e| CliError::io(e.to_string()))?;
    let artifact = opts.out.join("coverage.json");
    let linked = crate::report_links::decorate(&serde_json::to_value(&report)?)?;
    manifest::write_owned(&artifact, crate::coverage::REPORT_SCHEMA, &linked)?;
    crate::report_links::index(&artifact, &linked)?;
    crate::render::render_coverage_html(&report, &path, opts.out)?;
    Ok(report)
}
/// Reverify a manifest's approval and every recorded artifact hash.
pub fn verify(target: &Path, policy: &Policy) -> Result<(), CliError> {
    if policy.require_signed_approval {
        policy.check_manifest(target)?;
    }
    let findings = manifest::verify(target)?;
    if !findings.is_empty() {
        let code = if findings
            .iter()
            .any(|f| f["code"] == manifest::CODE_STALE_LINK)
        {
            "stale_link"
        } else {
            "link_missing"
        };
        let listed = findings
            .iter()
            .take(5)
            .filter_map(|f| f["message"].as_str())
            .collect::<Vec<_>>()
            .join("; ");
        let mut error = CliError::new(
            code,
            format!("{} link failure(s): {listed}", findings.len()),
        );
        error.hint="rebuild the manifest with `saccade manifest build` and re-link, or restore the recorded file".into();
        return Err(error);
    }
    Ok(())
}
/// Verify approval and write a stable link to a manifest report identity.
pub fn link(dir: &Path, report_id: &str, out: &Path, policy: &Policy) -> Result<(), CliError> {
    if policy.require_signed_approval {
        policy.check_manifest(dir)?;
    }
    manifest::link(dir, report_id, out)?;
    Ok(())
}
/// Classify a local document, verifying approval for manifest consumers.
pub fn classify(path: &Path, policy: &Policy) -> Result<super::WireResult, CliError> {
    let value = manifest::classify(path)?;
    if policy.require_signed_approval
        && (value["manifest"] == true || value["kind"] == "manifest" || value["kind"] == "link")
    {
        policy.check_manifest(path)?;
    }
    Ok(super::WireResult { value })
}
