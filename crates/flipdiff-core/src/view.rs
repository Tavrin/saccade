//! The review viewer model (`flipdiff view`, SPEC §10) and the decisions file
//! it exports (`flipdiff approve --decisions`).
//!
//! [`build_view`] pairs 2 to 6 directories by relative path, compares every
//! non-reference image against the reference with FLIP, copies the images under
//! `<out>/images/`, and writes a self-contained `<out>/index.html`.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::compare::{CompareOptions, compare};
use crate::error::{Error, Result};
use crate::render::write_view_html;
use crate::report::Metrics;
use crate::run::{collect_images, copy_into_report, decode, io_err};

/// Schema tag of the view model embedded in `index.html`.
pub const VIEW_SCHEMA: &str = "flipdiff-view.v1";
/// Schema tag of the decisions file exported by the viewer.
pub const DECISIONS_SCHEMA: &str = "flipdiff-decisions.v1";

/// Minimum and maximum number of directories a view accepts.
pub const MIN_DIRS: usize = 2;
/// See [`MIN_DIRS`].
pub const MAX_DIRS: usize = 6;

/// Largest seed the viewer can round-trip through a JavaScript number (2^53 - 1).
pub const MAX_SEED: u64 = (1 << 53) - 1;

/// Options for [`build_view`].
#[derive(Debug, Clone)]
pub struct ViewOptions {
    /// One label per directory; `None` derives them from the directory names.
    pub labels: Option<Vec<String>>,
    /// The FLIP reference: a label or a directory path. `None` is the first.
    pub reference: Option<String>,
    /// Shuffle and hide the pane labels for pairwise judging.
    pub blind: bool,
    /// Shuffle seed; `None` derives one from the clock. Recorded in the data.
    pub seed: Option<u64>,
    /// FLIP pixels per degree.
    pub pixels_per_degree: f32,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            labels: None,
            reference: None,
            blind: false,
            seed: None,
            pixels_per_degree: CompareOptions::default().pixels_per_degree,
        }
    }
}

/// One image of an image set, from one directory.
#[derive(Debug, Clone, Serialize)]
pub struct ViewPane {
    /// Image path relative to the view directory; `None` when the directory
    /// has no image of this name.
    pub path: Option<String>,
    /// Image width in pixels (0 when missing or undecodable).
    pub width: u32,
    /// Image height in pixels (0 when missing or undecodable).
    pub height: u32,
    /// FLIP heatmap against the reference (non-reference panes only).
    pub heatmap: Option<String>,
    /// Quantised (8-bit grayscale) FLIP error map as a PNG data URI, read by
    /// the pixel inspector and the ROI statistics.
    pub flip: Option<String>,
    /// FLIP statistics against the reference.
    pub metrics: Option<Metrics>,
    /// Why this pane could not be compared or decoded.
    pub error: Option<String>,
}

/// All panes sharing one relative path.
#[derive(Debug, Clone, Serialize)]
pub struct ViewSet {
    /// Relative image path, `/`-separated.
    pub name: String,
    /// Display order of pane indices (shuffled in blind mode).
    pub order: Vec<usize>,
    /// One pane per directory, in directory order.
    pub panes: Vec<ViewPane>,
    /// Script that defines per-pane data URIs, loaded only when the browser
    /// refuses to read pixels from the image files (canvas taint on `file://`).
    pub pixels_script: String,
}

/// The data embedded in the viewer's `index.html`.
#[derive(Debug, Clone, Serialize)]
pub struct ViewModel {
    /// Always [`VIEW_SCHEMA`].
    pub schema: String,
    /// `flipdiff` version.
    pub tool_version: String,
    /// Generation time, Unix seconds.
    pub generated_at_unix: u64,
    /// Keys the viewer's `localStorage` entries.
    pub id: String,
    /// One label per directory.
    pub labels: Vec<String>,
    /// Index of the FLIP reference directory.
    pub reference: usize,
    /// Blind mode: labels are hidden until the judge reveals them.
    pub blind: bool,
    /// Shuffle seed (the per-set order derives from it and the set name).
    pub seed: u64,
    /// FLIP pixels per degree.
    pub pixels_per_degree: f32,
    /// The image sets, sorted by name.
    pub sets: Vec<ViewSet>,
}

/// A human verdict on one image set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Promote the capture to baseline.
    Accept,
    /// Do not promote.
    Reject,
    /// Not decided; the renderer needs more work.
    NeedsWork,
}

/// A region of interest in image pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roi {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width.
    pub w: u32,
    /// Height.
    pub h: u32,
}

