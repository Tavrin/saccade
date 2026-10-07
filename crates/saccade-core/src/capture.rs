//! Offline conformance of producer capture receipts and their bound image bytes.
use crate::arms;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Component, Path},
};

/// Capture receipt contract; future incompatible changes use a successor version.
pub const RECORD_SCHEMA: &str = "saccade-capture-record.v1";
/// Conformance result contract, independent of pixel or performance acceptance.
pub const RESULT_SCHEMA: &str = "saccade-capture-conformance.v1";
pub(super) const RECORD_LIMIT: u64 = 1024 * 1024;
pub(super) const IMAGE_LIMIT: u64 = 64 * 1024 * 1024;
pub(super) const ITEM_LIMIT: usize = 256;

/// Generic adapter family, not an assertion that the producer was executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Producer {
    /// Externally rendered image and sidecar.
    Renderer,
    /// Externally acquired browser screenshot and receipt.
    Browser,
}
/// Explicit completion state; partial runs cannot conform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Completion {
    /// Producer reports all planned attempts were recorded.
    Complete,
    /// Producer reports an interrupted or incomplete run.
    Partial,
}
/// Acquisition state; failed and skipped slots must remain in the receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Image acquisition succeeded.
    Captured,
    /// Attempt failed, even if an old image still exists.
    Failed,
    /// Planned acquisition was not attempted.
    Skipped,
}
/// A planned slot, retained separately from the producer's observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Unique slot identifier.
    pub id: String,
    /// Exact planned capture settings, including any required stabilization.
    pub settings: BTreeMap<String, Value>,
    /// Planned executable, prepared input, build and readiness identities.
    #[cfg_attr(feature = "schema", schemars(with = "arms::Fingerprint"))]
    pub fingerprint: Value,
    /// Declared timing domain, for example monotonic or a frozen browser clock.
    pub clock_domain: String,
}
/// Bytes attributed to an acquisition; paths resolve beside the receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Image {
    /// Relative file path; traversal and symlinks are refused.
    pub path: String,
    /// Lowercase, full SHA-256 of encoded image bytes.
    pub sha256: String,
}
/// Timing provenance supplied by the producer, not measured by conformance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Clock {
    /// Clock domain used for both observations.
    pub domain: String,
    /// Run identity associating timestamps with this receipt.
    pub run_id: String,
    /// Acquisition start in nanoseconds in the declared clock domain.
    pub start_ns: u64,
    /// Acquisition end in nanoseconds in the same clock domain.
    pub end_ns: u64,
}
/// One acquisition, including unavailable evidence on failed attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Acquisition {
    /// Planned slot identifier.
    pub id: String,
    /// Reported acquisition state.
    pub status: Status,
    /// Failure detail; a captured slot must have no error.
    pub error: Option<String>,
    /// Actual image binding, absent when acquisition produced no image.
    pub image: Option<Image>,
    /// Actual producer settings, absent if acquisition failed before configuration.
    pub settings: Option<BTreeMap<String, Value>>,
    /// Actual producer identity, absent when unavailable.
    #[cfg_attr(feature = "schema", schemars(with = "Option<arms::Fingerprint>"))]
    pub fingerprint: Option<Value>,
    /// Actual clock provenance, absent when unavailable.
    pub clock: Option<Clock>,
    /// Adapter-specific observations, such as final URL and HTTP status; data only.
    pub details: BTreeMap<String, Value>,
}
/// Full run record, preserving the plan and every attempted acquisition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// Versioned discriminator.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-capture-record.v1")))]
    pub schema: String,
    /// Adapter family.
    pub producer: Producer,
    /// Nonempty run identity, also bound by each fingerprint and clock.
    pub run_id: String,
    /// Explicit producer completion state.
    pub completion: Completion,
    /// Predeclared expected slots; never inferred from surviving images.
    pub expected: Vec<Expected>,
    /// All acquisitions, including failures and skips.
    pub acquisitions: Vec<Acquisition>,
}
/// Stable failure classification; variants are additive within result v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Code {
    /// Receipt cannot be opened.
    RecordUnavailable,
    /// Receipt is malformed, oversized, duplicated or structurally inconsistent.
    InvalidRecord,
    /// Unsupported record contract version.
    UnsupportedSchema,
    /// Run incomplete, acquisition omitted or explicitly skipped.
    PartialRun,
    /// Acquisition failed or carries an error, even with an image present.
    AcquisitionFailed,
    /// Image binding or image file missing.
    MissingImage,
    /// Image path traverses outside the receipt root or uses a symlink.
    UnsafeImagePath,
    /// Image cannot be read or exceeds the encoded byte limit.
    ImageUnavailable,
    /// Bound bytes disagree with the recorded hash.
    StaleHash,
    /// Capture settings disagree with the plan or are absent.
    SettingsMismatch,
    /// Producer fingerprint is incomplete or malformed.
    MissingIdentity,
    /// Producer identity or run session disagrees with the plan.
    IdentityMismatch,
    /// Readiness was not reached.
    ReadinessNotReached,
    /// Clock is absent, reversed or bound to a different domain or run.
    ClockMismatch,
}
/// One retained conformance failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Finding {
    /// Stable machine code.
    pub code: Code,
    /// Slot identifier, null for whole-record failures.
    pub id: Option<String>,
}
/// Offline validation receipt; does not authorize an image or timing verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Report {
    /// Versioned discriminator.
    pub schema: String,
    /// True only when every declared slot conforms.
    pub conformant: bool,
    /// Number of planned slots.
    pub expected: usize,
    /// Number of retained acquisitions.
    pub acquired: usize,
    /// All failures in plan order; at most one of each code per slot.
    pub findings: Vec<Finding>,
    /// Native receipt or explicitly adapted historical evidence.
    pub provenance: String,
    /// Mapping decisions for legacy fields; empty for native receipts.
    pub fields: Vec<super::capture_legacy::FieldEvidence>,
    /// Contract requirements historical evidence cannot establish.
    pub unavailable: Vec<super::capture_legacy::Unavailable>,
    /// Retained historical retry observations, without inventing successful attempts.
    pub retry_records: Vec<Value>,
}
impl Report {
    pub(super) fn new() -> Self {
        Self {
            schema: RESULT_SCHEMA.into(),
            conformant: true,
            expected: 0,
            acquired: 0,
            findings: Vec::new(),
            provenance: "native".into(),
            fields: Vec::new(),
            unavailable: Vec::new(),
            retry_records: Vec::new(),
        }
    }
    pub(super) fn fail(&mut self, code: Code, id: Option<&str>) {
        if !self
            .findings
            .iter()
            .any(|f| f.code == code && f.id.as_deref() == id)
        {
            self.findings.push(Finding {
                code,
                id: id.map(str::to_owned),
            });
        }
        self.conformant = false;
    }
    /// Exit: 0 native pass, 1 rejected evidence, 2 invalid contract,
    /// 3 adapted pass, 4 unavailable historical requirements.
    pub fn exit_code(&self) -> u8 {
        if self.findings.iter().any(|f| {
            matches!(
                f.code,
                Code::RecordUnavailable | Code::InvalidRecord | Code::UnsupportedSchema
            )
        }) {
            2
        } else {
            if !self.findings.is_empty() {
                1
            } else if !self.unavailable.is_empty() {
                4
            } else if self.provenance == "adapted_legacy" {
                3
            } else {
                u8::from(!self.conformant)
            }
        }
    }
}
fn normalized_absolute(path: &Path) -> std::io::Result<std::path::PathBuf> {
    let mut normalized = std::path::PathBuf::new();
    for component in std::path::absolute(path)?.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            _ => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}
