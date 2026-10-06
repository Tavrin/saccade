//! Metadata sidecars: the configuration a capture was made with, compared
//! alongside its pixels.
//!
//! A sidecar is a flat JSON object (string, number, bool or null values). The
//! directory-level file `<dir>/<name>` applies to every image below `<dir>`;
//! the per-image file `<stem>.<name>` next to an image overrides it key by key.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use globset::GlobMatcher;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::compile_glob;
use crate::error::{Error, Result};
use crate::report::{MetaDiff, MetaSettings};

/// Default sidecar file name.
pub const DEFAULT_META_NAME: &str = "saccade-meta.json";

/// Keys ignored by default: timings, timestamps and run ids differ between any
/// two runs and say nothing about the configuration. Matched case-insensitively.
/// The patterns are deliberately narrow (no bare `*time*`, which would also
/// hide `timezone`, `timeout` or `lifetime`); every ignored key that differs is
/// still listed in the report as `meta_ignored_diff`.
pub const DEFAULT_IGNORE: &[&str] = &[
    "*timestamp*",
    "*_ms",
    "*.ms",
    "timing.*",
    "*duration*",
    "*elapsed*",
    "run.id",
    "*.started_at",
    "*.finished_at",
    "generated_at*",
];

/// Placeholder shown for a key (or a whole sidecar) missing on one side.
pub const ABSENT: &str = "<absent>";

/// Largest sidecar read, in bytes.
const MAX_SIDECAR_BYTES: u64 = 1 << 20;

/// Explicit measurement/provenance keys ignored by a capture proof profile.
pub const PROOF_IGNORE: &[&str] = &[
    "run.id",
    "generated_at_unix",
    "capture.timestamp",
    "capture.started_at",
    "capture.finished_at",
    "gpu_ms",
    "frame_ms",
    "timing.gpu_ms",
    "timing.frame_ms",
    "elapsed.s",
];

/// Capture comparability, independent of native sample equality.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Validity {
    /// Supplied capture requirements are satisfied.
    Valid,
    /// Evidence violates a supplied requirement or cannot be decoded safely.
    Invalid,
    /// The supplied context cannot establish comparability.
    #[default]
    Unknown,
}

/// Validity and the missing context or violated requirements explaining it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureValidity {
    /// Valid, invalid, or unknown.
    pub status: Validity,
    /// Explicit limits or failures.
    pub reasons: Vec<String>,
}

impl Default for CaptureValidity {
    fn default() -> Self {
        Self {
            status: Validity::Unknown,
            reasons: vec!["capture context was not checked".into()],
        }
    }
}

/// A predeclared intervention; matching values do not prove it happened.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredChange {
    /// Exact metadata key.
    pub key: String,
    /// Why this difference is permitted.
    pub reason: String,
    /// Expected baseline value, when specified.
    pub before: Option<Value>,
    /// Expected candidate value, when specified.
    pub after: Option<Value>,
}

/// Sidecar settings for a run or a view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaOptions {
    /// Refuse verdicts without complete matching arm identity.
    pub require_valid_arms: bool,
    /// Generic producer field mapping file.
    pub fingerprint_map: Option<std::path::PathBuf>,
    /// Intended experiment variables, exact keys or globs, kept separately.
    pub intended: Vec<String>,
    /// Sidecar file name; the per-image sidecar is `<stem>.<name>`.
    pub name: String,
    /// Whether an undeclared differing key is an error (`compare`/`identity`).
    pub required: bool,
    /// Keys (or globs) whose difference is expected.
    pub declared: Vec<String>,
    /// Extra ignore globs, added to [`DEFAULT_IGNORE`].
    pub ignore: Vec<String>,
    /// Non-null metadata fields required by the project's capture contract.
    pub required_keys: Vec<String>,
    /// Structured declarations with reasons and optional expected values.
    pub changes: Vec<DeclaredChange>,
}

impl Default for MetaOptions {
    fn default() -> Self {
        Self {
            require_valid_arms: false,
            fingerprint_map: None,
            intended: Vec::new(),
            name: DEFAULT_META_NAME.to_owned(),
            required: false,
            declared: Vec::new(),
            ignore: Vec::new(),
            required_keys: Vec::new(),
            changes: Vec::new(),
        }
    }
}

/// A parsed, validated sidecar: key to scalar JSON value.
pub type Meta = BTreeMap<String, Value>;

