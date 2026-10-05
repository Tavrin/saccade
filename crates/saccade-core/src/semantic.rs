//! Optional text-region model plumbing; frozen mask import remains model-independent.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One pinned runtime-downloaded model, graph, tokenizer or parity artifact.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelArtifact {
    /// Unique role, such as grounding_dino, sam_encoder or sam_decoder.
    pub role: String,
    /// Exact checkpoint/export version.
    pub version: String,
    /// checkpoint, onnx, tokenizer or parity.
    pub format: String,
    /// Revision-pinned HTTPS download URL.
    pub url: String,
    /// Exact expected encoded bytes.
    pub bytes: u64,
    /// SHA-256 of artifact bytes.
    pub sha256: String,
    /// Commercial-safe reviewed license, Apache-2.0 or MIT.
    pub license: String,
}
/// Checkpoint/export provenance; a known checkpoint is not a qualified ONNX export.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelManifest {
    /// saccade-region-models.v1.
    pub schema: String,
    /// Explicit qualification state, not inferred from graph loading.
    pub qualification: String,
    /// Pinned runtime API version.
    pub runtime: String,
    /// Original preprocessing and tokenizer specification/version.
    pub preprocessing: String,
    /// CPU f32 for this initial runtime boundary.
    pub execution_provider: String,
    /// Pinned artifacts, including checkpoint/export hashes and licenses.
    pub artifacts: Vec<ModelArtifact>,
    /// Explicit missing parity/export conditions.
    pub residuals: Vec<String>,
}
fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
/// Validates a manifest without downloading or loading models.
pub fn validate(manifest: &ModelManifest) -> Result<()> {
    let mut roles = std::collections::BTreeSet::new();
    if manifest.schema != "saccade-region-models.v1"
        || manifest.runtime != "ONNX Runtime 1.22"
        || manifest.execution_provider != "CPU f32"
        || manifest.preprocessing.is_empty()
        || manifest.artifacts.is_empty()
        || manifest.artifacts.len() > 16
        || manifest.qualification.is_empty()
    {
        return Err(Error::Config(
            "invalid model manifest or unsupported runtime/provider".into(),
        ));
    }
    for a in &manifest.artifacts {
        if a.role.is_empty()
            || !roles.insert(&a.role)
            || a.version.is_empty()
            || !matches!(
                a.format.as_str(),
                "checkpoint" | "onnx" | "tokenizer" | "parity"
            )
            || !a.url.starts_with("https://")
            || a.bytes == 0
            || a.bytes > 1024 * 1024 * 1024
            || !valid_hash(&a.sha256)
            || !matches!(a.license.as_str(), "Apache-2.0" | "MIT")
        {
            return Err(Error::Config(
                "invalid artifact identity, size, URL or license".into(),
            ));
        }
    }
    Ok(())
}
/// Content-addressed local artifact path. No model bytes are committed.
pub fn artifact_path(cache: &Path, artifact: &ModelArtifact) -> Result<PathBuf> {
    if !valid_hash(&artifact.sha256) {
        return Err(Error::Config("invalid model digest".into()));
    }
    Ok(cache.join(&artifact.sha256))
}