pub(super) fn safe_directory(path: &Path) -> std::io::Result<cap_std::fs::Dir> {
    use cap_fs_ext::DirExt;
    let absolute = normalized_absolute(path)?;
    let mut dir = cap_std::fs::Dir::open_ambient_dir(
        absolute.ancestors().last().unwrap_or(Path::new("/")),
        cap_std::ambient_authority(),
    )?;
    for component in absolute.components() {
        match component {
            Component::Normal(name) => dir = dir.open_dir_nofollow(name)?,
            Component::RootDir | Component::Prefix(_) => {}
            _ => return Err(std::io::Error::other("unsafe path")),
        }
    }
    Ok(dir)
}
pub(super) fn bounded_read(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    let absolute = normalized_absolute(path)?;
    let dir = safe_directory(
        absolute
            .parent()
            .ok_or_else(|| std::io::Error::other("missing parent"))?,
    )?;
    let name = absolute
        .file_name()
        .ok_or_else(|| std::io::Error::other("missing filename"))?;
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = dir.open_with(name, &options)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "not a bounded regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "byte limit exceeded",
        ));
    }
    Ok(bytes)
}
fn hash_valid(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn fingerprint_meta(fp: &Value) -> crate::meta::Meta {
    let mut meta = crate::meta::Meta::new();
    arms::flatten("fingerprint", fp, &mut meta);
    arms::readiness(&mut meta);
    meta
}
fn full_identity(value: &Value) -> bool {
    let Ok(fp) = serde_json::from_value::<arms::Fingerprint>(value.clone()) else {
        return false;
    };
    fp.schema == arms::FINGERPRINT_SCHEMA
        && fp
            .producer
            .as_ref()
            .and_then(|p| p.binary.as_deref())
            .is_some_and(|s| s.strip_prefix("sha256:").is_some_and(hash_valid))
        && fp
            .inputs
            .as_ref()
            .and_then(|i| i.identity.as_deref())
            .is_some_and(|s| s.strip_prefix("sha256:").is_some_and(hash_valid))
        && arms::check(&fingerprint_meta(value), &fingerprint_meta(value), &[], &[])
            .offending
            .iter()
            .all(|f| f.reason == "readiness_not_reached")
}
pub(super) fn image_code(root: &Path, image: &Image) -> Option<Code> {
    let relative = Path::new(&image.path);
    if image.path.is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Some(Code::UnsafeImagePath);
    }
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Some(Code::UnsafeImagePath);
        }
    }
    if !hash_valid(&image.sha256) {
        return Some(Code::InvalidRecord);
    }
    match bounded_read(&path, IMAGE_LIMIT) {
        Ok(bytes) => {
            (format!("{:x}", Sha256::digest(&bytes)) != image.sha256).then_some(Code::StaleHash)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(Code::MissingImage),
        Err(_) => Some(Code::ImageUnavailable),
    }
}
/// Check declared slots, states, settings, identity, clock and image byte bindings.
/// The caller supplies the image root; no inference, provider or browser is run.
pub fn conform(record: &Record, root: &Path) -> Report {
    let mut report = Report::new();
    if record.schema != RECORD_SCHEMA {
        report.fail(Code::UnsupportedSchema, None);
        return report;
    }
    report.expected = record.expected.len();
    report.acquired = record.acquisitions.len();
    let expected: BTreeSet<_> = record.expected.iter().map(|e| e.id.as_str()).collect();
    let acquired: BTreeSet<_> = record.acquisitions.iter().map(|a| a.id.as_str()).collect();
    if record.run_id.trim().is_empty()
        || record.expected.is_empty()
        || record.expected.len() > ITEM_LIMIT
        || record.acquisitions.len() > ITEM_LIMIT
        || expected.len() != record.expected.len()
        || acquired.len() != record.acquisitions.len()
        || expected
            .iter()
            .chain(acquired.iter())
            .any(|id| id.trim().is_empty() || id.len() > 128)
        || !acquired.is_subset(&expected)
    {
        report.fail(Code::InvalidRecord, None);
        return report;
    }
    if record.completion == Completion::Partial {
        report.fail(Code::PartialRun, None);
    }
    for plan in &record.expected {
        let id = Some(plan.id.as_str());
        if plan.clock_domain.trim().is_empty()
            || !full_identity(&plan.fingerprint)
            || !arms::check(
                &fingerprint_meta(&plan.fingerprint),
                &fingerprint_meta(&plan.fingerprint),
                &[],
                &[],
            )
            .offending
            .is_empty()
            || plan
                .fingerprint
                .pointer("/run/session")
                .and_then(Value::as_str)
                != Some(record.run_id.as_str())
        {
            report.fail(Code::InvalidRecord, id);
            continue;
        }
        let Some(actual) = record.acquisitions.iter().find(|a| a.id == plan.id) else {
            report.fail(Code::PartialRun, id);
            continue;
        };
        if actual.status == Status::Failed || actual.error.is_some() {
            report.fail(Code::AcquisitionFailed, id);
            continue;
        }
        if actual.status == Status::Skipped {
            report.fail(Code::PartialRun, id);
            continue;
        }
        if actual.settings.as_ref() != Some(&plan.settings) {
            report.fail(Code::SettingsMismatch, id);
        }
        if let Some(fp) = &actual.fingerprint {
            if !full_identity(fp) {
                report.fail(Code::MissingIdentity, id);
            } else {
                let check = arms::check(
                    &fingerprint_meta(&plan.fingerprint),
                    &fingerprint_meta(fp),
                    &[],
                    &[],
                );
                if check
                    .offending
                    .iter()
                    .any(|f| f.reason == "readiness_not_reached")
                {
                    report.fail(Code::ReadinessNotReached, id);
                }
                if check
                    .offending
                    .iter()
                    .any(|f| f.reason != "readiness_not_reached")
                {
                    report.fail(Code::IdentityMismatch, id);
                }
            }
        } else {
            report.fail(Code::MissingIdentity, id);
        }
        if actual.clock.as_ref().is_none_or(|c| {
            c.domain != plan.clock_domain || c.run_id != record.run_id || c.end_ns < c.start_ns
        }) {
            report.fail(Code::ClockMismatch, id);
        }
        match &actual.image {
            None => report.fail(Code::MissingImage, id),
            Some(image) => {
                if let Some(code) = image_code(root, image) {
                    report.fail(code, id);
                }
            }
        }
    }
    report
}
/// Read at most 1 MiB of receipt JSON and validate images relative to its parent.
pub fn conform_path(path: &Path) -> Report {
    let mut failed = Report::new();
    // Explicit native receipt roots may be reached through caller directory aliases.
    // Pin the resolved root; descendant image components still refuse symlinks.
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let root = match parent.canonicalize() {
        Ok(root) => root,
        Err(_) => {
            failed.fail(Code::RecordUnavailable, None);
            return failed;
        }
    };
    let resolved = root.join(path.file_name().unwrap_or_default());
    let bytes = match bounded_read(&resolved, RECORD_LIMIT) {
        Ok(bytes) => bytes,
        Err(e) => {
            failed.fail(
                if e.kind() == std::io::ErrorKind::InvalidData {
                    Code::InvalidRecord
                } else {
                    Code::RecordUnavailable
                },
                None,
            );
            return failed;
        }
    };
    let value: Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => {
            failed.fail(Code::InvalidRecord, None);
            return failed;
        }
    };
    if value
        .get("schema")
        .and_then(Value::as_str)
        .is_some_and(|schema| schema != RECORD_SCHEMA)
    {
        failed.fail(Code::UnsupportedSchema, None);
        return failed;
    }
    match serde_json::from_slice(&bytes) {
        Ok(record) => conform(&record, &root),
        Err(_) => {
            failed.fail(Code::InvalidRecord, None);
            failed
        }
    }
}
