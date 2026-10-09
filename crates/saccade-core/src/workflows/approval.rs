//! Exact-content-bound CLI baseline updates. CLI records never attest a person.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::evidence::canonical::{self, Digest};
use crate::evidence::case::{
    ArtifactRef, Availability, EvidenceCase, Input, Measurement, Provenance, Scope, Validity,
    ValidityStatus,
};
use crate::evidence::human::{
    AppliedEntry, ApprovalReceipt, Channel, Disposition, HumanDecision, ReviewBinding,
    ReviewerExposure,
};
use crate::evidence::{Artifact, Document};
use crate::paths;
use crate::report::{Report, Status};
use serde_json::json;

use super::CommandError as CliError;

/// Exact-content approval application options; no signing key is accepted.
#[derive(Default)]
pub struct Options {
    /// Policy.
    pub policy: super::signed_approval::Policy,
    /// Signing.
    pub signing: super::signed_approval::SigningArgs,
    /// Names.
    pub names: Vec<String>,
    /// All failing.
    pub all_failing: bool,
    /// Include errors.
    pub include_errors: bool,
    /// Prune missing.
    pub prune_missing: bool,
    /// Dry run.
    pub dry_run: bool,
    /// Out.
    pub out: Option<PathBuf>,
    /// Absolute.
    pub absolute: bool,
}

fn contract(e: crate::evidence::ContractError) -> CliError {
    CliError::new("approve_mismatch", e.to_string())
}
fn io(e: std::io::Error) -> CliError {
    CliError::io(e.to_string())
}
fn mismatch(message: impl Into<String>) -> CliError {
    CliError::new("approve_mismatch", message)
}
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}
fn safe(name: &str) -> Result<(), CliError> {
    if crate::view::is_safe_name(name) && !name.contains(['\\', ':']) {
        Ok(())
    } else {
        Err(CliError::new(
            "unsafe_path",
            format!("unsafe image name {name:?}"),
        ))
    }
}
fn digest(hash: &Option<String>) -> Result<Option<Digest>, CliError> {
    hash.as_ref()
        .map(|h| Digest::parse(format!("sha256:{h}")).map_err(contract))
        .transpose()
}
fn current(path: &Path) -> Result<Option<Vec<u8>>, CliError> {
    match std::fs::symlink_metadata(paths::native(path)) {
        Ok(m) if m.is_file() => std::fs::read(paths::native(path)).map(Some).map_err(io),
        Ok(_) => Err(mismatch(format!(
            "{} is not a regular file",
            paths::portable(path)
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(e)),
    }
}
fn checked(
    path: &Path,
    expected: &Option<Digest>,
    what: &str,
) -> Result<Option<Vec<u8>>, CliError> {
    let bytes = current(path)?;
    if bytes.as_deref().map(Digest::of_bytes) != *expected {
        return Err(mismatch(format!(
            "{}: the {what} file changed since it was reviewed (including absence)",
            paths::portable(path)
        )));
    }
    Ok(bytes)
}
fn entry<'a>(report: &'a Report, name: &str) -> Result<&'a crate::Entry, CliError> {
    safe(name)?;
    let mut matches = report.entries.iter().filter(|e| e.name == name);
    let e = matches
        .next()
        .ok_or_else(|| mismatch(format!("report has no entry {name:?}")))?;
    if matches.next().is_some() {
        return Err(mismatch("report contains duplicate entry names"));
    }
    Ok(e)
}

