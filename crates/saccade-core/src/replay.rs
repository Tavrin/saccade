//! Offline evidence replay contracts and bounded integrity checks.
use crate::wave7::models::{digest, read_bounded, valid_hash};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Component, Path, PathBuf},
};

/// First replay recipe contract; successors get a new discriminator.
pub const RECIPE_SCHEMA: &str = "saccade-replay-recipe.v1";
/// First evidence pack manifest contract.
pub const PACK_SCHEMA: &str = "saccade-replay-pack.v1";
/// Bounded execution receipt contract.
pub const RESULT_SCHEMA: &str = "saccade-replay-result.v1";
/// Maximum regular file size (256 MiB).
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum aggregate pack size (2 GiB).
pub const MAX_PACK_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Maximum recorded files.
pub const MAX_FILES: usize = 8192;

/// Typed refusal, never a successful verification.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ReplayError {
    /// Stable machine-readable reason.
    pub code: &'static str,
    /// Bounded description of the failed requirement.
    pub message: String,
}
/// Replay-specific result.
pub type Result<T> = std::result::Result<T, ReplayError>;
/// Construct a typed refusal.
pub fn fail(code: &'static str, message: impl Into<String>) -> ReplayError {
    ReplayError {
        code,
        message: message.into(),
    }
}

/// An explicitly selected registry contract and compatible local runtime.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ocr {
    /// Shared model registry, relative to the recipe.
    pub registry: PathBuf,
    /// Stable OCR contract id in that registry (no automatic selection).
    pub contract_id: String,
    /// Exact ONNX Runtime library; no downloads or ambient cache lookup.
    pub library: PathBuf,
}
/// Closed local operations, deliberately excluding shell and provider execution.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Standard raster file or directory comparison with explicit tolerances.
    Compare {
        /// Reference input relative to the recipe.
        a: PathBuf,
        /// Candidate input relative to the recipe.
        b: PathBuf,
        /// FLIP deciding threshold in [0,1].
        threshold: f64,
        /// mean, max, p95 or p99.
        metric: String,
        /// Observer pixels per degree, positive and finite.
        ppd: f32,
    },
    /// Text comparison using imported observations or local pinned PP-OCRv5.
    Text {
        /// Reference raster relative to the recipe.
        a: PathBuf,
        /// Candidate raster relative to the recipe.
        b: PathBuf,
        /// Optional reference image-bound observations.
        a_source: Option<PathBuf>,
        /// Optional candidate image-bound observations.
        b_source: Option<PathBuf>,
        /// Required when either imported source is absent.
        ocr: Option<Ocr>,
        /// Exact expected Unicode text, treated as inert data.
        expect_text: Vec<String>,
        /// Confidence cutoff in [0,100].
        readable_confidence: f64,
        /// Positive or zero movement cutoff in pixels.
        moved_px: f64,
    },
}
/// Versioned, complete configuration for the recorded operation.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// RECIPE_SCHEMA.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-replay-recipe.v1")))]
    pub schema: String,
    /// Local operation and all its effective options.
    pub run: Operation,
}
impl Recipe {
    /// Reject unsupported options rather than silently falling back.
    pub fn validate(&self) -> Result<()> {
        if self.schema != RECIPE_SCHEMA {
            return Err(fail("replay_schema_changed", "unsupported recipe schema"));
        }
        let valid = match &self.run {
            Operation::Compare {
                threshold,
                metric,
                ppd,
                ..
            } => {
                threshold.is_finite()
                    && (0.0..=1.0).contains(threshold)
                    && matches!(metric.as_str(), "mean" | "max" | "p95" | "p99")
                    && ppd.is_finite()
                    && *ppd > 0.
            }
            Operation::Text {
                a_source,
                b_source,
                ocr,
                expect_text,
                readable_confidence,
                moved_px,
                ..
            } => {
                (a_source.is_some() && b_source.is_some() || ocr.is_some())
                    && expect_text.len() <= 64
                    && expect_text.iter().all(|s| s.len() <= 4096)
                    && readable_confidence.is_finite()
                    && (0.0..=100.0).contains(readable_confidence)
                    && moved_px.is_finite()
                    && *moved_px >= 0.
            }
        };
        if !valid {
            return Err(fail(
                "replay_config_changed",
                "invalid or incomplete replay configuration",
            ));
        }
        Ok(())
    }
}
/// Integrity category; each has a distinct drift code.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Encoded inputs, sidecars and imported observations.
    Input,
    /// Recipe and rewritten model registry.
    Config,
    /// Model weights or dictionary.
    Model,
    /// Optional native runtime.
    Runtime,
    /// Exact CLI binary and build identity.
    Tool,
    /// Exact shipped schemas.
    Schema,
    /// Full report and report manifest/index.
    Report,
}
impl Role {
    /// Stable changed-content refusal code.
    pub fn changed(self) -> &'static str {
        match self {
            Self::Input => "replay_input_changed",
            Self::Config => "replay_config_changed",
            Self::Model => "replay_model_changed",
            Self::Runtime => "replay_runtime_changed",
            Self::Tool => "replay_tool_changed",
            Self::Schema => "replay_schema_changed",
            Self::Report => "replay_report_changed",
        }
    }
}
/// One regular pack file; external paths and symlinks are never followed.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    /// Portable relative path within the pack.
    pub path: String,
    /// Exact byte count.
    pub bytes: u64,
    /// Lowercase SHA-256 of exact bytes.
    pub sha256: String,
    /// Integrity category.
    pub role: Role,
}
/// Model ids and provenance, distinct from reproduction/quality acceptance.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Stable shared-registry contract id.
    pub contract_id: String,
    /// Artifact role within that contract.
    pub role: String,
    /// Immutable source revision.
    pub revision: String,
    /// Exact artifact digest.
    pub sha256: String,
    /// Upstream licence.
    pub license: String,
    /// Source URL, never fetched during replay.
    pub source: String,
}
/// Immutable evidence pack manifest, sealed separately by pack.sha256.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    /// PACK_SCHEMA.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-replay-pack.v1")))]
    pub schema: String,
    /// All files consumed by replay and its recorded artifacts.
    pub files: Vec<File>,
    /// Exact selected model identities; empty for model-independent operations.
    pub models: Vec<Model>,
    /// Exact generated argument array; never shell syntax.
    pub argv: Vec<String>,
    /// Full recorded report's path inside the pack.
    pub report: String,
    /// Existing relocatable report_id.
    pub report_id: String,
    /// Complete replay projection identity, excluding only documented provenance and diagnostic compute durations.
    pub comparison_id: String,
    /// Recorded measurement exit code, 0 or 1; a regression can reproduce.
    pub measurement_exit: u8,
    /// Operating system and architecture required by the binary/runtime.
    pub platform: String,
}
/// Successful bounded replay receipt; errors use the existing typed CLI envelope.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    /// RESULT_SCHEMA.
    #[cfg_attr(feature = "schema", schemars(extend("const" = "saccade-replay-result.v1")))]
    pub schema: String,
    /// pack or verify.
    pub operation: String,
    /// Always reproduced; drift/unavailability never uses a success receipt.
    pub status: String,
    /// Verified pack manifest hash.
    pub pack_sha256: String,
    /// Reproduced report identity.
    pub report_id: String,
    /// Actual fresh report id; diagnostic computation durations may make it differ.
    pub replayed_report_id: String,
    /// Independent original measurement exit code.
    pub measurement_exit: u8,
}
/// Hash the complete replay comparison projection. This preserves real performance
/// measurements; only known diagnostic computation durations are non-repeatable.
pub fn comparison_id(report: &serde_json::Value) -> Result<String> {
    let mut value = report.clone();
    let original =
        crate::report_links::original_schema(report["schema"].as_str().unwrap_or_default());
    if let Some(object) = value.as_object_mut() {
        object.insert("schema".into(), original.into());
        for key in [
            "generated_at_unix",
            "generated_at",
            "generated_at_utc",
            "report_id",
            "source_refs",
        ] {
            object.remove(key);
        }
        if original == crate::report::REPORT_SCHEMA {
            object.remove("baseline_dir");
            object.remove("capture_dir");
            if let Some(entries) = object
                .get_mut("entries")
                .and_then(serde_json::Value::as_array_mut)
            {
                for entry in entries {
                    if let Some(object) = entry.as_object_mut() {
                        object.remove("paths");
                    }
                    if let Some(diagnostics) = entry
                        .get_mut("diagnostics")
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        diagnostics.remove("elapsed_ms");
                        if let Some(shift) = diagnostics
                            .get_mut("shift")
                            .and_then(serde_json::Value::as_object_mut)
                        {
                            shift.remove("estimate_ms");
                        }
                    }
                }
            }
        }
    }
    Ok(crate::evidence::canonical::digest(&value)
        .map_err(|_| fail("replay_report_changed", "cannot hash report projection"))?
        .as_str()
        .into())
}

