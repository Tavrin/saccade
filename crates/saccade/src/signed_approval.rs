//! Detached OpenSSH approval records. Private keys never enter this process.
use crate::agent::CliError;
use saccade_core::evidence::{canonical::Digest, human::AppliedEntry};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Seek, Write},
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    #[serde(default)]
    require_signed_approval: bool,
    allowed_signers: Option<PathBuf>,
    #[serde(default = "default_verifier")]
    verifier: PathBuf,
    max_approval_age_seconds: Option<u64>,
    ledger: Option<PathBuf>,
}
fn default_verifier() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(r"C:\Windows\System32\OpenSSH\ssh-keygen.exe")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/usr/bin/ssh-keygen")
    }
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            require_signed_approval: false,
            allowed_signers: None,
            verifier: default_verifier(),
            max_approval_age_seconds: None,
            ledger: None,
        }
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sequence: Option<u64>,
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

// Fixed system policy is read before HOME. No environment/CLI override exists.
#[cfg(not(windows))]
const SYSTEM_POLICY: &str = "/etc/saccade/approval-policy.json";
#[cfg(windows)]
const SYSTEM_POLICY: &str = r"C:\ProgramData\saccadepproval-policy.json";

fn trust_error() -> CliError {
    error(
        "approval_trust_unsafe",
        "approval trust file ownership, permissions or platform is unsafe",
    )
}
fn trusted_open(path: &Path, writable: bool, root_only: bool) -> Result<std::fs::File, CliError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let mut options = std::fs::OpenOptions::new();
        options
            .read(true)
            .write(writable)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let file = options.open(path).map_err(|_| trust_error())?;
        let meta = file.metadata().map_err(|_| trust_error())?;
        let uid = rustix::process::geteuid().as_raw();
        if !meta.is_file()
            || meta.mode() & 0o022 != 0
            || (meta.uid() != 0 && (root_only || meta.uid() != uid))
        {
            return Err(trust_error());
        }
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        let _ = (path, writable, root_only);
        Err(trust_error())
    }
}
fn trust_bytes(path: &Path, root_only: bool) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    trusted_open(path, false, root_only)?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| trust_error())?;
    if bytes.len() as u64 > LIMIT {
        return Err(trust_error());
    }
    Ok(bytes)
}
fn load_policy(system: &Path, home: Option<&Path>) -> Result<Policy, CliError> {
    let user = home.map(|p| p.join(".config/saccade/approval-policy.json"));
    for (path, root_only) in
        std::iter::once((system, true)).chain(user.as_deref().map(|p| (p, false)))
    {
        match std::fs::symlink_metadata(path) {
            Ok(_) => {
                return serde_json::from_slice(&trust_bytes(path, root_only)?)
                    .map_err(|_| error("approval_policy_invalid", "invalid approval-policy.json"));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(error(
                    "approval_policy_invalid",
                    "approval policy is inaccessible",
                ));
            }
        }
    }
    Ok(Policy::default())
}
pub(crate) fn init(required: bool, signers: Option<PathBuf>) -> Result<(), CliError> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut policy = load_policy(Path::new(SYSTEM_POLICY), home.as_deref())?;
    if policy
        .allowed_signers
        .as_ref()
        .is_some_and(|p| !p.is_absolute())
        || !policy.verifier.is_absolute()
        || policy.ledger.as_ref().is_some_and(|p| !p.is_absolute())
    {
        return Err(error(
            "approval_policy_invalid",
            "policy trust paths must be absolute",
        ));
    }
    if let Some(signers) = signers {
        let signers = if signers.is_absolute() {
            signers
        } else {
            std::env::current_dir()
                .map_err(|_| trust_error())?
                .join(signers)
        };
        if policy.require_signed_approval && policy.allowed_signers.as_ref() != Some(&signers) {
            return Err(error(
                "approval_policy_invalid",
                "CLI cannot replace required policy signers",
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
    })?;
    if self::required() {
        saccade_core::run::set_approval_guard(|baseline| {
            check_baseline(baseline).map_err(|e| saccade_core::Error::ApprovalRefused {
                code: e.code,
                message: e.message,
            })
        })?;
    }
    Ok(())
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
        sequence: next_sequence(baseline)?,
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
    std::fs::write(&allowed, trust_bytes(signers, false)?).map_err(|_| invalid())?;
    std::fs::write(&payload, &signed.record).map_err(|_| invalid())?;
    let input = std::fs::File::open(payload).map_err(|_| invalid())?;
    let policy = POLICY.get().ok_or_else(invalid)?;
    let _verifier = trusted_open(&policy.verifier, false, false)?;
    let mut child = Command::new(&policy.verifier)
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
                check_age(&record, policy)?;
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
    check_next(&actual_record(&signed)?)?;
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
    advance_ledger(&record, &signed.record)?;
    file.persist(baseline.join(FILE)).map_err(|_| invalid())?;
    Ok(())
}
pub(crate) fn check_baseline(baseline: &Path) -> Result<BTreeMap<String, String>, CliError> {
    checked_record(&baseline.join(FILE), Some(baseline)).map(|r| r.baseline_files)
}
pub(crate) fn check_anchor(path: &Path, baseline: Option<&Path>) -> Result<(), CliError> {
    checked_record(path, baseline).map(|_| ())
}
fn checked_record(path: &Path, baseline: Option<&Path>) -> Result<Record, CliError> {
    if !path.exists() {
        return Err(error(
            "approval_signature_required",
            "baseline has no signed approval",
        ));
    }
    let signed: Signed = serde_json::from_slice(&read(path)?).map_err(|_| invalid())?;
    let record = verify(&signed)?;
    check_current(&record, &signed.record)?;
    let root = baseline.unwrap_or(&record.baseline);
    if root.canonicalize().map_err(|_| invalid())? != record.baseline
        || inventory(root)? != record.baseline_files
    {
        return Err(error(
            "approval_content_mismatch",
            "baseline contents differ from the signed approval",
        ));
    }
    Ok(record)
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
            && s != "saccade-manifest.v2"
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
        if linked["schema"] != "saccade-manifest.v1" && linked["schema"] != "saccade-manifest.v2" {
            return Err(invalid());
        }
        return check_manifest_value(&path, &linked);
    }
    if value["schema"] == "saccade-manifest.v2" {
        check_case_anchors(file, &value["cases"])?;
    }
    if value["schema"] == "saccade-manifest.v1" || value["schema"] == "saccade-manifest.v2" {
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

fn actual_record(signed: &Signed) -> Result<Record, CliError> {
    serde_json::from_str(&signed.record).map_err(|_| invalid())
}
fn check_age(record: &Record, policy: &Policy) -> Result<(), CliError> {
    if let Some(max_age) = policy.max_approval_age_seconds {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| invalid())?
            .as_millis();
        let stamp = u128::from(record.timestamp_unix_ms);
        if stamp > now || now - stamp > u128::from(max_age) * 1000 {
            return Err(error(
                "approval_expired",
                "approval timestamp is outside policy age limit",
            ));
        }
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerEntry {
    sequence: u64,
    record_sha256: Digest,
}
type Ledger = BTreeMap<PathBuf, LedgerEntry>;
fn with_ledger<T>(
    operation: impl FnOnce(&mut Ledger) -> Result<T, CliError>,
    write: bool,
) -> Result<Option<T>, CliError> {
    let Some(path) = POLICY.get().and_then(|p| p.ledger.as_deref()) else {
        return Ok(None);
    };
    let mut file = trusted_open(path, write, false)?;
    if write {
        fs2::FileExt::lock_exclusive(&file)
    } else {
        fs2::FileExt::lock_shared(&file)
    }
    .map_err(|_| trust_error())?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| trust_error())?;
    if bytes.len() as u64 > LIMIT {
        return Err(trust_error());
    }
    let mut ledger: Ledger = serde_json::from_slice(&bytes).map_err(|_| {
        error(
            "approval_ledger_invalid",
            "trusted approval ledger is invalid",
        )
    })?;
    let result = operation(&mut ledger)?;
    if write {
        let bytes = serde_json::to_vec(&ledger)?;
        if bytes.len() as u64 > LIMIT {
            return Err(error("approval_ledger_invalid", "trusted ledger is full"));
        }
        file.rewind().map_err(|_| trust_error())?;
        file.set_len(0).map_err(|_| trust_error())?;
        file.write_all(&bytes).map_err(|_| trust_error())?;
        file.sync_all().map_err(|_| trust_error())?;
    }
    Ok(Some(result))
}
fn next_sequence(baseline: &Path) -> Result<Option<u64>, CliError> {
    let destination = baseline.canonicalize().map_err(|_| invalid())?;
    with_ledger(
        |ledger| {
            ledger
                .get(&destination)
                .map_or(Some(1), |e| e.sequence.checked_add(1))
                .ok_or_else(|| error("approval_ledger_invalid", "approval sequence exhausted"))
        },
        false,
    )
}
fn replay() -> CliError {
    error(
        "approval_replayed",
        "approval is not the current trusted revision",
    )
}
fn require_next(record: &Record, ledger: &Ledger) -> Result<(), CliError> {
    let next = ledger
        .get(&record.baseline)
        .map_or(Some(1), |e| e.sequence.checked_add(1));
    if record.sequence != next || next.is_none() {
        return Err(replay());
    }
    Ok(())
}
fn check_next(record: &Record) -> Result<(), CliError> {
    with_ledger(|ledger| require_next(record, ledger), false).map(|_| ())
}
fn advance_ledger(record: &Record, bytes: &str) -> Result<(), CliError> {
    with_ledger(
        |ledger| {
            require_next(record, ledger)?;
            ledger.insert(
                record.baseline.clone(),
                LedgerEntry {
                    sequence: record.sequence.ok_or_else(replay)?,
                    record_sha256: Digest::of_bytes(bytes.as_bytes()),
                },
            );
            Ok(())
        },
        true,
    )
    .map(|_| ())
}
fn check_current(record: &Record, bytes: &str) -> Result<(), CliError> {
    with_ledger(
        |ledger| {
            let entry = ledger.get(&record.baseline).ok_or_else(replay)?;
            if record.sequence != Some(entry.sequence)
                || entry.record_sha256 != Digest::of_bytes(bytes.as_bytes())
            {
                return Err(replay());
            }
            Ok(())
        },
        false,
    )
    .map(|_| ())
}

/// All stock measurement consumers share enforcement; callers may retain a precheck
/// inventory across configuration loading for deterministic replacement refusal.
pub(crate) fn run(
    baseline: &Path,
    capture: &Path,
    out: &Path,
    config: &saccade_core::config::RunConfig,
    verified: Option<&BTreeMap<String, String>>,
) -> Result<saccade_core::Report, CliError> {
    let inventory = if required() && verified.is_none() {
        Some(check_baseline(baseline)?)
    } else {
        None
    };
    if let Some(files) = verified.or(inventory.as_ref()) {
        saccade_core::run::run_approved(baseline, capture, out, config, files).map_err(|e| {
            if matches!(&e, saccade_core::Error::ApprovalContentMismatch) {
                error(
                    "approval_content_mismatch",
                    "baseline changed between verification and measurement",
                )
            } else {
                e.into()
            }
        })
    } else {
        Ok(saccade_core::run::run(baseline, capture, out, config)?)
    }
}

pub(crate) fn check_case_anchors(file: &Path, cases: &serde_json::Value) -> Result<(), CliError> {
    for case in cases.as_array().ok_or_else(invalid)? {
        let anchor = &case["approved_anchor"];
        if anchor.is_null() {
            continue;
        }
        let path = saccade_core::paths::resolve(anchor["path"].as_str().ok_or_else(invalid)?, file);
        let path = path.canonicalize().map_err(|_| invalid())?;
        let root = path
            .ancestors()
            .skip(1)
            .take(64)
            .find(|root| root.join(FILE).is_file())
            .ok_or_else(|| {
                error(
                    "approval_signature_required",
                    "declared case anchor has no signed baseline",
                )
            })?;
        let files = check_baseline(root)?;
        let name = saccade_core::paths::portable(path.strip_prefix(root).map_err(|_| invalid())?);
        let expected = format!("sha256:{}", anchor["sha256"].as_str().ok_or_else(invalid)?);
        if files.get(&name) != Some(&expected) {
            return Err(error(
                "approval_content_mismatch",
                "case anchor hash differs from its signed inventory",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn system_policy_cannot_be_hidden_by_home() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().expect("fixture");
        let system = temp.path().join("system.json");
        std::fs::write(&system, br#"{"require_signed_approval":true}"#).expect("policy");
        std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o600)).expect("mode");
        let home = temp.path().join("other-home");
        match load_policy(&system, Some(&home)) {
            Ok(policy) => assert!(policy.require_signed_approval),
            // Non-root tests cannot provision a root-owned system policy. Refusal
            // still proves HOME cannot cause fallback to an unauthenticated policy.
            Err(error) => assert_eq!(error.code, "approval_trust_unsafe"),
        }
        std::fs::set_permissions(&system, std::fs::Permissions::from_mode(0o666)).expect("mode");
        assert!(matches!(load_policy(&system, None), Err(e) if e.code == "approval_trust_unsafe"));
    }
    #[test]
    #[cfg(windows)]
    fn windows_policy_off_defaults_use_absolute_paths() {
        assert!(default_verifier().is_absolute());
        assert!(Path::new(SYSTEM_POLICY).is_absolute());
        assert!(!Policy::default().require_signed_approval);
    }
    #[test]
    #[cfg(not(unix))]
    fn platforms_without_unix_trust_checks_refuse() {
        assert!(
            matches!(trusted_open(Path::new("trust"), false, false), Err(e) if e.code == "approval_trust_unsafe")
        );
    }
}