/// Builds the canonical R4 case for the exact selected report scope. Missing
/// native/capture/build evidence remains explicit rather than inferred.
fn case_for(
    report_path: &Path,
    report: &Report,
    capture: &Path,
    baseline: &Path,
    scope: Vec<String>,
    document: &Path,
    absolute: bool,
) -> Result<EvidenceCase, CliError> {
    let mut inputs = Vec::new();
    for name in &scope {
        let e = entry(report, name)?;
        for (role, dir, hash, recorded_path) in [
            ("baseline", baseline, &e.baseline_sha256, &e.paths.baseline),
            ("candidate", capture, &e.capture_sha256, &e.paths.capture),
        ] {
            if recorded_path.is_some() != hash.is_some() {
                return Err(mismatch(format!(
                    "{name}: {role} lacks a recorded content hash"
                )));
            }
            if let Some(hash) = digest(hash)? {
                let image = dir.join(name);
                let mut sidecars = Vec::new();
                let per_image = image.with_file_name(format!(
                    "{}.{}",
                    image.file_stem().unwrap_or_default().to_string_lossy(),
                    report.config.meta.name
                ));
                for sidecar in [dir.join(&report.config.meta.name), per_image] {
                    if current(&sidecar)?.is_some() {
                        sidecars.push(
                            ArtifactRef::from_file(&sidecar, document, absolute)
                                .map_err(contract)?,
                        );
                    }
                }
                inputs.push(Input {
                    id: format!("{role}/{name}"),
                    content: ArtifactRef {
                        path: paths::record(
                            &image,
                            document.parent().unwrap_or(Path::new(".")),
                            absolute,
                        ),
                        sha256: hash,
                    },
                    sidecars,
                    native_samples: Availability::missing(
                        "native description remains in the measured report",
                    ),
                    capture: Availability::missing(
                        "capture context remains in the measured report",
                    ),
                    build: Availability::missing("build identity was not independently verified"),
                    provenance: Provenance::default(),
                });
            }
        }
    }
    let validity = report.capture_validity();
    let mut case = EvidenceCase {
        case_id: Digest::of_bytes(b""),
        inputs,
        measurement: Measurement::from_report(report_path, document, scope.clone(), "saccade-report.v1".into(), "saccade-config.v1".into()).map_err(contract)?,
        scope: Scope { entries: scope, exclusions: Vec::new() },
        effective_config: BTreeMap::from([("report_config".into(), serde_json::to_value(&report.config)?)]),
        calibration: Availability::missing("no decision calibration grants approval authority"),
        validity: Validity { status: match validity.status {
            crate::meta::Validity::Valid => ValidityStatus::Valid,
            crate::meta::Validity::Invalid => ValidityStatus::Invalid,
            crate::meta::Validity::Unknown => ValidityStatus::Unknown,
        }, reasons: validity.reasons },
        facts: Vec::new(),
        intent: Availability::missing("this update binds the report; it supplies no new intent claim"),
        requests: Vec::new(), proposals: Vec::new(), human_decisions: Vec::new(), next_actions: Vec::new(),
        limits: vec!["CLI approval is an application policy and audit boundary; it does not authenticate a human with respect to an agent with shell access".into()],
        provenance: Provenance::default(),
    };
    case.refresh_id().map_err(contract)?;
    case.validate().map_err(contract)?;
    Ok(case)
}

fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), CliError> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(paths::native(path))
        .map_err(io)?;
    file.write_all(&bytes)
        .and_then(|()| file.write_all(b"\n"))
        .map_err(io)
}

/// Validate and draft or apply a reviewed baseline update without printing.
pub fn run(
    report_path: &Path,
    capture: &Path,
    baseline: &Path,
    decision_path: Option<&Path>,
    opts: Options,
) -> Result<ApprovalResult, CliError> {
    run_with_plan_observer(report_path, capture, baseline, decision_path, opts, |_| {
        Ok(())
    })
}

/// Exact plan available before applying any selected baseline changes.
pub struct ApprovalPlan<'a> {
    /// Written plan directory.
    pub out: &'a Path,
    /// Selected exact changes.
    pub entries: &'a [AppliedEntry],
    /// Whether this is a draft with no baseline mutation.
    pub dry_run: bool,
}