/// Resolve a portable relative path without links, traversal, devices or root escape.
pub fn safe_path(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.is_empty()
        || relative.len() > 4096
        || relative.contains('\\')
        || relative.contains(':')
        || relative.starts_with('/')
        || relative
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(fail("unsafe_path", "invalid pack-relative path"));
    }
    let mut path = root.to_path_buf();
    for part in Path::new(relative).components() {
        if !matches!(part, Component::Normal(_)) {
            return Err(fail("unsafe_path", "pack path traversal"));
        }
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.is_symlink() || !(m.is_dir() || m.is_file()) => {
                return Err(fail("unsafe_path", "pack path is a link or special file"));
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(fail("replay_io", "cannot inspect pack path")),
        }
    }
    Ok(path)
}
/// Verify structure and each exact file before any operation or native loading.
pub fn verify_files(root: &Path, pack: &Pack) -> Result<()> {
    if pack.schema != PACK_SCHEMA {
        return Err(fail("replay_schema_changed", "unsupported pack schema"));
    }
    if pack.files.is_empty() || pack.files.len() > MAX_FILES || pack.measurement_exit > 1 {
        return Err(fail("replay_pack_changed", "invalid pack bounds"));
    }
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for f in &pack.files {
        if !names.insert(&f.path) || !valid_hash(&f.sha256) || f.bytes > MAX_FILE_BYTES {
            return Err(fail("replay_pack_changed", "invalid file inventory"));
        }
        total = total.saturating_add(f.bytes);
        if total > MAX_PACK_BYTES {
            return Err(fail("replay_pack_changed", "pack exceeds byte bound"));
        }
        let path = safe_path(root, &f.path)?;
        if !path.is_file() {
            return Err(fail(
                if matches!(f.role, Role::Model | Role::Runtime) {
                    "not_reproducible_here"
                } else {
                    f.role.changed()
                },
                format!("recorded {:?} file missing: {}", f.role, f.path),
            ));
        }
        let data = read_bounded(&path, f.bytes).map_err(|_| {
            fail(
                f.role.changed(),
                format!("file size/read changed: {}", f.path),
            )
        })?;
        if data.len() as u64 != f.bytes || digest(&data) != f.sha256 {
            return Err(fail(
                f.role.changed(),
                format!("file content changed: {}", f.path),
            ));
        }
    }
    // Input membership is part of the finding: added images/sidecars are drift too.
    fn scan(
        root: &Path,
        relative: &str,
        names: &BTreeSet<&String>,
        visited: &mut usize,
        depth: usize,
    ) -> Result<()> {
        *visited += 1;
        if *visited > MAX_FILES || depth > 32 {
            return Err(fail("replay_input_changed", "input tree exceeds bound"));
        }
        let path = safe_path(root, relative)?;
        if path.is_dir() {
            for entry in
                std::fs::read_dir(path).map_err(|_| fail("replay_io", "cannot inspect inputs"))?
            {
                let entry = entry.map_err(|_| fail("replay_io", "cannot inspect input entry"))?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| fail("unsafe_path", "non-UTF-8 input path"))?;
                scan(
                    root,
                    &format!("{relative}/{name}"),
                    names,
                    visited,
                    depth + 1,
                )?;
            }
        } else if !names.contains(&relative.to_owned()) {
            return Err(fail("replay_input_changed", "unrecorded input file added"));
        }
        Ok(())
    }
    if root.join("inputs").exists() {
        scan(root, "inputs", &names, &mut 0, 0)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn pack(file: File) -> Pack {
        Pack {
            schema: PACK_SCHEMA.into(),
            files: vec![file],
            models: vec![],
            argv: vec![],
            report: "report.json".into(),
            report_id: "sha256:test".into(),
            comparison_id: "sha256:test".into(),
            measurement_exit: 0,
            platform: "test".into(),
        }
    }
    #[test]
    fn report_projection_ignores_compute_duration_but_keeps_performance_evidence() {
        let mut report = serde_json::json!({"schema":"saccade-report.v2","entries":[{"paths":{"baseline":"a"},"diagnostics":{"elapsed_ms":12.,"shift":{"estimate_ms":2.,"score":0.3}}}],"perf_diff":{"elapsed_ms":5.}});
        let original = comparison_id(&report).unwrap();
        report["entries"][0]["diagnostics"]["elapsed_ms"] = 50.0.into();
        report["entries"][0]["diagnostics"]["shift"]["estimate_ms"] = 8.0.into();
        assert_eq!(comparison_id(&report).unwrap(), original);
        report["perf_diff"]["elapsed_ms"] = 6.0.into();
        assert_ne!(comparison_id(&report).unwrap(), original);
    }
    #[test]
    fn missing_optional_artifacts_never_verify() {
        let dir = tempfile::tempdir().unwrap();
        for role in [Role::Model, Role::Runtime] {
            let p = pack(File {
                path: "missing".into(),
                bytes: 4,
                sha256: digest(b"data"),
                role,
            });
            assert_eq!(
                verify_files(dir.path(), &p).unwrap_err().code,
                "not_reproducible_here"
            );
            std::fs::write(dir.path().join("missing"), b"drift").unwrap();
            assert_eq!(
                verify_files(dir.path(), &p).unwrap_err().code,
                role.changed()
            );
            std::fs::remove_file(dir.path().join("missing")).unwrap();
        }
    }
    #[test]
    fn paths_and_inventory_are_bounded() {
        let dir = tempfile::tempdir().unwrap();
        for path in ["../x", "/x", "a/../../x", "a\\x", "C:/x", "a//x", "./x"] {
            assert!(safe_path(dir.path(), path).is_err());
        }
        let f = File {
            path: "x".into(),
            bytes: MAX_FILE_BYTES + 1,
            sha256: digest(b"x"),
            role: Role::Input,
        };
        assert_eq!(
            verify_files(dir.path(), &pack(f)).unwrap_err().code,
            "replay_pack_changed"
        );
    }
    #[test]
    fn scalar_configuration_and_ocr_selection_are_explicit() {
        let mut recipe = Recipe {
            schema: RECIPE_SCHEMA.into(),
            run: Operation::Compare {
                a: "a".into(),
                b: "b".into(),
                threshold: 0.01,
                metric: "mean".into(),
                ppd: 67.,
            },
        };
        assert!(recipe.validate().is_ok());
        if let Operation::Compare { threshold, .. } = &mut recipe.run {
            *threshold = 1.1;
        }
        assert_eq!(recipe.validate().unwrap_err().code, "replay_config_changed");
        recipe.run = Operation::Text {
            a: "a".into(),
            b: "b".into(),
            a_source: None,
            b_source: None,
            ocr: None,
            expect_text: vec![],
            readable_confidence: 80.,
            moved_px: 3.,
        };
        assert!(recipe.validate().is_err());
    }
}