#[cfg(feature = "semantic-regions")]
fn verify_file(path: &Path, a: &ModelArtifact) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let file =
        std::fs::File::open(path).map_err(crate::run::io_err("opening cached model".into()))?;
    if file
        .metadata()
        .map_err(crate::run::io_err("cached model metadata".into()))?
        .len()
        != a.bytes
    {
        return Err(Error::Config("cached model size mismatch".into()));
    }
    let mut reader = file.take(a.bytes + 1);
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = reader
            .read(&mut buffer)
            .map_err(crate::run::io_err("reading cached model".into()))?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    if format!("{:x}", hash.finalize()) != a.sha256 {
        return Err(Error::Config("cached model hash mismatch".into()));
    }
    Ok(())
}
#[cfg(feature = "semantic-regions")]
fn install(cache: &Path, a: &ModelArtifact, reader: impl std::io::Read) -> Result<PathBuf> {
    use std::io::Write;
    let mut temporary = tempfile::NamedTempFile::new_in(cache)
        .map_err(crate::run::io_err("creating model cache temporary".into()))?;
    let written = std::io::copy(&mut reader.take(a.bytes + 1), &mut temporary)
        .map_err(crate::run::io_err("downloading model".into()))?;
    if written != a.bytes {
        return Err(Error::Config("download size mismatch".into()));
    }
    temporary
        .flush()
        .map_err(crate::run::io_err("flushing model cache".into()))?;
    verify_file(temporary.path(), a)?;
    let path = artifact_path(cache, a)?;
    temporary
        .persist_noclobber(&path)
        .map_err(|e| Error::Config(format!("persisting model: {e}")))?;
    Ok(path)
}
/// Explicit runtime download. Ordinary comparisons and imports never call this.
#[cfg(feature = "semantic-regions")]
pub fn cache_models(manifest: &ModelManifest, cache: &Path) -> Result<Vec<PathBuf>> {
    use fs2::FileExt;
    validate(manifest)?;
    std::fs::create_dir_all(cache).map_err(crate::run::io_err("creating model cache".into()))?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(cache.join("cache.lock"))
        .map_err(crate::run::io_err("model cache lock".into()))?;
    lock.lock_exclusive()
        .map_err(crate::run::io_err("locking model cache".into()))?;
    let mut paths = vec![];
    for a in &manifest.artifacts {
        let path = artifact_path(cache, a)?;
        if path.exists() {
            verify_file(&path, a)?;
            paths.push(path);
            continue;
        }
        let mut response = ureq::get(&a.url)
            .call()
            .map_err(|e| Error::Config(format!("model download: {e}")))?;
        paths.push(install(cache, a, response.body_mut().as_reader())?);
    }
    Ok(paths)
}
/// Loads self-contained cached ONNX graphs via optional Rust ort. This proves
/// graph loading only, not preprocessing, inference or source-model parity.
#[cfg(feature = "semantic-regions")]
pub fn probe_runtime(
    manifest: &ModelManifest,
    cache: &Path,
    library: &Path,
) -> Result<Vec<String>> {
    validate(manifest)?;
    let required = ["grounding_dino", "sam_encoder", "sam_decoder"];
    if required.iter().any(|role| {
        !manifest
            .artifacts
            .iter()
            .any(|a| a.role == *role && a.format == "onnx")
    }) {
        return Err(Error::Config("checkpoint-specific ONNX detector/encoder/decoder exports are unavailable; import a frozen mask".into()));
    }
    if !library.is_file() {
        return Err(Error::Config(
            "ONNX Runtime 1.22 library unavailable; set an explicit library path".into(),
        ));
    }
    // ort's dynamic loader may panic on missing symbols or incompatible ABI.
    // Keep that optional boundary a capability error instead of aborting the CLI.
    std::panic::catch_unwind(|| -> Result<Vec<String>> {
        ort::init_from(library.display().to_string())
            .commit()
            .map_err(|e| Error::Config(e.to_string()))?;
        let mut loaded = vec![];
        for a in manifest.artifacts.iter().filter(|a| a.format == "onnx") {
            let path = artifact_path(cache, a)?;
            verify_file(&path, a)?;
            let _session = ort::session::Session::builder()
                .map_err(|e| Error::Config(e.to_string()))?
                .with_intra_threads(1)
                .map_err(|e| Error::Config(e.to_string()))?
                .commit_from_file(path)
                .map_err(|e| Error::Config(e.to_string()))?;
            loaded.push(a.role.clone());
        }
        Ok(loaded)
    })
    .map_err(|_| Error::Config("ONNX runtime dynamic ABI/load failure".into()))?
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn checkpoint_manifest_is_pinned_and_does_not_claim_export_parity() {
        let manifest: ModelManifest =
            serde_json::from_str(include_str!("../models/semantic-regions.json")).unwrap();
        validate(&manifest).unwrap();
        assert_eq!(
            manifest.qualification,
            "checkpoint_only_export_and_parity_unqualified"
        );
        assert!(manifest.artifacts.iter().all(|a| a.format == "checkpoint"));
        let mut bad = manifest.clone();
        bad.artifacts[0].license = "SAM License".into();
        assert!(validate(&bad).is_err());
        bad = manifest;
        bad.artifacts[0].sha256 = "../escape".into();
        assert!(artifact_path(Path::new("cache"), &bad.artifacts[0]).is_err());
    }
    #[cfg(feature = "semantic-regions")]
    #[test]
    fn cache_rejects_corruption_and_runtime_without_qualified_exports() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"synthetic model-shaped bytes";
        let artifact = ModelArtifact {
            role: "grounding_dino".into(),
            version: "synthetic".into(),
            format: "onnx".into(),
            url: "https://example.invalid/pinned.onnx".into(),
            bytes: bytes.len() as u64,
            sha256: crate::localized::digest(bytes),
            license: "MIT".into(),
        };
        let path = install(dir.path(), &artifact, &bytes[..]).unwrap();
        verify_file(&path, &artifact).unwrap();
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(verify_file(&path, &artifact).is_err());
        std::fs::remove_file(&path).unwrap();
        assert!(install(dir.path(), &artifact, &b"wrong"[..]).is_err());
        assert!(!path.exists());
        let manifest: ModelManifest =
            serde_json::from_str(include_str!("../models/semantic-regions.json")).unwrap();
        assert!(probe_runtime(&manifest, dir.path(), Path::new("missing-runtime.so")).is_err());
    }
}
