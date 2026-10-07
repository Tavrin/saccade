//! Detached OpenSSH approval records. Private keys never enter this process.
use super::CommandError as CliError;
use crate::evidence::{canonical::Digest, human::AppliedEntry};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Signed-approval envelope contract identifier.
pub const SCHEMA: &str = "saccade-signed-approval.v1";
/// Signed record file stored at the authenticated baseline root.
pub const FILE: &str = ".saccade-approval.json";
const LIMIT: u64 = 4 * 1024 * 1024;
const RECORD_SCHEMA: &str = "saccade-approval-record.v1";
const NAMESPACE: &str = "saccade-approval";

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
/// Independent approval trust policy. Defaults off; trust files are checked on use.
pub struct Policy {
    #[serde(default)]
    /// Require signed approval.
    pub require_signed_approval: bool,
    /// Allowed signers.
    pub allowed_signers: Option<PathBuf>,
    #[serde(default = "default_verifier")]
    /// Verifier.
    pub verifier: PathBuf,
    /// Max approval age seconds.
    pub max_approval_age_seconds: Option<u64>,
    /// Ledger.
    pub ledger: Option<PathBuf>,
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

#[derive(Default, Clone)]
/// Public record/signature paths and optional signer principal; never private keys.
pub struct SigningArgs {
    /// Human principal listed in the external OpenSSH allowed-signers file (plan only).
    pub approver: Option<String>,
    /// Exact approval.json from the reviewed dry run, signed externally.
    pub approval_record: Option<PathBuf>,
    /// Detached OpenSSH signature produced with namespace saccade-approval.
    pub approval_signature: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Exact reviewed content, destination, scope and optional monotonic revision.
pub struct Record {
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
/// Externally signed record; retains exact payload bytes including whitespace.
pub struct Signed {
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

// System policy is read before HOME. Unix locations cannot be overridden.

// Compare directory aliases without following a final signer-file symlink.
// Keep the original paths for O_NOFOLLOW validation at verification time.
fn same_signers(left: &Path, right: &Path) -> bool {
    left == right
        || (left.file_name().is_some()
            && left.file_name() == right.file_name()
            && left
                .parent()
                .and_then(|p| p.canonicalize().ok())
                .is_some_and(|parent| {
                    right.parent().and_then(|p| p.canonicalize().ok()).as_ref() == Some(&parent)
                }))
}

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

// Bound traversal, reject symlinks and special files, hash metadata as well as images.
/// Hash a bounded-depth baseline inventory, rejecting symlinks and special files.
pub fn inventory(root: &Path) -> Result<BTreeMap<String, String>, CliError> {
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

/// All stock measurement consumers share enforcement; callers may retain a precheck
/// inventory across configuration loading for deterministic replacement refusal.
impl Policy {
    /// Load trusted system policy before user policy and apply monotonic enablement.
    /// Relative CLI signer paths resolve against `cwd`; required policy signers cannot be replaced.
    pub fn load(
        system: &Path,
        home: Option<&Path>,
        required: bool,
        signers: Option<PathBuf>,
        cwd: &Path,
    ) -> Result<Self, CliError> {
        let mut policy = load_policy(system, home)?;
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
                cwd.join(signers)
            };
            if policy.require_signed_approval
                && !policy
                    .allowed_signers
                    .as_deref()
                    .is_some_and(|p| same_signers(p, &signers))
            {
                return Err(error(
                    "approval_policy_invalid",
                    "CLI cannot replace required policy signers",
                ));
            }
            if !policy.require_signed_approval {
                policy.allowed_signers = Some(signers);
            }
        }
        policy.require_signed_approval |= required;
        #[cfg(not(unix))]
        if policy.require_signed_approval || policy.allowed_signers.is_some() {
            // Refuse enablement at startup, before any command can claim enforcement.
            return Err(trust_error());
        }
        Ok(policy)
    }
    /// Bind a reviewed decision and proposed baseline inventory into a signable record.
    pub fn draft(
        &self,
        report: &crate::Report,
        report_bytes: &[u8],
        decision: &crate::evidence::human::HumanDecision,
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
            report_id: crate::evidence::case::Measurement::report_identity(report)
                .map_err(|_| invalid())?,
            report_sha256: Digest::of_bytes(report_bytes),
            decision_id: decision.decision_id.clone(),
            approver: approver.into(),
            timestamp_unix_ms: decision.timestamp_unix_ms.ok_or_else(invalid)?,
            baseline: baseline.canonicalize().map_err(|_| invalid())?,
            scope: scope.to_vec(),
            baseline_files: files,
            sequence: self.next_sequence(baseline)?,
        })
    }
    /// Verify an external OpenSSH signature and policy age limit without verifier diagnostics.
    /// Baseline contents and ledger revision are checked by `check_baseline`/`check_anchor`.
    pub fn verify(&self, signed: &Signed) -> Result<Record, CliError> {
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
        let signers = self.allowed_signers.as_deref().ok_or_else(|| {
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
        let policy = self;
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
    /// Verify the exact externally signed draft against expected content and next ledger revision.
    pub fn authorize(&self, args: &SigningArgs, expected: &Record) -> Result<Signed, CliError> {
        let (Some(record), Some(signature)) = (&args.approval_record, &args.approval_signature)
        else {
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
        let actual = self.verify(&signed)?;
        let mut expected_value = serde_json::to_value(expected)?;
        // The signer principal belongs to the reviewed record, not a CLI override.
        expected_value["approver"] = serde_json::json!(actual.approver);
        if serde_json::to_value(actual)? != expected_value {
            return Err(error(
                "approval_content_mismatch",
                "signed approval does not match this report, decision, destination or scope",
            ));
        }
        self.check_next(&actual_record(&signed)?)?;
        Ok(signed)
    }
    /// Persist.
    pub(crate) fn persist(&self, baseline: &Path, signed: &Signed) -> Result<(), CliError> {
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
        self.advance_ledger(&record, &signed.record)?;
        file.persist(baseline.join(FILE)).map_err(|_| invalid())?;
        Ok(())
    }
    /// Authenticate the baseline root, current ledger revision and full content inventory.
    pub fn check_baseline(&self, baseline: &Path) -> Result<BTreeMap<String, String>, CliError> {
        self.checked_record(&baseline.join(FILE), Some(baseline))
            .map(|r| r.baseline_files)
    }
    /// Authenticate an anchor record and the baseline it binds.
    pub fn check_anchor(&self, path: &Path, baseline: Option<&Path>) -> Result<(), CliError> {
        self.checked_record(path, baseline).map(|_| ())
    }
    fn checked_record(&self, path: &Path, baseline: Option<&Path>) -> Result<Record, CliError> {
        if !path.exists() {
            return Err(error(
                "approval_signature_required",
                "baseline has no signed approval",
            ));
        }
        let signed: Signed = serde_json::from_slice(&read(path)?).map_err(|_| invalid())?;
        let record = self.verify(&signed)?;
        self.check_current(&record, &signed.record)?;
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
    /// Verify signed anchors referenced by a manifest or manifest link.
    pub fn check_manifest(&self, target: &Path) -> Result<(), CliError> {
        let file = if target.is_dir() {
            target.join("saccade-manifest.json")
        } else {
            target.to_path_buf()
        };
        let value: serde_json::Value =
            serde_json::from_slice(&read(&file)?).map_err(|_| invalid())?;
        self.check_manifest_value(&file, &value)
    }
    fn check_manifest_value(&self, file: &Path, value: &serde_json::Value) -> Result<(), CliError> {
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
            let path = crate::paths::resolve(manifest, file);
            let linked: serde_json::Value =
                serde_json::from_slice(&read(&path)?).map_err(|_| invalid())?;
            if linked["schema"] != "saccade-manifest.v1"
                && linked["schema"] != "saccade-manifest.v2"
            {
                return Err(invalid());
            }
            return self.check_manifest_value(&path, &linked);
        }
        if value["schema"] == "saccade-manifest.v2" {
            self.check_case_anchors(file, &value["cases"])?;
        }
        if value["schema"] == "saccade-manifest.v1" || value["schema"] == "saccade-manifest.v2" {
            if let Some(path) = value["approval"]["approved_anchor"]["path"].as_str() {
                self.check_anchor(&crate::paths::resolve(path, file), None)?;
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
    fn with_ledger<T>(
        &self,
        operation: impl FnOnce(&mut Ledger) -> Result<T, CliError>,
        write: bool,
    ) -> Result<Option<T>, CliError> {
        let Some(path) = self.ledger.as_deref() else {
            return Ok(None);
        };
        let mut file = trusted_open(path, write, false)?;
        #[cfg(any(unix, windows))]
        if write {
            fs2::FileExt::lock_exclusive(&file)
        } else {
            fs2::FileExt::lock_shared(&file)
        }
        .map_err(|_| trust_error())?;
        #[cfg(not(any(unix, windows)))]
        return Err(trust_error());
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
    fn next_sequence(&self, baseline: &Path) -> Result<Option<u64>, CliError> {
        let destination = baseline.canonicalize().map_err(|_| invalid())?;
        self.with_ledger(
            |ledger| {
                ledger
                    .get(&destination)
                    .map_or(Some(1), |e| e.sequence.checked_add(1))
                    .ok_or_else(|| error("approval_ledger_invalid", "approval sequence exhausted"))
            },
            false,
        )
    }
    fn check_next(&self, record: &Record) -> Result<(), CliError> {
        self.with_ledger(|ledger| require_next(record, ledger), false)
            .map(|_| ())
    }
    fn advance_ledger(&self, record: &Record, bytes: &str) -> Result<(), CliError> {
        self.with_ledger(
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
    fn check_current(&self, record: &Record, bytes: &str) -> Result<(), CliError> {
        self.with_ledger(
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
    /// Run.
    pub fn run(
        &self,
        baseline: &Path,
        capture: &Path,
        out: &Path,
        config: &crate::config::RunConfig,
        verified: Option<&BTreeMap<String, String>>,
    ) -> Result<crate::Report, CliError> {
        let inventory = if self.require_signed_approval && verified.is_none() {
            Some(self.check_baseline(baseline)?)
        } else {
            None
        };
        if let Some(files) = verified.or(inventory.as_ref()) {
            crate::run::run_scoped(baseline, capture, out, config, Some(files)).map_err(|e| {
                if matches!(&e, crate::Error::ApprovalContentMismatch) {
                    error(
                        "approval_content_mismatch",
                        "baseline changed between verification and measurement",
                    )
                } else {
                    e.into()
                }
            })
        } else {
            Ok(crate::run::run_scoped(
                baseline, capture, out, config, None,
            )?)
        }
    }
    /// Verify each declared case anchor against its authenticated full baseline inventory.
    pub fn check_case_anchors(
        &self,
        file: &Path,
        cases: &serde_json::Value,
    ) -> Result<(), CliError> {
        for case in cases.as_array().ok_or_else(invalid)? {
            let anchor = &case["approved_anchor"];
            if anchor.is_null() {
                continue;
            }
            let path = crate::paths::resolve(anchor["path"].as_str().ok_or_else(invalid)?, file);
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
            let files = self.check_baseline(root)?;
            let name = crate::paths::portable(path.strip_prefix(root).map_err(|_| invalid())?);
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
}
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn absent_policy_defaults_off() {
        let temp = tempfile::tempdir().expect("fixture");
        let policy = load_policy(&temp.path().join("absent/system.json"), Some(temp.path()))
            .expect("no policy");
        assert!(!policy.require_signed_approval);
        assert!(policy.allowed_signers.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn directory_aliases_match_but_final_symlinks_stay_unsafe() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("fixture");
        let real = temp.path().join("real");
        let alias = temp.path().join("alias");
        std::fs::create_dir(&real).expect("directory");
        symlink(&real, &alias).expect("directory alias");
        std::fs::write(real.join("signers"), b"signers").expect("trust file");
        assert!(same_signers(&real.join("signers"), &alias.join("signers")));
        assert!(!same_signers(&real.join("signers"), &alias.join("other")));
        symlink(real.join("signers"), real.join("link")).expect("file alias");
        assert!(!same_signers(&real.join("signers"), &real.join("link")));
        assert!(matches!(trusted_open(&alias.join("link"), false, false),
            Err(e) if e.code == "approval_trust_unsafe"));
    }

    #[test]
    #[cfg(unix)]
    fn configured_user_policy_fails_closed() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let temp = tempfile::tempdir().expect("fixture");
        let system = temp.path().join("absent/system.json");
        let directory = temp.path().join(".config/saccade");
        std::fs::create_dir_all(&directory).expect("config");
        let policy = directory.join("approval-policy.json");
        std::fs::write(&policy, b"{}").expect("policy");
        for mode in [0o666, 0o000] {
            std::fs::set_permissions(&policy, std::fs::Permissions::from_mode(mode)).expect("mode");
            // A root process can still read mode 000; exercise unreadability as a user.
            if mode != 0 || rustix::process::geteuid().as_raw() != 0 {
                assert!(matches!(load_policy(&system, Some(temp.path())),
                    Err(e) if e.code == "approval_trust_unsafe"));
            }
        }
        std::fs::remove_file(&policy).expect("remove");
        symlink(directory.join("missing"), &policy).expect("dangling policy");
        assert!(matches!(load_policy(&system, Some(temp.path())),
            Err(e) if e.code == "approval_trust_unsafe"));
        std::fs::remove_file(&policy).expect("remove");
        std::fs::write(&policy, b"invalid json").expect("policy");
        std::fs::set_permissions(&policy, std::fs::Permissions::from_mode(0o600)).expect("mode");
        assert!(matches!(load_policy(&system, Some(temp.path())),
            Err(e) if e.code == "approval_policy_invalid"));
    }

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
        let path = Policy::default().verifier;
        assert!(path.is_absolute());
        assert_eq!(
            path.file_name(),
            Some(std::ffi::OsStr::new("ssh-keygen.exe"))
        );
        assert!(!path.to_string_lossy().chars().any(char::is_control));
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