/// Capture identities advertised by either the selected sidecar or a Moss cost card.
/// These are declarations by the producer, not hashes computed from image pixels.
pub fn capture_evidence(root: &Path, rel: &str, name: &str) -> BTreeMap<String, String> {
    let mut evidence = BTreeMap::new();
    for sidecar in ["cost-card.json", name] {
        let options = MetaOptions {
            name: sidecar.into(),
            ..Default::default()
        };
        if let Ok(checker) = options.checker()
            && let Ok(Some(meta)) = checker.load(root, rel)
        {
            for (key, value) in meta {
                let normalized = key.to_ascii_lowercase().replace(['.', '-', ' '], "_");
                let field = match normalized.as_str() {
                    "binary_sha"
                    | "binary_sha256"
                    | "binary_hash"
                    | "build_binary_sha256"
                    | "producer_binary" => "binary_sha256",
                    "source_head"
                    | "git_head"
                    | "source_git_head"
                    | "build_commit"
                    | "producer_build_commit" => "source_head",
                    "capture_timestamp" => "timestamp",
                    "capture_id" | "capture_uuid" => "capture_id",
                    "content_hash" | "capture_hash" | "capture_sha256" => "content_hash",
                    _ => continue,
                };
                if field == "timestamp" {
                    if let Some(timestamp) = value.as_u64() {
                        evidence.insert(field.into(), timestamp.to_string());
                    } else if let Some(value) = value.as_str().filter(|s| !s.trim().is_empty()) {
                        evidence.insert(field.into(), value.into());
                    }
                } else if let Some(value) = value.as_str().filter(|s| !s.trim().is_empty()) {
                    evidence.insert(field.into(), value.into());
                }
            }
        }
    }
    #[cfg(feature = "graphics")]
    if !evidence.contains_key("content_hash")
        && let Ok(Some(perf)) = crate::perf::CapturePerf::read(root, crate::perf::DEFAULT_PERF_NAME)
        && let Some(hash) = perf.context.as_ref().and_then(|c| c.capture_hash.as_ref())
    {
        evidence.insert("content_hash".into(), hash.as_str().into());
    }
    evidence
}