/// Draft or apply approval, notifying the caller after the plan is written and before mutation.
/// An observer error aborts application; this preserves CLI transport failure behavior.
pub fn run_with_plan_observer(
    report_path: &Path,
    capture: &Path,
    baseline: &Path,
    decision_path: Option<&Path>,
    opts: Options,
    mut observer: impl FnMut(&ApprovalPlan<'_>) -> Result<(), CliError>,
) -> Result<ApprovalResult, CliError> {
    for name in &opts.names {
        safe(name)?;
    }
    let report_bytes = std::fs::read(paths::native(report_path)).map_err(io)?;
    let report: Report = canonical::decode(&report_bytes).map_err(contract)?;
    if crate::report_links::original_schema(&report.schema) != crate::report::REPORT_SCHEMA {
        return Err(mismatch("approval requires saccade-report.v1"));
    }
    for (what, recorded, given) in [
        ("candidate", &report.capture_dir, capture),
        ("baseline", &report.baseline_dir, baseline),
    ] {
        let recorded = recorded
            .as_ref()
            .ok_or_else(|| mismatch(format!("report has no {what} directory")))?;
        if crate::run::normalise_path(&paths::resolve(recorded, report_path))
            != crate::run::normalise_path(given)
        {
            return Err(mismatch(format!(
                "the {what} directory is not the directory recorded in the report"
            )));
        }
    }
    let supplied = if let Some(path) = decision_path {
        let doc = Document::read(path).map_err(contract)?;
        let authority = doc
            .authority()
            .map_or("unattested".into(), |a| a.to_string());
        let Artifact::HumanDecision(d) = doc.artifact else {
            return Err(mismatch(format!(
                "approval requires an explicit canonical cli disposition; {authority} records cannot authorize CLI updates"
            )));
        };
        if d.channel != Channel::Cli || d.disposition != Disposition::Accept {
            return Err(mismatch(
                "CLI approval requires an accepting cli disposition; ties and workbench attestations cannot be promoted through CLI",
            ));
        }
        d.binding.reviewed_content.verify(path).map_err(contract)?;
        Some(*d)
    } else {
        if !opts.dry_run {
            return Err(mismatch(
                "approval requires a reviewed canonical --decisions file; first use --dry-run --out DIR. Legacy finals must be reviewed again",
            ));
        }
        None
    };
    let mut scope = supplied
        .as_ref()
        .map_or_else(|| opts.names.clone(), |d| d.binding.scope.entries.clone());
    if supplied.is_none() {
        if opts.all_failing {
            scope.extend(
                report
                    .entries
                    .iter()
                    .filter(|e| {
                        matches!(e.status, Status::Fail | Status::New)
                            || (opts.include_errors && e.status == Status::Error)
                    })
                    .map(|e| e.name.clone()),
            );
        }
        if opts.prune_missing {
            scope.extend(
                report
                    .entries
                    .iter()
                    .filter(|e| e.status == Status::Missing)
                    .map(|e| e.name.clone()),
            );
        }
        scope.sort();
        scope.dedup();
    }
    if scope.is_empty() {
        return Err(mismatch("select at least one report entry for approval"));
    }
    let source_doc = decision_path.unwrap_or(report_path);
    let case = case_for(
        report_path,
        &report,
        capture,
        baseline,
        scope,
        source_doc,
        opts.absolute,
    )?;
    let mut decision = match supplied {
        Some(d) => {
            d.validate_for(&case).map_err(contract)?;
            d
        }
        None => {
            let mut d = HumanDecision {
                decision_id: Digest::of_bytes(b""),
                binding: ReviewBinding::for_case(
                    &case,
                    ArtifactRef::from_file(report_path, source_doc, opts.absolute)
                        .map_err(contract)?,
                )
                .map_err(contract)?,
                disposition: Disposition::Accept,
                approve_deletions: opts.prune_missing,
                channel: Channel::Cli,
                exposure: ReviewerExposure {
                    mapping_access: Availability::missing("reviewer exposure was not declared"),
                    implementation_context: Availability::missing(
                        "reviewer exposure was not declared",
                    ),
                    reviewer: None,
                },
                note: "CLI update draft: review the manifest and exact report before applying"
                    .into(),
                timestamp_unix_ms: Some(now_ms()),
            };
            d.refresh_id().map_err(contract)?;
            d
        }
    };
    let selected = if opts.names.is_empty() {
        case.scope.entries.clone()
    } else {
        opts.names.clone()
    };
    let mut applied = Vec::new();
    let mut staged = Vec::new();
    // Validate all reviewed inputs (also the unselected ones) before any write.
    for name in &case.scope.entries {
        let e = entry(&report, name)?;
        super::support::check_no_symlinks(baseline, Path::new(name))?;
        super::support::check_no_symlinks(capture, Path::new(name))?;
        let before = digest(&e.baseline_sha256)?;
        let after = digest(&e.capture_sha256)?;
        checked(&baseline.join(name), &before, "baseline")?;
        let bytes = checked(&capture.join(name), &after, "candidate")?;
        if !selected.contains(name) {
            continue;
        }
        if before.is_none() && after.is_none() {
            return Err(mismatch(format!("{name}: update has no content")));
        }
        if let Some(bytes) = bytes {
            if !crate::run::is_decodable(&capture.join(name)) {
                return Err(mismatch(format!("{name}: candidate cannot be decoded")));
            }
            staged.push((name.clone(), Some(bytes)));
        } else {
            if !opts.prune_missing || !decision.approve_deletions || e.status != Status::Missing {
                return Err(mismatch(format!(
                    "{name}: deletion requires an accepting deletion decision and --prune-missing"
                )));
            }
            staged.push((name.clone(), None));
        }
        applied.push(AppliedEntry {
            entry_id: name.clone(),
            before,
            after,
        });
    }
    if selected.iter().any(|n| !case.scope.entries.contains(n)) {
        return Err(mismatch(
            "selected entry is outside the reviewed decision scope",
        ));
    }
    if applied.is_empty() {
        return Err(mismatch("no selected updates"));
    }
    // Recheck report identity before recording a plan or applying.
    if Digest::of_bytes(&std::fs::read(paths::native(report_path)).map_err(io)?)
        != Digest::of_bytes(&report_bytes)
    {
        return Err(mismatch("report changed during approval"));
    }
    let signed_mode = opts.policy.require_signed_approval
        || opts.signing.approval_record.is_some()
        || opts.signing.approver.is_some();
    let signing_record = if signed_mode {
        Some(opts.policy.draft(
            &report,
            &report_bytes,
            &decision,
            baseline,
            &applied,
            opts.signing.approver.as_deref().unwrap_or("reviewer"),
        )?)
    } else {
        None
    };
    let signed = if !opts.dry_run
        && let Some(record) = &signing_record
    {
        Some(opts.policy.authorize(&opts.signing, record)?)
    } else {
        None
    };
    let out = opts.out.unwrap_or_else(|| {
        report_path.parent().unwrap_or(Path::new(".")).join(format!(
            "approval-{}-{}",
            now_ms(),
            &decision.decision_id.as_str()[7..19]
        ))
    });
    crate::run::guard_output_dir(&out, &[capture, baseline], &[])?;
    if out.exists()
        && std::fs::read_dir(paths::native(&out))
            .map_err(io)?
            .next()
            .is_some()
    {
        return Err(CliError::new(
            "not_empty_out_dir",
            "approval --out must be empty",
        ));
    }
    std::fs::create_dir_all(paths::native(&out)).map_err(io)?;
    // Rebase provenance; canonical identities deliberately exclude path spelling.
    let output_doc = out.join("decision.json");
    decision.binding.reviewed_content.path = paths::record(
        &paths::resolve(&decision.binding.reviewed_content.path, source_doc),
        &out,
        opts.absolute,
    );
    let output_case = case_for(
        report_path,
        &report,
        capture,
        baseline,
        case.scope.entries.clone(),
        &output_doc,
        opts.absolute,
    )?;
    decision.validate_for(&output_case).map_err(contract)?;
    let receipt = ApprovalReceipt {
        decision_id: decision.decision_id.clone(),
        binding: decision.binding.clone(),
        channel: Channel::Cli,
        human_attestation: None,
        applied: applied.clone(),
        timestamp_unix_ms: now_ms(),
    };
    receipt
        .validate_for(&decision, &output_case)
        .map_err(contract)?;
    let plan = json!({"case_id":case.case_id,"decision_id":decision.decision_id,"channel":"cli","human_attestation":null,"entries":applied});
    write_json(&out.join(".saccade-run"), &"saccade-evidence.v1")?;
    write_json(
        &out.join("case.json"),
        &Document::new(Artifact::Case(Box::new(output_case))),
    )?;
    write_json(
        &output_doc,
        &Document::new(Artifact::HumanDecision(Box::new(decision))),
    )?;
    write_json(&out.join("manifest.json"), &plan)?;
    if opts.dry_run
        && let Some(record) = &signing_record
    {
        write_json(&out.join("approval.json"), record)?;
    }
    if let Some(signed) = &signed {
        write_json(&out.join("signed-approval.json"), signed)?;
    }
    // Reserve the receipt path before mutation. A dry-run never writes a receipt.
    let mut receipt_file = if opts.dry_run {
        None
    } else {
        Some(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(paths::native(&out.join("receipt.json")))
                .map_err(io)?,
        )
    };
    let mut copied = Vec::new();
    let mut pruned = Vec::new();
    observer(&ApprovalPlan {
        out: &out,
        entries: &applied,
        dry_run: opts.dry_run,
    })?;
    if !opts.dry_run {
        for (name, bytes) in &staged {
            let target = baseline.join(name);
            super::support::check_no_symlinks(baseline, Path::new(name))?;
            let e = entry(&report, name)?;
            checked(&target, &digest(&e.baseline_sha256)?, "baseline")?;
            if let Some(bytes) = bytes {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(paths::native(parent)).map_err(io)?;
                }
                let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap_or(baseline))
                    .map_err(io)?;
                temp.write_all(bytes).map_err(io)?;
                temp.persist(paths::native(&target))
                    .map_err(|e| io(e.error))?;
                copied.push(json!({"name":name,"from":paths::cwd(&capture.join(name),opts.absolute),"to":paths::cwd(&target,opts.absolute)}));
            } else {
                std::fs::remove_file(paths::native(&target)).map_err(io)?;
                pruned.push(paths::cwd(&target, opts.absolute));
            }
        }
        if let Some(signed) = &signed {
            opts.policy.persist(baseline, signed)?;
        }
        if let Some(file) = &mut receipt_file {
            let bytes = serde_json::to_vec_pretty(&Document::new(Artifact::ApprovalReceipt(
                Box::new(receipt),
            )))?;
            file.write_all(&bytes)
                .and_then(|()| file.write_all(b"\n"))
                .map_err(io)?;
        }
    }

    let mut value = super::support::base_result("approve");
    value["artifact"] = super::support::reference(&output_doc)?;
    value["review"] = json!(if opts.dry_run { "pending" } else { "accepted" });
    value["counts"] = json!({"copied":copied.len(),"pruned":pruned.len()});
    value["data"] = json!({"copied":copied.into_iter().take(5).collect::<Vec<_>>(),"pruned":pruned.into_iter().take(5).collect::<Vec<_>>(),"dry_run":opts.dry_run,"authority":"cli","human_attestation":null});
    if signed.is_some() {
        value["data"]["authority"] = json!("signed_cli");
        value["data"]["signed_approval"] =
            super::support::reference(&out.join("signed-approval.json"))?;
    }

    Ok(ApprovalResult {
        value: super::support::bounded(value, 4096)?,
        out,
        applied,
        dry_run: opts.dry_run,
        signed: signed.is_some(),
    })
}

/// Artifacts and entries produced by approval, with no process exit.
pub struct ApprovalResult {
    /// Bounded CLI-compatible result.
    pub value: serde_json::Value,
    /// Output artifact directory.
    pub out: PathBuf,
    /// Exact selected changes.
    pub applied: Vec<AppliedEntry>,
    /// Whether no baseline bytes were modified.
    pub dry_run: bool,
    /// Whether an external signature was verified and persisted.
    pub signed: bool,
}
