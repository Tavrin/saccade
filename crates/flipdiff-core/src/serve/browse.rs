//! Read-only browsing of the archive root: safe path resolution, lazy
//! one-level listings, bounded searches and thumbnails.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

use super::State;

/// Image extensions the server lists and serves from the archive.
pub(crate) const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "exr", "hdr"];

/// Fake image name used to load a run's directory-level sidecar.
const RUN_PROBE: &str = ".flipdiff-run";
/// Longest time one `ls` spends probing sub-directories for images.
const LS_PROBE_BUDGET: Duration = Duration::from_millis(1500);
/// Entries read from one directory while probing it for an image.
const PROBE_ENTRIES: usize = 2000;
/// Image names returned for a run.
const RUN_NAMES: usize = 60;
/// Deepest directory a search descends to, below its start.
const SEARCH_DEPTH: usize = 8;
/// Largest number of runs a search returns.
const SEARCH_LIMIT: usize = 400;
/// Runs returned by a "recent" search.
const RECENT_LIMIT: usize = 30;
/// Longest edge of a thumbnail, in pixels.
const THUMB_EDGE: u32 = 320;
/// Largest image file a thumbnail is generated for.
const THUMB_MAX_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) fn is_image_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTS.iter().any(|k| e.eq_ignore_ascii_case(k)))
}

/// Why a client path was refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PathError {
    /// Absolute, `..`, NUL, or otherwise not a plain relative path.
    Invalid,
    /// Resolves (through symlinks) outside the base directory.
    Escapes,
    /// Does not exist.
    Missing,
}

/// Checks that `rel` is a plain relative path (normal components only); the
/// empty path means the base itself.
pub(crate) fn is_plain_rel(rel: &str) -> bool {
    !rel.contains('\0')
        && Path::new(rel)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

/// Resolves a client path below `base` (which must be canonical) and requires
/// the canonical result to stay below it, so symlinks cannot escape.
pub(crate) fn resolve_under(base: &Path, rel: &str) -> Result<PathBuf, PathError> {
    if !is_plain_rel(rel) {
        return Err(PathError::Invalid);
    }
    let canon = base.join(rel).canonicalize().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            PathError::Missing
        } else {
            PathError::Invalid
        }
    })?;
    if canon.starts_with(base) {
        Ok(canon)
    } else {
        Err(PathError::Escapes)
    }
}

