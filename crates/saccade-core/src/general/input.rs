//! Bounded raster input; documents are rejected until a licensed renderer is available.
use crate::{Error, Result};
use std::{io::Read, path::Path};
/// Maximum encoded input size (64 MiB).
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum decoded raster size (16 million pixels).
pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
/// Reads an untrusted regular file with a streaming bound, including growth after stat.
pub fn bytes(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file =
        std::fs::File::open(path).map_err(crate::run::io_err("opening image input".into()))?;
    if !file
        .metadata()
        .map_err(crate::run::io_err("image metadata".into()))?
        .is_file()
    {
        return Err(Error::Config("input must be a regular file".into()));
    }
    let mut out = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut out)
        .map_err(crate::run::io_err("reading bounded image input".into()))?;
    if out.len() as u64 > limit {
        return Err(Error::Config("input exceeds byte limit".into()));
    }
    Ok(out)
}
/// Decodes one bounded image, rejecting HDR conversion in these SDR-only tools.
pub fn decode(encoded: &[u8]) -> Result<image::RgbaImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(encoded))
        .with_guessed_format()
        .map_err(crate::run::io_err("guessing raster format".into()))?;
    if matches!(
        reader.format(),
        Some(image::ImageFormat::OpenExr | image::ImageFormat::Hdr)
    ) {
        return Err(Error::Config(
            "general-image tools require SDR input; use HDR-FLIP for HDR".into(),
        ));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(MAX_PIXELS * 8);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|e| Error::Config(format!("raster decode: {e}")))?;
    if image.width() == 0
        || image.height() == 0
        || u64::from(image.width()) * u64::from(image.height()) > MAX_PIXELS
    {
        return Err(Error::Config("empty raster or pixel limit exceeded".into()));
    }
    if image.color() != image::ColorType::Rgb8
        && image.color() != image::ColorType::Rgba8
        && image.color() != image::ColorType::L8
        && image.color() != image::ColorType::La8
    {
        return Err(Error::Config(
            "general-image tools require 8-bit input; native-depth comparison remains available"
                .into(),
        ));
    }
    Ok(image.to_rgba8())
}
/// Reads and decodes one raster exactly once.
pub fn load(path: &Path) -> Result<image::RgbaImage> {
    decode(&bytes(path, MAX_BYTES)?)
}

/// Lists supported raster/document candidates without following symlinks, bounded by visited entries.
/// Traversal failures and symlinks fail closed; unreadable captures cannot disappear from results.
pub fn files(root: &Path, max: usize) -> Result<Vec<std::path::PathBuf>> {
    if !root.is_dir() {
        return Err(Error::Config("input must be a directory".into()));
    }
    let mut out = Vec::new();
    for (count, item) in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .enumerate()
    {
        if count > max.saturating_mul(4) {
            return Err(Error::Config("directory traversal limit exceeded".into()));
        }
        let item = item.map_err(|e| Error::Config(format!("directory traversal: {e}")))?;
        if item.path_is_symlink() {
            return Err(Error::Config("symlinks are not followed".into()));
        }
        if item.file_type().is_file()
            && item
                .path()
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| {
                    matches!(
                        s.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "svg" | "pdf"
                    )
                })
        {
            if out.len() >= max {
                return Err(Error::Config("image count limit exceeded".into()));
            }
            if item.path().as_os_str().len() > 4096 {
                return Err(Error::Config("path length limit exceeded".into()));
            }
            out.push(item.into_path());
        }
    }
    out.sort();
    Ok(out)
}

/// SHA-256 with a streaming byte bound; a growing untrusted file cannot make reads unbounded.
pub fn sha256(path: &Path, limit: u64) -> Result<String> {
    use sha2::{Digest, Sha256};
    let file = std::fs::File::open(path)
        .map_err(crate::run::io_err("opening bounded digest input".into()))?;
    let mut reader = file.take(limit.saturating_add(1));
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    let mut count = 0u64;
    loop {
        let n = reader
            .read(&mut buffer)
            .map_err(crate::run::io_err("hashing bounded input".into()))?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > limit {
            return Err(Error::Config("digest input exceeds byte limit".into()));
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
