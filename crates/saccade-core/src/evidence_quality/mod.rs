//! Opt-in rendering evidence: occupancy, structure, layers and temporal uncertainty.
pub mod effect;
pub mod layers;

use crate::{Error, Result};
use std::io::Read;
use std::path::{Component, Path};

/// Read a regular file with a strict retained-byte limit.
pub fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(crate::run::io_err(format!("reading {}", path.display())))?;
    if !meta.file_type().is_file() || meta.len() > limit {
        return Err(Error::Config(
            "evidence input must be a bounded regular file".into(),
        ));
    }
    let file = std::fs::File::open(path).map_err(crate::run::io_err("opening evidence".into()))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(crate::run::io_err("reading evidence".into()))?;
    if bytes.len() as u64 > limit {
        return Err(Error::Config("evidence exceeds byte limit".into()));
    }
    Ok(bytes)
}

/// Resolve a sidecar below its declared parent, rejecting traversal and escaped links.
pub fn relative(root: &Path, name: &str) -> Result<std::path::PathBuf> {
    if name.is_empty()
        || Path::new(name)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(Error::Config(
            "evidence path must be relative without traversal".into(),
        ));
    }
    let path = root.join(name);
    let canon = crate::paths::canonicalize(&path)
        .map_err(crate::run::io_err("resolving evidence".into()))?;
    let parent = crate::paths::canonicalize(root)
        .map_err(crate::run::io_err("resolving evidence root".into()))?;
    if !canon.starts_with(parent) {
        return Err(Error::Config("evidence path escapes its parent".into()));
    }
    Ok(canon)
}

/// Decode from bounded retained bytes, with a pixel allocation limit.
pub fn image(path: &Path) -> Result<image::DynamicImage> {
    let bytes = read(path, 128 << 20)?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(crate::run::io_err("recognizing evidence image".into()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 << 20);
    reader.limits(limits);
    let img = reader.decode().map_err(|source| Error::Decode {
        path: path.into(),
        source,
    })?;
    if u64::from(img.width()) * u64::from(img.height()) > 16_777_216 {
        return Err(Error::Config("evidence image exceeds pixel limit".into()));
    }
    Ok(img)
}