/// `/`-separated path of `abs` relative to `base`.
pub(crate) fn rel_of(base: &Path, abs: &Path) -> String {
    abs.strip_prefix(base)
        .map(|r| {
            r.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default()
}

fn unix_secs(t: std::io::Result<SystemTime>) -> u64 {
    t.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// What a directory directly contains, image-wise.
#[derive(Default)]
struct Images {
    names: Vec<String>,
    bytes: u64,
}

fn scan_images(dir: &Path, with_sizes: bool) -> Images {
    let mut out = Images::default();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !is_image_name(&name) || !e.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        if with_sizes {
            out.bytes += e.metadata().map_or(0, |m| m.len());
        }
        out.names.push(name);
    }
    out.names.sort();
    out
}

/// The first image directly inside `dir`, reading at most [`PROBE_ENTRIES`]
/// entries.
fn probe_image(dir: &Path) -> Option<String> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut best: Option<String> = None;
    for e in rd.flatten().take(PROBE_ENTRIES) {
        let name = e.file_name().to_string_lossy().into_owned();
        if is_image_name(&name)
            && e.file_type().is_ok_and(|t| t.is_file())
            && best.as_ref().is_none_or(|b| name < *b)
        {
            best = Some(name);
        }
    }
    best
}

fn render(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The run-level sidecar (the directory-level file in the run), rendered as
/// strings, or the reason it could not be read.
fn run_meta(state: &State, run: &Path) -> (Option<BTreeMap<String, String>>, Option<String>) {
    match state.meta.load(run, RUN_PROBE) {
        Ok(Some(m)) => (
            Some(m.iter().map(|(k, v)| (k.clone(), render(v))).collect()),
            None,
        ),
        Ok(None) => (None, None),
        Err(e) => (None, Some(e)),
    }
}

/// One sub-directory in a listing.
#[derive(Serialize)]
pub(crate) struct DirItem {
    name: String,
    mtime: u64,
    /// `None` when the probe budget ran out before this directory was read.
    has_images: Option<bool>,
    /// Name of one image in the directory, for a thumbnail.
    sample: Option<String>,
}

/// The images of a directory that is a run.
#[derive(Serialize)]
pub(crate) struct RunInfo {
    images: usize,
    bytes: u64,
    names: Vec<String>,
    meta: Option<BTreeMap<String, String>>,
    meta_error: Option<String>,
}

/// Result of `GET /api/ls`.
#[derive(Serialize)]
pub(crate) struct Listing {
    path: String,
    root_name: String,
    parent: Option<String>,
    mtime: u64,
    dirs: Vec<DirItem>,
    run: Option<RunInfo>,
    partial: bool,
    /// Directory symlinks that point outside the root: never listed or followed.
    outside_links: usize,
}

/// Lists one directory level below the root.
pub(crate) fn list(state: &State, rel: &str) -> Result<Listing, PathError> {
    let dir = resolve_under(&state.root, rel)?;
    if !dir.is_dir() {
        return Err(PathError::Missing);
    }
    let rel = rel_of(&state.root, &dir);
    let imgs = scan_images(&dir, true);
    let run = (!imgs.names.is_empty()).then(|| {
        let (meta, meta_error) = run_meta(state, &dir);
        RunInfo {
            images: imgs.names.len(),
            bytes: imgs.bytes,
            names: imgs.names.iter().take(RUN_NAMES).cloned().collect(),
            meta,
            meta_error,
        }
    });

    let mut subs: Vec<(String, PathBuf, u64)> = Vec::new();
    let mut outside_links = 0usize;
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = e.path();
            // A symlinked directory is listed only when it stays inside the root.
            let ok = if ft.is_dir() {
                true
            } else if ft.is_symlink() {
                match path.canonicalize() {
                    Ok(c) if c.is_dir() && c.starts_with(&state.root) => true,
                    Ok(c) if c.is_dir() => {
                        outside_links += 1;
                        false
                    }
                    _ => false,
                }
            } else {
                false
            };
            if ok {
                let name = e.file_name().to_string_lossy().into_owned();
                let mtime = unix_secs(e.metadata().and_then(|m| m.modified()));
                subs.push((name, path, mtime));
            }
        }
    }
    subs.sort_by(|a, b| a.0.cmp(&b.0));
    let started = Instant::now();
    let mut partial = false;
    let dirs = subs
        .into_iter()
        .map(|(name, path, mtime)| {
            if started.elapsed() > LS_PROBE_BUDGET {
                partial = true;
                return DirItem {
                    name,
                    mtime,
                    has_images: None,
                    sample: None,
                };
            }
            let sample = probe_image(&path);
            DirItem {
                name,
                mtime,
                has_images: Some(sample.is_some()),
                sample,
            }
        })
        .collect();
    let parent = (!rel.is_empty()).then(|| rel.rsplit_once('/').map_or("", |(p, _)| p).to_owned());
    Ok(Listing {
        root_name: state
            .root
            .file_name()
            .map_or_else(|| "/".into(), |n| n.to_string_lossy().into_owned()),
        mtime: unix_secs(std::fs::metadata(&dir).and_then(|m| m.modified())),
        path: rel,
        parent,
        dirs,
        run,
        partial,
        outside_links,
    })
}

/// Filters of a search.
pub(crate) struct SearchQuery {
    /// Case-insensitive substring of the run path.
    pub text: String,
    /// Sidecar filter `key` (present) or `key=value`.
    pub meta: Option<(String, Option<String>)>,
    /// Return the most recently modified runs instead of the first found.
    pub recent: bool,
    /// Time budget.
    pub budget: Duration,
}

/// One run found by a search.
#[derive(Serialize)]
pub(crate) struct RunHit {
    path: String,
    mtime: u64,
    images: usize,
    sample: String,
    meta: Option<BTreeMap<String, String>>,
}

/// Result of `GET /api/search`.
#[derive(Serialize)]
pub(crate) struct SearchResult {
    runs: Vec<RunHit>,
    /// The walk stopped (budget, limit or depth) before covering the subtree.
    partial: bool,
    directories_scanned: usize,
}

