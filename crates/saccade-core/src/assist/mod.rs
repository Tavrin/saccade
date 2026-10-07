//! Experimental advisory observations; no authority-bearing decision inputs.
pub mod batch;
pub mod catalog;
pub mod execution;
pub mod geometry;
/// Recorded OpenRouter chat-completions adapter.
pub mod openrouter;
pub mod price;
pub mod routing;
pub mod schema;

use crate::evidence::canonical::{self, Digest};
use serde::{Serialize, de::DeserializeOwned};
use std::io::Read;
use std::path::Path;

/// Fail-closed assist errors. Provider text is never interpolated into diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Malformed, stale or unsupported evidence.
    #[error("assist invalid evidence: {0}")]
    Invalid(&'static str),
    /// Exhausted authorization, time, requests or monetary allowance.
    #[error("assist policy refused: {0}")]
    Policy(&'static str),
    /// Local storage error without untrusted content.
    #[error("assist storage unavailable")]
    Storage,
    /// Incomplete external execution; local comparison remains usable.
    #[error("assist provider execution incomplete")]
    Provider,
}
impl Error {
    /// Stable diagnostic without provider bodies, filesystem details or credentials.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(reason) | Self::Policy(reason) => reason,
            Self::Storage => "assist_storage_unavailable",
            Self::Provider => "assist_provider_execution_incomplete",
        }
    }
}
/// Assist result, distinct from comparison failures.
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn require(ok: bool, reason: &'static str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(reason))
    }
}
/// Canonical content identity; preserves arrays, exact text and numerical spelling.
pub fn digest<T: Serialize>(value: &T) -> Result<Digest> {
    canonical::digest(value).map_err(|_| Error::Invalid("noncanonical value"))
}
/// Bounded ordinary-file read; symlinks and oversized files are refused.
pub fn read_bytes(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| Error::Storage)?;
    require(
        metadata.is_file() && metadata.len() <= limit as u64,
        "unbounded or nonordinary input",
    )?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| Error::Storage)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Storage)?;
    require(bytes.len() <= limit, "input grew beyond limit")?;
    Ok(bytes)
}
/// Strict decoding rejects duplicate keys and unknown typed fields.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    require(bytes.len() <= 32 * 1024 * 1024, "unbounded JSON")?;
    canonical::decode(bytes).map_err(|_| Error::Invalid("closed schema or JSON violation"))
}
/// Atomically persists an artifact without following a destination symlink.
pub fn write<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    use std::io::Write;
    require(
        !std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()),
        "artifact symlink",
    )?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|_| Error::Storage)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|_| Error::Storage)?;
    temporary
        .write_all(
            &serde_json::to_vec_pretty(value)
                .map_err(|_| Error::Invalid("artifact serialization"))?,
        )
        .map_err(|_| Error::Storage)?;
    temporary.as_file().sync_all().map_err(|_| Error::Storage)?;
    temporary.persist(path).map_err(|_| Error::Storage)?;
    #[cfg(unix)]
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| Error::Storage)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

pub mod mask_audit;
pub mod workflow;

/// Fixture-only hosted vision mappings bound to immutable assist evidence.
#[cfg(feature = "vision-providers")]
pub mod vision_provider;