/// The judge's entry for one image set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetDecision {
    /// Relative image path of the set.
    pub name: String,
    /// accept / reject / needs-work, if decided.
    #[serde(default)]
    pub decision: Option<Verdict>,
    /// Pairwise judging: the true label of the preferred pane.
    #[serde(default)]
    pub chosen_label: Option<String>,
    /// Pairwise judging: "no visible difference".
    #[serde(default)]
    pub no_difference: bool,
    /// Free-text note.
    #[serde(default)]
    pub note: String,
    /// The ROI that was drawn, if any.
    #[serde(default)]
    pub roi: Option<Roi>,
    /// When the entry was last edited, Unix milliseconds.
    #[serde(default)]
    pub timestamp_ms: u64,
}

/// The `flipdiff-decisions.v1.json` file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decisions {
    /// Always [`DECISIONS_SCHEMA`].
    pub schema: String,
    /// The shuffle seed of the view the decisions were made in.
    pub seed: u64,
    /// The view's directory labels.
    pub labels: Vec<String>,
    /// Whether the judging was blind.
    #[serde(default)]
    pub blind: bool,
    /// One entry per image set.
    pub sets: Vec<SetDecision>,
}

impl Decisions {
    /// Names of the sets whose verdict is [`Verdict::Accept`].
    pub fn accepted(&self) -> Vec<String> {
        self.sets
            .iter()
            .filter(|s| s.decision == Some(Verdict::Accept))
            .map(|s| s.name.clone())
            .collect()
    }
}

/// Reads and validates a decisions file.
pub fn read_decisions(path: &Path) -> Result<Decisions> {
    let text = std::fs::read_to_string(path)
        .map_err(io_err(format!("reading decisions file {}", path.display())))?;
    let decisions: Decisions = serde_json::from_str(&text)?;
    if decisions.schema != DECISIONS_SCHEMA {
        return Err(Error::Config(format!(
            "{} has schema {:?}, expected {DECISIONS_SCHEMA:?}",
            path.display(),
            decisions.schema
        )));
    }
    Ok(decisions)
}

