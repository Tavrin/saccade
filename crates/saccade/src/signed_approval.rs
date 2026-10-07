//! Detached OpenSSH approval records. Private keys never enter this process.
use crate::agent::CliError;
use saccade_core::evidence::{canonical::Digest, human::AppliedEntry};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};

pub(crate) const SCHEMA: &str = "saccade-signed-approval.v1";
pub(crate) const FILE: &str = ".saccade-approval.json";
const LIMIT: u64 = 4 * 1024 * 1024;
const RECORD_SCHEMA: &str = "saccade-approval-record.v1";
const NAMESPACE: &str = "saccade-approval";
static POLICY: OnceLock<Policy> = OnceLock::new();

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    #[serde(default)]
    require_signed_approval: bool,
    allowed_signers: Option<PathBuf>,
}

#[derive(clap::Args, Default)]
pub(crate) struct SigningArgs {
    /// Human principal listed in the external OpenSSH allowed-signers file (plan only).
    #[arg(long)]
    pub approver: Option<String>,
    /// Exact approval.json from the reviewed dry run, signed externally.
    #[arg(long)]
    pub approval_record: Option<PathBuf>,
    /// Detached OpenSSH signature produced with namespace saccade-approval.
    #[arg(long, requires = "approval_record")]
    pub approval_signature: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    schema: String,
    report_id: Digest,
    report_sha256: Digest,
    decision_id: Digest,
    approver: String,
    timestamp_unix_ms: u64,
    baseline: PathBuf,
    scope: Vec<AppliedEntry>,
    baseline_files: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Signed {
    schema: String,
    // Preserve the exact signed bytes, including whitespace, across persistence.
    record: String,
    signature: String,
}
fn error(code: &'static str, message: &str) -> CliError {
    CliError::new(code, message)
}
fn invalid() -> CliError {
    error(
        "approval_signature_invalid",
        "approval signature or signed record is invalid",
    )
}
fn read(path: &Path) -> Result<Vec<u8>, CliError> {
    let mut data = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| invalid())?
        .take(LIMIT + 1)
        .read_to_end(&mut data)
        .map_err(|_| invalid())?;
    if data.len() as u64 > LIMIT {
        return Err(invalid());
    }
    Ok(data)
}

pub(crate) fn init(required: bool, signers: Option<PathBuf>) -> Result<(), CliError> {
    let config = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|p| p.join(".config/saccade/approval-policy.json"));
    let mut policy: Policy =
        match config {
            Some(path) => match std::fs::symlink_metadata(&path) {
                Ok(_) => serde_json::from_slice(&read(&path).map_err(|_| {
                    error("approval_policy_invalid", "approval policy is unreadable")
                })?)
                .map_err(|_| error("approval_policy_invalid", "invalid approval-policy.json"))?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Policy::default(),
                Err(_) => {
                    return Err(error(
                        "approval_policy_invalid",
                        "approval policy is inaccessible",
                    ));
                }
            },
            None => Policy::default(),
        };
    if policy
        .allowed_signers
        .as_ref()
        .is_some_and(|p| !p.is_absolute())
    {
        return Err(error(
            "approval_policy_invalid",
            "user-policy allowed_signers must be absolute",
        ));
    }
    if let Some(signers) = signers {
        if policy.require_signed_approval && policy.allowed_signers.as_ref() != Some(&signers) {
            return Err(error(
                "approval_policy_invalid",
                "CLI cannot replace required user-policy signers",
            ));
        }
        policy.allowed_signers = Some(signers);
    }
    policy.require_signed_approval |= required;
    POLICY.set(policy).map_err(|_| {
        error(
            "approval_policy_invalid",
            "approval policy already initialized",
        )
    })
}
pub(crate) fn required() -> bool {
    POLICY.get().is_some_and(|p| p.require_signed_approval)
}

