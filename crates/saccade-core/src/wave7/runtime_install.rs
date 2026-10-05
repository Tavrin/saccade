//! Pinned CPU runtime provisioning shared by CLI and future container/wheel consumers.
use super::models::{self, Artifact, Result, VisionError};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

/// Runtime version compiled against by ort rc.10.
pub const REQUIRED_VERSION: &str = "1.22.x (API 22)";
/// Frozen official release and inner-file integrity manifest.
#[derive(Debug, Deserialize, Serialize)]
pub struct RuntimePin {
    /// Exact official release.
    pub version: String,
    /// Supported archive platform.
    pub platform: String,
    /// Official archive licence.
    pub license: String,
    /// Release URL, never a latest alias.
    pub url: String,
    /// Exact archive bytes.
    pub bytes: u64,
    /// Locally computed archive hash.
    pub sha256: String,
    /// Where the digest was measured.
    pub hash_provenance: String,
    /// Allowlisted regular files; archive links are never extracted.
    pub files: Vec<RuntimeFile>,
}
/// One exact file inside the release archive.
#[derive(Debug, Deserialize, Serialize)]
pub struct RuntimeFile {
    /// Reviewed relative release path.
    pub path: String,
    /// Exact uncompressed bytes.
    pub bytes: u64,
    /// SHA-256 of uncompressed bytes.
    pub sha256: String,
}
/// The currently supported official release archive.
pub fn pin() -> Result<RuntimePin> {
    Ok(serde_json::from_str(include_str!(
        "../../assets/wave7-runtime.json"
    ))?)
}
fn supported() -> Result<()> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err(VisionError::Unavailable(format!(
            "runtime pull supports linux-x64; provide an ONNX Runtime {REQUIRED_VERSION} library for this platform"
        )));
    }
    Ok(())
}
fn artifact(p: &RuntimePin) -> Artifact {
    Artifact {
        role: "runtime-archive".into(),
        revision: format!("v{}", p.version),
        url: p.url.clone(),
        bytes: p.bytes,
        sha256: p.sha256.clone(),
        license: p.license.clone(),
        hash_provenance: Some(p.hash_provenance.clone()),
    }
}
fn directory(cache: &Path, p: &RuntimePin) -> PathBuf {
    cache.join("runtime").join(&p.sha256)
}
fn verify_directory(dir: &Path, p: &RuntimePin) -> Result<PathBuf> {
    for f in &p.files {
        let path = dir.join(&f.path);
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.len() != f.bytes {
            return Err(VisionError::Integrity(format!(
                "runtime file type/size: {}",
                f.path
            )));
        }
        if models::digest(&models::read_bounded(&path, f.bytes)?) != f.sha256 {
            return Err(VisionError::Integrity(format!(
                "runtime file hash: {}",
                f.path
            )));
        }
    }
    Ok(dir.join("lib/libonnxruntime.so.1.22.0"))
}
/// Verify cached runtime files without network access or loading a library.
pub fn cached(cache: &Path) -> Result<PathBuf> {
    supported()?;
    let p = pin()?;
    verify_directory(&directory(cache, &p), &p)
}
/// Provision the exact archive, preserving licence/notices and verifying every extracted file.
/// This is an explicit download operation. It never runs for ordinary inference.
pub fn pull(cache: &Path) -> Result<PathBuf> {
    use fs2::FileExt;
    supported()?;
    let p = pin()?;
    std::fs::create_dir_all(cache)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(cache.join("wave7.lock"))?;
    lock.lock_exclusive()?;
    let dir = directory(cache, &p);
    if dir.exists() {
        return verify_directory(&dir, &p);
    }
    let a = artifact(&p);
    let archive = models::artifact_path(cache, &a)?;
    if archive.exists() {
        models::verify(cache, &a)?;
    } else {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(300)))
            .build()
            .new_agent();
        let mut response = agent.get(&p.url).call().map_err(|_| {
            VisionError::Unavailable("official runtime archive download failed".into())
        })?;
        models::install(cache, &a, response.body_mut().as_reader())?;
    }
    let parent = cache.join("runtime");
    std::fs::create_dir_all(&parent)?;
    let temporary = tempfile::tempdir_in(&parent)?;
    extract(&archive, temporary.path(), &p)?;
    verify_directory(temporary.path(), &p)?;
    std::fs::rename(temporary.path(), &dir)?;
    verify_directory(&dir, &p)
}
fn extract(archive: &Path, dir: &Path, p: &RuntimePin) -> Result<()> {
    // Only copy allowlisted regular members; no tar unpack, links or archive permissions.
    let input = flate2::read::GzDecoder::new(std::fs::File::open(archive)?);
    let mut tar = tar::Archive::new(input.take(64 * 1024 * 1024));
    let prefix = format!("onnxruntime-linux-x64-{}/", p.version);
    let mut seen = std::collections::BTreeSet::new();
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let Some(path) = path.to_str().and_then(|s| s.strip_prefix(&prefix)) else {
            continue;
        };
        let Some(f) = p.files.iter().find(|f| f.path == path) else {
            continue;
        };
        if !entry.header().entry_type().is_file()
            || entry.size() != f.bytes
            || !seen.insert(f.path.clone())
        {
            return Err(VisionError::Integrity(
                "runtime archive member type/size/duplicate".into(),
            ));
        }
        let destination = dir.join(&f.path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;
        if std::io::copy(&mut entry, &mut output)? != f.bytes {
            return Err(VisionError::Integrity("runtime archive truncated".into()));
        }
        output.sync_all()?;
    }
    if seen.len() != p.files.len() {
        return Err(VisionError::Integrity(
            "runtime archive required member missing".into(),
        ));
    }
    Ok(())
}
/// Select an explicit CLI path, ORT_DYLIB_PATH, or the verified provisioned runtime.
/// Never mutates the process environment and never downloads implicitly.
pub fn resolve(explicit: Option<&Path>, cache: &Path) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_owned());
    }
    if let Some(path) = std::env::var_os("ORT_DYLIB_PATH") {
        return Ok(path.into());
    }
    cached(cache).map_err(|e| VisionError::Unavailable(format!(
        "ONNX Runtime {REQUIRED_VERSION} needed; use models pull runtime, --runtime-library or ORT_DYLIB_PATH ({e})"
    )))
}
/// Additive list status; no runtime is loaded.
pub fn status(cache: &Path) -> serde_json::Value {
    match pin() {
        Ok(p) => {
            serde_json::json!({"pin":p,"status":if cached(cache).is_ok() {"cached_verified"} else {"missing_or_corrupt"}})
        }
        Err(e) => serde_json::json!({"status":"unavailable","reason":e.to_string()}),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn cached_runtime_detects_corruption_and_never_fetches() {
        let cache = tempfile::tempdir().unwrap();
        assert!(cached(cache.path()).is_err());
        assert_eq!(std::fs::read_dir(cache.path()).unwrap().count(), 0);
        let mut p = pin().unwrap();
        p.files = vec![RuntimeFile {
            path: "lib/libonnxruntime.so.1.22.0".into(),
            bytes: 3,
            sha256: models::digest(b"lib"),
        }];
        let dir = directory(cache.path(), &p);
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        std::fs::write(dir.join(&p.files[0].path), b"lib").unwrap();
        verify_directory(&dir, &p).unwrap();
        std::fs::write(dir.join(&p.files[0].path), b"bad").unwrap();
        assert!(matches!(
            verify_directory(&dir, &p),
            Err(VisionError::Integrity(_))
        ));
    }
    #[test]
    fn extraction_requires_exact_regular_allowlisted_members() {
        use std::io::Write;
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("archive");
        let mut p = pin().unwrap();
        p.files = vec![RuntimeFile {
            path: "LICENSE".into(),
            bytes: 3,
            sha256: models::digest(b"MIT"),
        }];
        let gz = flate2::write::GzEncoder::new(
            std::fs::File::create(&archive).unwrap(),
            flate2::Compression::fast(),
        );
        let mut tar = tar::Builder::new(gz);
        let mut header = tar::Header::new_gnu();
        header.set_size(3);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(
            &mut header,
            "onnxruntime-linux-x64-1.22.0/LICENSE",
            &b"MIT"[..],
        )
        .unwrap();
        tar.into_inner().unwrap().finish().unwrap().flush().unwrap();
        let out = temp.path().join("out");
        std::fs::create_dir(&out).unwrap();
        extract(&archive, &out, &p).unwrap();
        assert_eq!(std::fs::read(out.join("LICENSE")).unwrap(), b"MIT");
        p.files[0].bytes = 4;
        assert!(matches!(
            extract(&archive, &out, &p),
            Err(VisionError::Integrity(_))
        ));
    }
}