/// A bounded breadth-first walk below `rel` for runs matching `q`.
pub(crate) fn search(state: &State, rel: &str, q: &SearchQuery) -> Result<SearchResult, PathError> {
    let start = resolve_under(&state.root, rel)?;
    let text = q.text.to_lowercase();
    let started = Instant::now();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::from([(start, 0)]);
    let mut hits: Vec<(u64, RunHit)> = Vec::new();
    let mut partial = false;
    let mut scanned = 0usize;
    while let Some((dir, depth)) = queue.pop_front() {
        if started.elapsed() > q.budget || (!q.recent && hits.len() >= SEARCH_LIMIT) {
            partial = true;
            break;
        }
        scanned += 1;
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut images = 0usize;
        let mut sample: Option<String> = None;
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            if ft.is_dir() {
                if name.starts_with('.') {
                    continue;
                }
                if depth < SEARCH_DEPTH {
                    queue.push_back((e.path(), depth + 1));
                } else {
                    partial = true;
                }
            } else if ft.is_file() && is_image_name(&name) {
                images += 1;
                if sample.as_ref().is_none_or(|s| name < *s) {
                    sample = Some(name);
                }
            }
        }
        let Some(sample) = sample else { continue };
        let run_rel = rel_of(&state.root, &dir);
        if !text.is_empty() && !run_rel.to_lowercase().contains(&text) {
            continue;
        }
        let (meta, _) = run_meta(state, &dir);
        if let Some((key, want)) = &q.meta {
            let ok = meta
                .as_ref()
                .and_then(|m| m.get(key))
                .is_some_and(|v| want.as_ref().is_none_or(|w| v == w));
            if !ok {
                continue;
            }
        }
        let mtime = unix_secs(std::fs::metadata(&dir).and_then(|m| m.modified()));
        hits.push((
            mtime,
            RunHit {
                path: run_rel,
                mtime,
                images,
                sample,
                meta,
            },
        ));
    }
    if q.recent {
        hits.sort_by_key(|h| std::cmp::Reverse(h.0));
        hits.truncate(RECENT_LIMIT);
    }
    Ok(SearchResult {
        runs: hits.into_iter().map(|(_, h)| h).collect(),
        partial: partial || !queue.is_empty(),
        directories_scanned: scanned,
    })
}

fn fnv(parts: &[&[u8]], seed: u64) -> u64 {
    let mut h = seed;
    for p in parts {
        for &b in *p {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        h = (h ^ 0xff).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A 128-bit FNV-based digest as 32 hex characters (a cache key, not a
/// security boundary).
pub(crate) fn hash128(parts: &[&[u8]]) -> String {
    format!(
        "{:016x}{:016x}",
        fnv(parts, 0xcbf2_9ce4_8422_2325),
        fnv(parts, 0x8422_2325_cbf2_9ce4)
    )
}

/// A downscaled PNG of the image at `abs`, from the thumbnail cache.
pub(crate) fn thumbnail(state: &State, abs: &Path) -> Result<PathBuf, String> {
    let meta = std::fs::metadata(abs).map_err(|e| e.to_string())?;
    if meta.len() > THUMB_MAX_BYTES {
        return Err("image too large for a thumbnail".into());
    }
    let key = hash128(&[
        abs.to_string_lossy().as_bytes(),
        &meta.len().to_le_bytes(),
        &unix_secs(meta.modified()).to_le_bytes(),
    ]);
    let dest = state.cache.join("thumbs").join(format!("{key}.png"));
    if dest.is_file() {
        return Ok(dest);
    }
    let rgba = if crate::hdr::is_hdr_path(abs) {
        let hdr = crate::hdr::decode_hdr(abs).map_err(|e| e.to_string())?;
        image::DynamicImage::ImageRgb8(crate::hdr::display_image(&hdr, state.view.hdr.tonemapper))
            .to_rgba8()
    } else {
        crate::run::decode(abs).map_err(|e| e.to_string())?
    };
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return Err("image is empty".into());
    }
    let scale = (f64::from(THUMB_EDGE) / f64::from(w.max(h))).min(1.0);
    let (tw, th) = (
        ((f64::from(w) * scale).round() as u32).max(1),
        ((f64::from(h) * scale).round() as u32).max(1),
    );
    let thumb = image::imageops::thumbnail(&rgba, tw, th);
    let tmp = dest.with_extension(format!("tmp{}", std::process::id()));
    thumb
        .save_with_format(&tmp, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}