// Bound traversal, reject symlinks and special files, hash metadata as well as images.
pub(crate) fn inventory(root: &Path) -> Result<BTreeMap<String, String>, CliError> {
    fn walk(
        root: &Path,
        dir: &Path,
        depth: usize,
        out: &mut BTreeMap<String, String>,
    ) -> Result<(), CliError> {
        use sha2::{Digest as _, Sha256};
        if depth > 64 {
            return Err(invalid());
        }
        for entry in std::fs::read_dir(dir).map_err(|_| invalid())? {
            let entry = entry.map_err(|_| invalid())?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|_| invalid())?;
            if kind.is_symlink() {
                return Err(error("unsafe_path", "signed baseline contains a symlink"));
            }
            if path == root.join(FILE) {
                if !kind.is_file() {
                    return Err(invalid());
                }
                continue;
            }
            if kind.is_dir() {
                walk(root, &path, depth + 1, out)?;
            } else if kind.is_file() {
                if out.len() >= 10000 {
                    return Err(invalid());
                }
                let mut file = std::fs::File::open(&path)
                    .map_err(|_| invalid())?
                    .take(1024 * 1024 * 1024 + 1);
                let mut hasher = Sha256::new();
                let n = std::io::copy(&mut file, &mut hasher).map_err(|_| invalid())?;
                if n > 1024 * 1024 * 1024 {
                    return Err(invalid());
                }
                let relative = path.strip_prefix(root).map_err(|_| invalid())?;
                let parts = relative
                    .components()
                    .map(|part| {
                        let name = part.as_os_str().to_str().ok_or_else(invalid)?;
                        if name.contains(['\\', ':']) {
                            return Err(error("unsafe_path", "ambiguous signed baseline filename"));
                        }
                        Ok(name)
                    })
                    .collect::<Result<Vec<_>, CliError>>()?;
                let name = parts.join("/");
                out.insert(name, format!("sha256:{:x}", hasher.finalize()));
            } else {
                return Err(invalid());
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    walk(root, root, 0, &mut files)?;
    Ok(files)
}

pub(crate) fn draft(
    report: &saccade_core::Report,
    report_bytes: &[u8],
    decision: &saccade_core::evidence::human::HumanDecision,
    baseline: &Path,
    scope: &[AppliedEntry],
    approver: &str,
) -> Result<Record, CliError> {
    let mut files = inventory(baseline)?;
    for entry in scope {
        if let Some(hash) = &entry.after {
            files.insert(entry.entry_id.clone(), hash.as_str().to_owned());
        } else {
            files.remove(&entry.entry_id);
        }
    }
    Ok(Record {
        schema: RECORD_SCHEMA.into(),
        report_id: saccade_core::evidence::case::Measurement::report_identity(report)
            .map_err(|_| invalid())?,
        report_sha256: Digest::of_bytes(report_bytes),
        decision_id: decision.decision_id.clone(),
        approver: approver.into(),
        timestamp_unix_ms: decision.timestamp_unix_ms.ok_or_else(invalid)?,
        baseline: baseline.canonicalize().map_err(|_| invalid())?,
        scope: scope.to_vec(),
        baseline_files: files,
    })
}

fn verify(signed: &Signed) -> Result<Record, CliError> {
    if signed.schema != SCHEMA {
        return Err(invalid());
    }
    let record: Record = serde_json::from_str(&signed.record).map_err(|_| invalid())?;
    if record.schema != RECORD_SCHEMA
        || record.approver.is_empty()
        || record.approver.len() > 256
        || record.approver.chars().any(char::is_control)
        || record.scope.is_empty()
    {
        return Err(invalid());
    }
    let signers = POLICY
        .get()
        .and_then(|p| p.allowed_signers.as_deref())
        .ok_or_else(|| {
            error(
                "approval_policy_invalid",
                "signed approval requires an external allowed-signers file",
            )
        })?;
    let temp = tempfile::tempdir().map_err(|_| invalid())?;
    let sig = temp.path().join("signature");
    let allowed = temp.path().join("allowed_signers");
    let payload = temp.path().join("record");
    std::fs::write(&sig, &signed.signature).map_err(|_| invalid())?;
    std::fs::write(&allowed, read(signers)?).map_err(|_| invalid())?;
    std::fs::write(&payload, &signed.record).map_err(|_| invalid())?;
    let input = std::fs::File::open(payload).map_err(|_| invalid())?;
    let mut child = Command::new("ssh-keygen")
        .args(["-Y", "verify", "-f"])
        .arg(allowed)
        .args(["-I", &record.approver, "-n", NAMESPACE, "-s"])
        .arg(sig)
        .stdin(Stdio::from(input))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            error(
                "approval_verifier_unavailable",
                "OpenSSH ssh-keygen verifier unavailable",
            )
        })?;
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|_| invalid())? {
            return if status.success() {
                Ok(record)
            } else {
                Err(invalid())
            };
        }
        if started.elapsed() > std::time::Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error(
                "approval_verifier_unavailable",
                "OpenSSH verification timed out",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub(crate) fn authorize(args: &SigningArgs, expected: &Record) -> Result<Signed, CliError> {
    let (Some(record), Some(signature)) = (&args.approval_record, &args.approval_signature) else {
        return Err(error(
            "approval_signature_required",
            "supply an externally signed --approval-record and --approval-signature",
        ));
    };
    let signed = Signed {
        schema: SCHEMA.into(),
        record: String::from_utf8(read(record)?).map_err(|_| invalid())?,
        signature: String::from_utf8(read(signature)?).map_err(|_| invalid())?,
    };
    let actual = verify(&signed)?;
    let mut expected_value = serde_json::to_value(expected)?;
    // The signer principal belongs to the reviewed record, not a CLI override.
    expected_value["approver"] = serde_json::json!(actual.approver);
    if serde_json::to_value(actual)? != expected_value {
        return Err(error(
            "approval_content_mismatch",
            "signed approval does not match this report, decision, destination or scope",
        ));
    }
    Ok(signed)
}

pub(crate) fn persist(baseline: &Path, signed: &Signed) -> Result<(), CliError> {
    let record: Record = serde_json::from_str(&signed.record).map_err(|_| invalid())?;
    if inventory(baseline)? != record.baseline_files {
        return Err(error(
            "approval_content_mismatch",
            "baseline changed during approval",
        ));
    }
    let mut file = tempfile::NamedTempFile::new_in(baseline).map_err(|_| invalid())?;
    file.write_all(&serde_json::to_vec_pretty(signed)?)
        .map_err(|_| invalid())?;
    file.persist(baseline.join(FILE)).map_err(|_| invalid())?;
    Ok(())
}
pub(crate) fn check_baseline(baseline: &Path) -> Result<(), CliError> {
    check_anchor(&baseline.join(FILE), Some(baseline))
}
pub(crate) fn check_anchor(path: &Path, baseline: Option<&Path>) -> Result<(), CliError> {
    if !path.exists() {
        return Err(error(
            "approval_signature_required",
            "baseline has no signed approval",
        ));
    }
    let signed: Signed = serde_json::from_slice(&read(path)?).map_err(|_| invalid())?;
    let record = verify(&signed)?;
    let root = baseline.unwrap_or(&record.baseline);
    if root.canonicalize().map_err(|_| invalid())? != record.baseline
        || inventory(root)? != record.baseline_files
    {
        return Err(error(
            "approval_content_mismatch",
            "baseline contents differ from the signed approval",
        ));
    }
    Ok(())
}

pub(crate) fn check_manifest(target: &Path) -> Result<(), CliError> {
    let file = if target.is_dir() {
        target.join("saccade-manifest.json")
    } else {
        target.to_path_buf()
    };
    let value: serde_json::Value = serde_json::from_slice(&read(&file)?).map_err(|_| invalid())?;
    check_manifest_value(&file, &value)
}
fn check_manifest_value(file: &Path, value: &serde_json::Value) -> Result<(), CliError> {
    if value["schema"].as_str().is_some_and(|s| {
        (s.starts_with("saccade-manifest.") || s.starts_with("saccade-link."))
            && s != "saccade-manifest.v1"
            && s != "saccade-link.v1"
    }) {
        return Err(error(
            "version_skew",
            "unsupported approval consumer schema",
        ));
    }
    if value["schema"] == "saccade-link.v1" {
        let manifest = value["manifest"].as_str().ok_or_else(invalid)?;
        let path = saccade_core::paths::resolve(manifest, file);
        let linked: serde_json::Value =
            serde_json::from_slice(&read(&path)?).map_err(|_| invalid())?;
        if linked["schema"] != "saccade-manifest.v1" {
            return Err(invalid());
        }
        return check_manifest_value(&path, &linked);
    }
    if value["schema"] == "saccade-manifest.v1" {
        if let Some(path) = value["approval"]["approved_anchor"]["path"].as_str() {
            check_anchor(&saccade_core::paths::resolve(path, file), None)?;
        } else if value["approval"]["state"] != "none"
            || !value["approval"]["approved_anchor"].is_null()
        {
            return Err(error(
                "approval_signature_required",
                "manifest has no signed anchor",
            ));
        }
    }
    Ok(())
}