/// A repeated producer identity means that the two inputs are the same capture.
pub fn same_capture(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> bool {
    ["capture_id", "content_hash"]
        .into_iter()
        .any(|key| a.get(key).zip(b.get(key)).is_some_and(|(a, b)| a == b))
}

/// [`MetaOptions`] with its globs compiled.
pub struct MetaChecker {
    fingerprint_map: Option<std::path::PathBuf>,
    intended_globs: Vec<GlobMatcher>,
    name: String,
    required: bool,
    declared: Vec<String>,
    declared_globs: Vec<GlobMatcher>,
    ignore: Vec<GlobMatcher>,
    explicit_ignore: Vec<GlobMatcher>,
    settings: MetaSettings,
    required_keys: Vec<String>,
    changes: Vec<DeclaredChange>,
}

/// Checked metadata, including differences that were permitted or ignored.
pub(crate) struct CheckedMeta {
    pub intended: Vec<MetaDiff>,
    pub covered_by_derivation: Vec<MetaDiff>,
    pub diff: Vec<MetaDiff>,
    pub ignored: Vec<MetaDiff>,
    pub unchanged: Vec<String>,
    pub validity: CaptureValidity,
    pub failure: Option<String>,
}

impl MetaOptions {
    /// Checks the file name and compiles the globs.
    pub fn checker(&self) -> Result<MetaChecker> {
        if self.name.is_empty() || self.name.contains(['/', '\\']) || self.name.contains('\0') {
            return Err(Error::Config(format!(
                "meta name {:?} must be a plain file name",
                self.name
            )));
        }
        let proof = !self.required_keys.is_empty();
        if proof && self.ignore.iter().any(|k| k.contains(['*', '?', '[', '{'])) {
            return Err(Error::Config(
                "capture proof ignores must name explicit fields, not globs".into(),
            ));
        }
        if self.required_keys.iter().any(|k| k.trim().is_empty()) {
            return Err(Error::Config(
                "capture required_keys must name nonempty metadata keys".into(),
            ));
        }
        for change in &self.changes {
            if change.key.trim().is_empty()
                || change.reason.trim().is_empty()
                || change.key.contains(['*', '?', '[', '{'])
                || change
                    .before
                    .iter()
                    .chain(&change.after)
                    .any(|v| matches!(v, Value::Array(_) | Value::Object(_)))
            {
                return Err(Error::Config(
                    "changes require an exact key, a reason, and scalar expected values".into(),
                ));
            }
        }
        let ignored: Vec<String> = (if self.require_valid_arms {
            &[][..]
        } else if proof {
            PROOF_IGNORE
        } else {
            DEFAULT_IGNORE
        })
        .iter()
        .map(|g| (*g).to_owned())
        .chain(self.ignore.iter().cloned())
        .collect();
        let ignore = ignored
            .iter()
            .map(|g| compile_glob(g))
            .collect::<Result<Vec<_>>>()?;
        let declared_globs = self
            .declared
            .iter()
            .map(|g| compile_glob(g))
            .collect::<Result<Vec<_>>>()?;
        Ok(MetaChecker {
            fingerprint_map: self.fingerprint_map.clone(),
            intended_globs: self
                .intended
                .iter()
                .map(|g| compile_glob(g))
                .collect::<Result<Vec<_>>>()?,
            name: self.name.clone(),
            required: self.required || proof || self.require_valid_arms,
            declared: self.declared.clone(),
            declared_globs,
            ignore,
            explicit_ignore: self
                .ignore
                .iter()
                .map(|g| compile_glob(g))
                .collect::<Result<_>>()?,
            required_keys: self.required_keys.clone(),
            changes: self.changes.clone(),
            settings: MetaSettings {
                require_valid_arms: self.require_valid_arms,
                arm_ignore: self.ignore.clone(),
                intended: self.intended.clone(),
                name: self.name.clone(),
                required: self.required || proof || self.require_valid_arms,
                declared: self.declared.clone(),
                ignored,
                required_keys: self.required_keys.clone(),
                changes: self.changes.clone(),
            },
        })
    }
}

fn render(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Reads `path` as a flat sidecar; `Ok(None)` when the file does not exist.
fn read_one(path: &Path) -> std::result::Result<Option<Meta>, String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if meta.file_type().is_symlink() {
        return Err(format!("{}: symlinks are not followed", path.display()));
    }
    if !meta.is_file() {
        return Ok(None);
    }
    if meta.len() > MAX_SIDECAR_BYTES {
        return Err(format!(
            "{}: larger than {MAX_SIDECAR_BYTES} bytes",
            path.display()
        ));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{}: invalid JSON: {e}", path.display()))?;
    let Value::Object(map) = value else {
        return Err(format!("{}: not a JSON object", path.display()));
    };
    let mut out = Meta::new();
    for (key, v) in map {
        if matches!(key.as_str(), "fingerprint" | "producer" | "inputs" | "run")
            || key.starts_with("producer.")
            || key.starts_with("inputs.")
            || key.starts_with("run.")
        {
            crate::arms::flatten(&key, &v, &mut out);
            continue;
        }
        if matches!(v, Value::Array(_) | Value::Object(_)) {
            return Err(format!(
                "{}: key {key:?} has a nested value (sidecars are flat: string, number, bool or null)",
                path.display()
            ));
        }
        out.insert(key, v);
    }
    Ok(Some(out))
}

impl MetaChecker {
    /// Effective metadata sidecar name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Loads the effective sidecar of image `rel` (`/`-separated, relative to
    /// `root`): directory sidecars from `root` down to the image's directory
    /// (nearer wins), then the per-image sidecar on top. `Ok(None)` when no
    /// sidecar applies; `Err` names the unreadable file or the nested key.
    pub fn load(&self, root: &Path, rel: &str) -> std::result::Result<Option<Meta>, String> {
        if let Some(path) = &self.fingerprint_map {
            let map = crate::arms::FingerprintMap::read(path).map_err(|e| e.to_string())?;
            return crate::arms::load_named(root, rel, &self.name, Some(&map))
                .map(|mut m| {
                    crate::arms::readiness(&mut m);
                    Some(m)
                })
                .map_err(|e| e.to_string());
        }
        let parts: Vec<&str> = rel.split('/').collect();
        let Some((file, dirs)) = parts.split_last() else {
            return Ok(None);
        };
        let mut found: Option<Meta> = None;
        let mut layers: Vec<std::path::PathBuf> = Vec::new();
        let mut dir = root.to_path_buf();
        layers.push(dir.join(&self.name));
        for d in dirs {
            dir.push(d);
            layers.push(dir.join(&self.name));
        }
        let stem = Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file);
        layers.push(dir.join(format!("{stem}.{}", self.name)));
        for path in layers {
            if let Some(layer) = read_one(&path)? {
                found.get_or_insert_with(Meta::new).extend(layer);
            }
        }
        if let Some(meta) = &mut found {
            crate::arms::readiness(meta);
        }
        Ok(found)
    }

    fn is_ignored(&self, key: &str) -> bool {
        (self.settings.require_valid_arms
            && self
                .settings
                .arm_ignore
                .iter()
                .any(|t| crate::arms::matches(t, key)))
            || self.explicit_ignore.iter().any(|g| g.is_match(key))
            || (![
                "qualification.",
                "producer.",
                "inputs.",
                "run.mode",
                "run.env",
                "run.readiness",
                "run.session",
                "fingerprint.",
            ]
            .iter()
            .any(|p| key.to_ascii_lowercase().starts_with(p))
                && self.ignore.iter().any(|g| g.is_match(key)))
    }

    fn is_intended(&self, key: &str) -> bool {
        self.intended_globs.iter().any(|g| g.is_match(key))
            || self
                .settings
                .intended
                .iter()
                .any(|t| crate::arms::matches(t, key))
    }

    fn is_declared(&self, key: &str) -> bool {
        self.is_intended(key)
            || self.declared.iter().any(|d| d == key)
            || self.declared_globs.iter().any(|g| g.is_match(key))
            || self.changes.iter().any(|c| c.key == key)
    }

    /// Keys that differ between the two sidecars, ignored keys excluded,
    /// sorted by key. A side without any sidecar shows every key of the other
    /// as [`ABSENT`]; two sides without one have no difference.
    pub fn diff(&self, baseline: Option<&Meta>, capture: Option<&Meta>) -> Vec<MetaDiff> {
        self.diff_split(baseline, capture).0
    }

    /// Like [`Self::diff`], plus the differing keys that were ignored.
    pub fn diff_split(
        &self,
        baseline: Option<&Meta>,
        capture: Option<&Meta>,
    ) -> (Vec<MetaDiff>, Vec<MetaDiff>) {
        let empty = Meta::new();
        let (b, c) = (baseline.unwrap_or(&empty), capture.unwrap_or(&empty));
        let keys: BTreeSet<&String> = b.keys().chain(c.keys()).collect();
        let (mut kept, mut ignored) = (Vec::new(), Vec::new());
        for k in keys.into_iter().filter(|k| b.get(*k) != c.get(*k)) {
            let d = MetaDiff {
                key: k.clone(),
                baseline: b.get(k).map_or_else(|| ABSENT.to_owned(), render),
                capture: c.get(k).map_or_else(|| ABSENT.to_owned(), render),
            };
            if self.is_ignored(k) {
                ignored.push(d);
            } else {
                kept.push(d);
            }
        }
        (kept, ignored)
    }

    /// The differing keys that are not declared, when matching is required.
    pub fn violations<'a>(&self, diff: &'a [MetaDiff]) -> Vec<&'a str> {
        if !self.required {
            return Vec::new();
        }
        diff.iter()
            .filter(|d| !self.is_declared(&d.key))
            .map(|d| d.key.as_str())
            .collect()
    }

    /// Loads both sides of `name` and returns the diff plus the error text
    /// when `required` and an undeclared key differs.
    pub(crate) fn compare(
        &self,
        base_root: &Path,
        cap_root: &Path,
        name: &str,
    ) -> std::result::Result<(Vec<MetaDiff>, Option<String>), String> {
        self.compare_split(base_root, cap_root, name)
            .map(|(diff, _, err)| (diff, err))
    }

    /// [`Self::compare`] plus the differing keys that were ignored.
    #[allow(clippy::type_complexity)]
    pub(crate) fn compare_split(
        &self,
        base_root: &Path,
        cap_root: &Path,
        name: &str,
    ) -> std::result::Result<(Vec<MetaDiff>, Vec<MetaDiff>, Option<String>), String> {
        self.compare_named(base_root, name, cap_root, name)
    }

    /// Metadata comparison using the actual filename on each side of a file pair.
    #[allow(clippy::type_complexity)]
    pub(crate) fn compare_named(
        &self,
        base_root: &Path,
        base_name: &str,
        cap_root: &Path,
        cap_name: &str,
    ) -> std::result::Result<(Vec<MetaDiff>, Vec<MetaDiff>, Option<String>), String> {
        let checked = self.check_named(base_root, base_name, cap_root, cap_name)?;
        Ok((checked.diff, checked.ignored, checked.failure))
    }

    /// Checks context without conflating metadata validity with pixel equality.
    pub(crate) fn check_named(
        &self,
        base_root: &Path,
        base_name: &str,
        cap_root: &Path,
        cap_name: &str,
    ) -> std::result::Result<CheckedMeta, String> {
        let b = self
            .load(base_root, base_name)
            .map_err(|e| format!("baseline sidecar: {e}"))?;
        let c = self
            .load(cap_root, cap_name)
            .map_err(|e| format!("capture sidecar: {e}"))?;
        let empty = Meta::new();
        let (bm, cm) = (b.as_ref().unwrap_or(&empty), c.as_ref().unwrap_or(&empty));
        let keys: BTreeSet<_> = bm.keys().chain(cm.keys()).collect();
        let intended = keys
            .into_iter()
            .filter(|k| self.is_intended(k))
            .map(|k| MetaDiff {
                key: k.clone(),
                baseline: bm.get(k).map_or_else(|| ABSENT.into(), render),
                capture: cm.get(k).map_or_else(|| ABSENT.into(), render),
            })
            .collect();
        let map = self
            .fingerprint_map
            .as_deref()
            .map(crate::arms::FingerprintMap::read)
            .transpose()
            .map_err(|e| e.to_string())?;
        let derived = crate::arms::derived_keys(map.as_ref(), &self.settings.intended);
        let is_derived = |key: &str| {
            derived.contains(key)
                && !self.is_intended(key)
                && key != "fingerprint.schema"
                && !(key.starts_with("run.readiness.")
                    && (key.ends_with(".criterion") || key.ends_with(".reached")))
                && bm.get(key).is_some_and(|v| !v.is_null())
                && cm.get(key).is_some_and(|v| !v.is_null())
        };
        let covered_by_derivation = bm
            .keys()
            .chain(cm.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|k| is_derived(k) && bm.get(*k) != cm.get(*k))
            .map(|k| MetaDiff {
                key: k.clone(),
                baseline: bm.get(k).map_or_else(|| ABSENT.into(), render),
                capture: cm.get(k).map_or_else(|| ABSENT.into(), render),
            })
            .collect();
        let (mut diff, mut ignored) = self.diff_split(b.as_ref(), c.as_ref());
        diff.retain(|d| !is_derived(&d.key));
        ignored.retain(|d| !is_derived(&d.key));
        diff.retain(|d| !self.is_intended(&d.key));
        ignored.retain(|d| !self.is_intended(&d.key));
        let bad = self.violations(&diff);
        let mut reasons = Vec::new();
        if !bad.is_empty() {
            reasons.push(format!(
                "configuration differs on undeclared keys: {} (declare them with --declare to accept)",
                bad.join(", ")
            ));
        }
        for (side, card) in [("baseline", &b), ("capture", &c)] {
            if card.as_ref().is_none_or(|m| m.is_empty()) {
                reasons.push(format!(
                    "{side} metadata is absent or empty; supply --meta-name {} with a sidecar",
                    self.name
                ));
            }
            for key in &self.required_keys {
                if card
                    .as_ref()
                    .and_then(|m| m.get(key))
                    .is_none_or(|v| v.is_null() || v.as_str().is_some_and(|s| s.trim().is_empty()))
                {
                    reasons.push(format!(
                        "{side} required capture key {key:?} is absent or empty in --meta-name {}",
                        self.name
                    ));
                }
            }
        }
        let mut unexpected = false;
        for change in &self.changes {
            for (side, expected, actual) in [
                ("baseline", &change.before, &b),
                ("capture", &change.after, &c),
            ] {
                if let Some(expected) = expected
                    && actual.as_ref().and_then(|m| m.get(&change.key)) != Some(expected)
                {
                    unexpected = true;
                    reasons.push(format!(
                        "{side} declared key {:?} does not match expected {} ({})",
                        change.key,
                        render(expected),
                        change.reason
                    ));
                }
            }
        }
        let unchanged = b.as_ref().map_or_else(Vec::new, |b| {
            b.iter()
                .filter(|(k, v)| {
                    self.is_declared(k) && c.as_ref().and_then(|c| c.get(*k)) == Some(*v)
                })
                .map(|(k, _)| k.clone())
                .collect()
        });
        let status = if reasons.is_empty() && diff.iter().all(|d| self.is_declared(&d.key)) {
            Validity::Valid
        } else if self.required || unexpected {
            Validity::Invalid
        } else {
            Validity::Unknown
        };
        if status == Validity::Unknown && reasons.is_empty() {
            let missing = diff
                .iter()
                .filter(|d| !self.is_declared(&d.key))
                .map(|d| d.key.as_str())
                .collect::<Vec<_>>();
            reasons.push(format!("metadata differs on undeclared keys: {}{}; use --declare KEY for an intentional change or --require-matching-meta to reject it", missing.iter().take(8).copied().collect::<Vec<_>>().join(", "), if missing.len()>8 {format!(", and {} more",missing.len()-8)} else {String::new()}));
        }
        let failure = (status == Validity::Invalid).then(|| reasons.join("; "));
        Ok(CheckedMeta {
            intended,
            covered_by_derivation,
            diff,
            ignored,
            unchanged,
            validity: CaptureValidity { status, reasons },
            failure,
        })
    }

    /// The settings as recorded in the report.
    pub fn settings(&self) -> MetaSettings {
        self.settings.clone()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod wave9_tests {
    use super::*;
    #[test]
    fn default_ignores_stay_compatible_but_fingerprint_fields_are_first_class() {
        let a = Meta::from([
            ("run.id".into(), serde_json::json!("a")),
            ("run.env.capture_ms".into(), serde_json::json!(1)),
        ]);
        let b = Meta::from([
            ("run.id".into(), serde_json::json!("b")),
            ("run.env.capture_ms".into(), serde_json::json!(2)),
        ]);
        let checker = MetaOptions::default().checker().unwrap();
        let (kept, ignored) = checker.diff_split(Some(&a), Some(&b));
        assert_eq!(kept[0].key, "run.env.capture_ms");
        assert_eq!(ignored[0].key, "run.id");
        let opts = MetaOptions {
            require_valid_arms: true,
            ..Default::default()
        };
        assert_eq!(opts.checker().unwrap().diff(Some(&a), Some(&b)).len(), 2);
    }
    #[test]
    fn intended_globs_are_separate_and_unexplained_keys_still_fail() {
        let tmp = tempfile::tempdir().unwrap();
        let b = tmp.path().join("b");
        let c = tmp.path().join("c");
        std::fs::create_dir(&b).unwrap();
        std::fs::create_dir(&c).unwrap();
        std::fs::write(
            b.join(DEFAULT_META_NAME),
            r#"{"env.FEATURE_A":false,"binary.sha":"a","camera":1,"env.FEATURE_B":true}"#,
        )
        .unwrap();
        std::fs::write(
            c.join(DEFAULT_META_NAME),
            r#"{"env.FEATURE_A":true,"binary.sha":"b","camera":1,"env.FEATURE_B":true}"#,
        )
        .unwrap();
        let options = MetaOptions {
            intended: vec!["env.FEATURE_*".into(), "binary.sha".into()],
            required: true,
            ..Default::default()
        };
        let checker = options.checker().unwrap();
        let checked = checker.check_named(&b, "a.png", &c, "a.png").unwrap();
        assert_eq!(checked.validity.status, Validity::Valid);
        assert!(checked.diff.is_empty());
        assert_eq!(checked.intended.len(), 3);
        assert!(
            checked
                .intended
                .iter()
                .any(|d| d.key == "env.FEATURE_B" && d.baseline == d.capture)
        );
        std::fs::write(
            c.join(DEFAULT_META_NAME),
            r#"{"env.FEATURE_A":true,"binary.sha":"b","camera":2}"#,
        )
        .unwrap();
        let checked = checker.check_named(&b, "a.png", &c, "a.png").unwrap();
        assert_eq!(checked.validity.status, Validity::Invalid);
        assert_eq!(checked.diff[0].key, "camera");
        assert!(
            checked
                .intended
                .iter()
                .any(|d| d.key == "env.FEATURE_B" && d.capture == ABSENT)
        );
    }
    #[test]
    fn intended_variables_do_not_override_expected_values_or_required_presence() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(DEFAULT_META_NAME), r#"{"binary.sha":"a"}"#).unwrap();
        let opts = MetaOptions {
            intended: vec!["binary.sha".into()],
            required_keys: vec!["camera".into()],
            changes: vec![DeclaredChange {
                key: "binary.sha".into(),
                reason: "expected build".into(),
                before: None,
                after: Some(Value::String("b".into())),
            }],
            ..Default::default()
        };
        let r = opts
            .checker()
            .unwrap()
            .check_named(tmp.path(), "a.png", tmp.path(), "a.png")
            .unwrap();
        assert_eq!(r.validity.status, Validity::Invalid);
        assert!(r.validity.reasons.len() >= 3);
    }
}