/// True when `name` is a non-empty relative path made only of normal
/// components, so joining it onto a directory cannot escape that directory.
pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Deterministic pane order for one image set: a Fisher-Yates shuffle of
/// `0..n` seeded by `seed` and the set name.
pub fn shuffled_order(seed: u64, name: &str, n: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    let mut state = seed ^ fnv1a(name.as_bytes());
    for i in (1..n).rev() {
        let j = (splitmix64(&mut state) % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }
    order
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(char::from(B64[((n >> (18 - 6 * k)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn mime_of(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => "image/png",
    }
}

/// Encodes the error map as an 8-bit grayscale PNG data URI.
fn flip_data_uri(error_map: &[f32], w: u32, h: u32) -> Result<String> {
    let gray: Vec<u8> = error_map
        .iter()
        .map(|&v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();
    let img = image::GrayImage::from_raw(w, h, gray).ok_or(Error::EmptyImage)?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|source| Error::Encode {
            path: PathBuf::from("<flip error map>"),
            source,
        })?;
    Ok(format!("data:image/png;base64,{}", base64(&png)))
}

fn default_labels(dirs: &[PathBuf]) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    for (i, d) in dirs.iter().enumerate() {
        let base = d
            .canonicalize()
            .ok()
            .as_deref()
            .unwrap_or(d)
            .file_name()
            .map_or_else(
                || format!("dir{}", i + 1),
                |n| n.to_string_lossy().into_owned(),
            );
        let mut label = base.clone();
        let mut n = 2;
        while labels.contains(&label) {
            label = format!("{base}-{n}");
            n += 1;
        }
        labels.push(label);
    }
    labels
}

fn resolve_reference(
    reference: Option<&str>,
    dirs: &[PathBuf],
    labels: &[String],
) -> Result<usize> {
    let Some(r) = reference else { return Ok(0) };
    if let Some(i) = labels.iter().position(|l| l == r) {
        return Ok(i);
    }
    let want = Path::new(r).canonicalize().ok();
    dirs.iter()
        .position(|d| d == Path::new(r) || (want.is_some() && d.canonicalize().ok() == want))
        .ok_or_else(|| Error::Config(format!("--reference {r:?} is not one of the directories")))
}

/// Builds the viewer in `out_dir` (`index.html` plus `images/`) and returns
/// the embedded model.
///
/// Only directory and write failures return `Err`; per-image problems become
/// a pane `error`.
pub fn build_view(dirs: &[PathBuf], out_dir: &Path, opts: &ViewOptions) -> Result<ViewModel> {
    if !(MIN_DIRS..=MAX_DIRS).contains(&dirs.len()) {
        return Err(Error::Config(format!(
            "view takes {MIN_DIRS} to {MAX_DIRS} directories, got {}",
            dirs.len()
        )));
    }
    let labels = match &opts.labels {
        Some(l) if l.len() != dirs.len() => {
            return Err(Error::Config(format!(
                "{} labels for {} directories",
                l.len(),
                dirs.len()
            )));
        }
        Some(l) => l.clone(),
        None => default_labels(dirs),
    };
    for (i, l) in labels.iter().enumerate() {
        if l.is_empty() || labels[..i].contains(l) {
            return Err(Error::Config(format!(
                "labels must be non-empty and unique (got {l:?})"
            )));
        }
    }
    let reference = resolve_reference(opts.reference.as_deref(), dirs, &labels)?;

    let maps: Vec<BTreeMap<String, PathBuf>> = dirs
        .iter()
        .map(|d| collect_images(d))
        .collect::<Result<_>>()?;
    let mut names: Vec<&String> = maps.iter().flat_map(BTreeMap::keys).collect();
    names.sort();
    names.dedup();

    std::fs::create_dir_all(out_dir).map_err(io_err(format!("creating {}", out_dir.display())))?;
    let generated_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // JSON numbers are doubles in the browser: keep the seed exactly representable.
    let seed = opts.seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64 & MAX_SEED)
    });
    if seed > MAX_SEED {
        return Err(Error::Config(format!("--seed must be at most {MAX_SEED}")));
    }
    let compare_opts = CompareOptions {
        pixels_per_degree: opts.pixels_per_degree,
    };

    let mut sets = Vec::with_capacity(names.len());
    for (set_idx, name) in names.into_iter().enumerate() {
        let sources: Vec<Option<&PathBuf>> = maps.iter().map(|m| m.get(name)).collect();
        let decoded: Vec<Option<std::result::Result<image::RgbImage, Error>>> =
            sources.iter().map(|s| s.map(|p| decode(p))).collect();
        let ref_img = match decoded.get(reference) {
            Some(Some(Ok(img))) => Some(img),
            _ => None,
        };

        let mut panes = Vec::with_capacity(dirs.len());
        let mut pixel_uris = Vec::with_capacity(dirs.len());
        for (i, src) in sources.iter().enumerate() {
            let mut pane = ViewPane {
                path: None,
                width: 0,
                height: 0,
                heatmap: None,
                flip: None,
                metrics: None,
                error: None,
            };
            let Some(src) = src else {
                pixel_uris.push(None);
                panes.push(pane);
                continue;
            };
            pane.path = Some(copy_into_report(src, out_dir, name, &format!("pane{i}"))?);
            let bytes = std::fs::read(src).map_err(io_err(format!("reading {}", src.display())))?;
            pixel_uris.push(Some(format!(
                "data:{};base64,{}",
                mime_of(src),
                base64(&bytes)
            )));
            match &decoded[i] {
                Some(Ok(img)) => {
                    (pane.width, pane.height) = img.dimensions();
                    if i != reference {
                        match ref_img {
                            Some(r) => match compare(img, r, &compare_opts) {
                                Ok(cmp) => {
                                    let rel = format!("images/{name}/heatmap{i}.png");
                                    let dest = out_dir.join(&rel);
                                    cmp.heatmap_rgb()
                                        .save(&dest)
                                        .map_err(|source| Error::Encode { path: dest, source })?;
                                    pane.heatmap = Some(rel);
                                    pane.flip = Some(flip_data_uri(
                                        &cmp.error_map,
                                        cmp.metrics.width,
                                        cmp.metrics.height,
                                    )?);
                                    pane.metrics = Some(cmp.metrics);
                                }
                                Err(e) => pane.error = Some(e.to_string()),
                            },
                            None => {
                                pane.error =
                                    Some("reference image is missing or unreadable".into());
                            }
                        }
                    }
                }
                Some(Err(e)) => pane.error = Some(e.to_string()),
                None => {}
            }
            panes.push(pane);
        }

        let px_rel = format!("images/_px/{set_idx}.js");
        let px_path = out_dir.join(&px_rel);
        if let Some(parent) = px_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(io_err(format!("creating {}", parent.display())))?;
        }
        let px_json = serde_json::to_string(&pixel_uris)?;
        let px_js =
            format!("(window.__flipdiffPx=window.__flipdiffPx||{{}})[{set_idx}]={px_json};\n");
        std::fs::write(&px_path, px_js)
            .map_err(io_err(format!("writing {}", px_path.display())))?;

        let order = if opts.blind {
            shuffled_order(seed, name, dirs.len())
        } else {
            (0..dirs.len()).collect()
        };
        sets.push(ViewSet {
            name: name.clone(),
            order,
            panes,
            pixels_script: px_rel,
        });
    }

    let id = {
        let mut key = format!("{generated_at_unix}|{seed}|{}", labels.join(","));
        for s in &sets {
            key.push('|');
            key.push_str(&s.name);
        }
        format!("{:016x}", fnv1a(key.as_bytes()))
    };
    let model = ViewModel {
        schema: VIEW_SCHEMA.to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at_unix,
        id,
        labels,
        reference,
        blind: opts.blind,
        seed,
        pixels_per_degree: opts.pixels_per_degree,
        sets,
    };
    write_view_html(&model, out_dir)?;
    Ok(model)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write_png(dir: &Path, name: &str, rgb: [u8; 3]) {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbImage::from_pixel(8, 8, image::Rgb(rgb))
            .save(path)
            .unwrap();
    }

    fn three_dirs(root: &Path) -> Vec<PathBuf> {
        let dirs: Vec<PathBuf> = ["a", "b", "c"].iter().map(|d| root.join(d)).collect();
        for d in &dirs {
            write_png(d, "x.png", [10, 10, 10]);
        }
        write_png(&dirs[1], "sub/y.png", [200, 20, 20]);
        write_png(&dirs[2], "x.png", [60, 10, 10]);
        dirs
    }

    #[test]
    fn pairs_n_ways_with_labels_and_reference() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = three_dirs(tmp.path());
        let opts = ViewOptions {
            labels: Some(vec!["old".into(), "mid".into(), "new".into()]),
            reference: Some("mid".into()),
            ..ViewOptions::default()
        };
        let out = tmp.path().join("view");
        let m = build_view(&dirs, &out, &opts).unwrap();
        assert_eq!(m.labels, ["old", "mid", "new"]);
        assert_eq!(m.reference, 1);
        let names: Vec<_> = m.sets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["sub/y.png", "x.png"]);
        let x = &m.sets[1];
        assert!(x.panes.iter().all(|p| p.path.is_some()));
        // Reference pane has no comparison; the others do.
        assert!(x.panes[1].metrics.is_none());
        assert!(x.panes[0].metrics.is_some() && x.panes[2].metrics.is_some());
        assert!(out.join("images/x.png/heatmap2.png").is_file());
        // y.png exists only in the reference directory.
        let y = &m.sets[0];
        assert!(y.panes[0].path.is_none() && y.panes[1].path.is_some());
        assert!(y.panes.iter().all(|p| p.metrics.is_none()));
        assert!(build_view(&dirs[..1], &out, &ViewOptions::default()).is_err());
    }

    #[test]
    fn blind_order_is_a_deterministic_seeded_permutation() {
        let a = shuffled_order(7, "x.png", 6);
        assert_eq!(a, shuffled_order(7, "x.png", 6));
        let mut sorted = a.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4, 5]);
        let differs = (0..32).any(|s| shuffled_order(s, "x.png", 6) != a)
            && (0..32).any(|s| shuffled_order(s, "x.png", 6) != (0..6).collect::<Vec<_>>());
        assert!(differs, "seed must change the order");
    }

    #[test]
    fn decisions_round_trip_and_accepted_names() {
        let d = Decisions {
            schema: DECISIONS_SCHEMA.into(),
            seed: 42,
            labels: vec!["a".into(), "b".into()],
            blind: true,
            sets: vec![
                SetDecision {
                    name: "x.png".into(),
                    decision: Some(Verdict::Accept),
                    chosen_label: Some("b".into()),
                    no_difference: false,
                    note: "fine".into(),
                    roi: Some(Roi {
                        x: 1,
                        y: 2,
                        w: 3,
                        h: 4,
                    }),
                    timestamp_ms: 9,
                },
                SetDecision {
                    name: "y.png".into(),
                    decision: Some(Verdict::NeedsWork),
                    chosen_label: None,
                    no_difference: true,
                    note: String::new(),
                    roi: None,
                    timestamp_ms: 10,
                },
            ],
        };
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("d.json");
        std::fs::write(&path, serde_json::to_string(&d).unwrap()).unwrap();
        let back = read_decisions(&path).unwrap();
        assert_eq!(back, d);
        assert_eq!(back.accepted(), ["x.png"]);
        std::fs::write(&path, r#"{"schema":"nope","seed":1,"labels":[],"sets":[]}"#).unwrap();
        assert!(read_decisions(&path).is_err());
    }

    #[test]
    fn html_is_self_contained() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = three_dirs(tmp.path());
        let out = tmp.path().join("view");
        build_view(&dirs, &out, &ViewOptions::default()).unwrap();
        let html = std::fs::read_to_string(out.join("index.html")).unwrap();
        for needle in [
            "http://",
            "https://",
            "src=\"//",
            "href=\"//",
            "@import",
            "url(//",
        ] {
            assert!(!html.contains(needle), "external reference {needle:?}");
        }
        assert!(html.contains("flipdiff-view.v1"));
        assert!(!html.contains("__FLIPDIFF_"), "unreplaced placeholder");
    }
}
