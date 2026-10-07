//! Bounded z/x/y tile inventories and shared perceptual comparisons.
use crate::{Error, Result, input};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// Tile-set report discriminator.
pub const SCHEMA: &str = "saccade-tiles.v1";
/// Maximum inventory entries, including directories and unrelated files.
pub const MAX_ENTRIES: usize = 200_000;
/// Maximum decoded tile area.
pub const MAX_TILE_PIXELS: u64 = 16_000_000;
type Key = (u32, u32, u32);
fn inventory(root: &Path) -> Result<BTreeMap<Key, PathBuf>> {
    if !root.is_dir() || root.is_symlink() {
        return Err(Error::Input(
            "tile input must be a directory without symlinks".into(),
        ));
    }
    let mut result = BTreeMap::new();
    for (count, entry) in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .enumerate()
    {
        if count >= MAX_ENTRIES {
            return Err(Error::Input("tile inventory exceeds 200000 entries".into()));
        }
        let entry = entry.map_err(|e| Error::Input(e.to_string()))?;
        if entry.file_type().is_symlink() {
            return Err(Error::Input("tile tree symlinks are refused".into()));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let extension = entry
            .path()
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp") {
            return Err(Error::Input(
                "tile trees may contain only PNG/JPEG/WebP tiles".into(),
            ));
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| Error::Input(e.to_string()))?;
        let parts: Vec<_> = rel.iter().collect();
        if parts.len() != 3 {
            return Err(Error::Input("tiles require z/x/y.ext paths".into()));
        }
        let parse = |s: &std::ffi::OsStr| -> Result<u32> {
            let text = s
                .to_str()
                .ok_or_else(|| Error::Input("tile coordinates must be UTF-8 numbers".into()))?;
            let n = text
                .parse::<u32>()
                .map_err(|_| Error::Input("tile coordinates must be unsigned integers".into()))?;
            if text != n.to_string() {
                return Err(Error::Input(
                    "tile coordinates must use canonical decimal spelling".into(),
                ));
            }
            Ok(n)
        };
        let z = parse(parts[0])?;
        let x = parse(parts[1])?;
        let y = parse(
            entry
                .path()
                .file_stem()
                .ok_or_else(|| Error::Input("tile has no stem".into()))?,
        )?;
        if z > 30 || x >= 1u32 << z || y >= 1u32 << z {
            return Err(Error::Input(
                "tile coordinates are outside their zoom grid".into(),
            ));
        }
        if result
            .insert((z, x, y), entry.path().to_path_buf())
            .is_some()
        {
            return Err(Error::Input(
                "duplicate tile coordinate, including mixed extensions".into(),
            ));
        }
    }
    if result.is_empty() {
        return Err(Error::Input("empty tile tree".into()));
    }
    Ok(result)
}
fn decode(path: &Path) -> Result<(image::RgbaImage, String)> {
    let bytes = input::bytes(path, 64 * 1024 * 1024)?;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format()?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP)
    ) {
        return Err(Error::Input("tile content is not PNG/JPEG/WebP".into()));
    }
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    reader.limits(limits);
    // Header dimensions are checked before allocating the decoded image.
    let dimensions = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()?
        .into_dimensions()?;
    if u64::from(dimensions.0) * u64::from(dimensions.1) > MAX_TILE_PIXELS {
        return Err(Error::Input("tile exceeds 16 million pixels".into()));
    }
    Ok((
        reader.decode()?.to_rgba8(),
        saccade_core::localized::digest(&bytes),
    ))
}
/// Compare two tile trees without rendering or fetching; coverage is relative to reference.
/// A tile is changed when native decoded RGBA bytes differ; FLIP remains a measurement.
pub fn compare(reference: &Path, candidate: &Path, out: &Path) -> Result<Value> {
    let a = inventory(reference)?;
    let b = inventory(candidate)?;
    crate::prepare_out(out, &[reference, candidate])?;
    let zooms: BTreeSet<_> = a.keys().chain(b.keys()).map(|k| k.0).collect();
    let mut coverage = Vec::new();
    for z in zooms {
        let missing: Vec<_> = a
            .keys()
            .filter(|k| k.0 == z && !b.contains_key(k))
            .map(|k| format!("{}/{}/{}", k.0, k.1, k.2))
            .collect();
        let extra: Vec<_> = b
            .keys()
            .filter(|k| k.0 == z && !a.contains_key(k))
            .map(|k| format!("{}/{}/{}", k.0, k.1, k.2))
            .collect();
        coverage.push(json!({"zoom":z,"reference_tiles":a.keys().filter(|k|k.0==z).count(),"candidate_tiles":b.keys().filter(|k|k.0==z).count(),"missing":missing,"extra":extra}));
    }
    let mut tiles = Vec::new();
    let mut changed = 0;
    for (key, apath) in &a {
        let Some(bpath) = b.get(key) else {
            continue;
        };
        let (aa, ahash) = decode(apath)?;
        let (bb, bhash) = decode(bpath)?;
        let comparison = saccade_core::compare::compare_rgba(&bb, &aa, &Default::default())?;
        let same = aa == bb;
        if !same {
            changed += 1;
        }
        let id = format!("{}/{}/{}", key.0, key.1, key.2);
        let heatmap = format!("tiles/{id}-flip.png");
        let path = out.join(&heatmap);
        let parent = path
            .parent()
            .ok_or_else(|| Error::Input("invalid tile output path".into()))?;
        std::fs::create_dir_all(parent)?;
        comparison.heatmap_rgb().save(path)?;
        tiles.push(json!({"tile":id,"reference":apath.strip_prefix(reference).map_err(|e|Error::Input(e.to_string()))?,"candidate":bpath.strip_prefix(candidate).map_err(|e|Error::Input(e.to_string()))?,"reference_sha256":ahash,"candidate_sha256":bhash,"decoded_identical":same,"metrics":comparison.metrics,"heatmap":heatmap}));
    }
    // Decode unmatched tiles too: corrupt/mislabeled coverage cannot masquerade as valid evidence.
    for (key, path) in &a {
        if !b.contains_key(key) {
            decode(path)?;
        }
    }
    for (key, path) in &b {
        if !a.contains_key(key) {
            decode(path)?;
        }
    }
    let identical = changed == 0 && a.keys().eq(b.keys());
    Ok(saccade_core::report_links::decorate(
        &json!({"schema":SCHEMA,"operation":"tile_compare","verdict":if identical {"identical"} else {"changed"},"policy":{"coverage":"coordinate keys, independent of codec","comparison":"shared RGBA FLIP (alpha over black and white)","grid":"z/x/y; no rendering, network or resampling"},"coverage":coverage,"compared_tiles":tiles.len(),"changed_tiles":changed,"tiles":tiles}),
    )?)
}
