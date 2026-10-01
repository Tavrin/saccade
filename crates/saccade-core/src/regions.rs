//! Regions of interest and masks (`[[region]]` / `[[mask]]` in `saccade.toml`).
//!
//! A mask excludes pixels from every statistic. A region is a named rectangle
//! with its own statistics and an optional threshold. Rectangles are fractions
//! of the frame, so a resolution change keeps their meaning.

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::compare::{Comparison, masked_metrics};
use crate::config::{RunConfig, compile_glob};
use crate::error::{Error, Result};
use crate::report::{Entry, Metric, Metrics, RegionResult, Status};
use crate::run::{metric_value, status_of};

/// A named region of interest, as written in a `[[region]]` table.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionSpec {
    /// Name shown in reports.
    pub name: String,
    /// Image-name glob; every image when absent.
    #[serde(default)]
    pub glob: Option<String>,
    /// `[x, y, w, h]` as fractions of the frame, each in `[0, 1]`.
    pub rect: [f64; 4],
    /// Pass threshold; without one the region is informational.
    #[serde(default)]
    pub threshold: Option<f64>,
    /// Deciding metric; the entry's metric when absent.
    #[serde(default)]
    pub metric: Option<Metric>,
}

/// An excluded area, as written in a `[[mask]]` table: a rectangle or an image.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskSpec {
    /// Image-name glob; every image when absent.
    #[serde(default)]
    pub glob: Option<String>,
    /// `[x, y, w, h]` as fractions of the frame.
    #[serde(default)]
    pub rect: Option<[f64; 4]>,
    /// Mask image path relative to the config file; white pixels are excluded
    /// and the image is resized (nearest) to the frame.
    #[serde(default)]
    pub image: Option<String>,
}

/// Epsilon that keeps `0.29 * 100` from flooring to 28.
const EPS: f64 = 1e-9;

/// Resolves a fractional rect to pixels `[x, y, w, h]`: floor the origin,
/// ceil the far edge, clamp to the frame. `None` when the result is empty.
pub fn resolve_rect(rect: [f64; 4], width: u32, height: u32) -> Option<[u32; 4]> {
    let [x, y, w, h] = rect;
    let (fw, fh) = (f64::from(width), f64::from(height));
    let edge = |lo: f64, len: f64, full: f64| {
        let a = (lo * full + EPS).floor().clamp(0.0, full);
        let b = ((lo + len) * full - EPS).ceil().clamp(0.0, full);
        (a as u32, b as u32)
    };
    let (x0, x1) = edge(x, w, fw);
    let (y0, y1) = edge(y, h, fh);
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
}

fn check_rect(what: &str, r: [f64; 4]) -> Result<()> {
    let ok = r.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) && r[2] > 0.0 && r[3] > 0.0;
    if ok {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "{what}: rect {r:?} needs finite fractions in [0, 1] with w > 0 and h > 0"
        )))
    }
}

/// Rejects absolute paths and any `..` component.
fn check_relative(path: &str) -> Result<()> {
    let p = Path::new(path);
    let bad = path.is_empty()
        || p.components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir));
    if bad {
        return Err(Error::Config(format!(
            "mask image {path:?} must be a relative path without `..`"
        )));
    }
    Ok(())
}

/// Validates the `[[region]]` and `[[mask]]` tables.
pub(crate) fn validate(regions: &[RegionSpec], masks: &[MaskSpec]) -> Result<()> {
    for r in regions {
        let what = format!("region {:?}", r.name);
        if r.name.is_empty() {
            return Err(Error::Config("region name must not be empty".into()));
        }
        check_rect(&what, r.rect)?;
        if let Some(g) = &r.glob {
            compile_glob(g)?;
        }
        if r.threshold.is_some_and(|t| !t.is_finite()) {
            return Err(Error::Config(format!("{what}: threshold must be finite")));
        }
    }
    for m in masks {
        if let Some(g) = &m.glob {
            compile_glob(g)?;
        }
        match (&m.rect, &m.image) {
            (Some(r), None) => check_rect("mask", *r)?,
            (None, Some(p)) => check_relative(p)?,
            _ => {
                return Err(Error::Config(
                    "a mask needs exactly one of `rect` or `image`".into(),
                ));
            }
        }
    }
    Ok(())
}

fn matches(glob: Option<&str>, name: &str) -> Result<bool> {
    match glob {
        Some(g) => Ok(compile_glob(g)?.is_match(name)),
        None => Ok(true),
    }
}

/// Loads a mask image into an exclusion bitmap of the frame size.
fn load_mask_image(path: &str, config_dir: Option<&Path>, w: u32, h: u32) -> Result<Vec<bool>> {
    check_relative(path)?;
    let root = config_dir.map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let full = root.join(path);
    let io = |source| Error::Io {
        context: format!("reading mask image {}", full.display()),
        source,
    };
    // A symlink inside the config directory must not lead out of it.
    let canon = crate::paths::canonicalize(&full).map_err(io)?;
    if !crate::paths::canonicalize(&root)
        .map_err(io)
        .is_ok_and(|r| canon.starts_with(r))
    {
        return Err(Error::Config(format!(
            "mask image {path:?} resolves outside the config directory"
        )));
    }
    let img = image::open(&canon)
        .map_err(|source| Error::Decode {
            path: canon.clone(),
            source,
        })?
        .to_luma8();
    let (mw, mh) = img.dimensions();
    if mw == 0 || mh == 0 {
        return Err(Error::EmptyImage);
    }
    let mut out = Vec::with_capacity(w as usize * h as usize);
    for y in 0..h {
        let sy = (u64::from(y) * u64::from(mh) / u64::from(h)) as u32;
        for x in 0..w {
            let sx = (u64::from(x) * u64::from(mw) / u64::from(w)) as u32;
            out.push(img.get_pixel(sx, sy).0[0] >= 128);
        }
    }
    Ok(out)
}

/// The union of every mask that applies to `name`, as a row-major exclusion
/// bitmap (`true` = excluded); `None` when no mask applies.
pub fn mask_for(
    name: &str,
    width: u32,
    height: u32,
    masks: &[MaskSpec],
    config_dir: Option<&Path>,
) -> Result<Option<Vec<bool>>> {
    let mut out: Option<Vec<bool>> = None;
    for m in masks {
        if !matches(m.glob.as_deref(), name)? {
            continue;
        }
        let buf = out.get_or_insert_with(|| vec![false; width as usize * height as usize]);
        if let Some(r) = m.rect {
            let [x, y, w, h] = resolve_rect(r, width, height).ok_or_else(|| {
                Error::Config(format!("mask rect {r:?} is empty at {width}x{height}"))
            })?;
            for row in y..y + h {
                let start = row as usize * width as usize + x as usize;
                buf[start..start + w as usize].fill(true);
            }
        }
        if let Some(p) = &m.image {
            let bits = load_mask_image(p, config_dir, width, height)?;
            for (b, on) in buf.iter_mut().zip(bits) {
                *b |= on;
            }
        }
    }
    Ok(out)
}

/// What [`evaluate`] hands back to the run loop.
pub(crate) struct Scene {
    /// Whole-image statistics with masked pixels excluded.
    pub metrics: Metrics,
    /// The exclusion bitmap, when any mask applied.
    pub mask: Option<Vec<bool>>,
}

/// Applies masks and regions to `cmp` for the entry: fills `entry.regions` and
/// `entry.masked_fraction` and returns the masked whole-image statistics.
///
/// Errors (which make the entry an `error` entry) when a rect resolves to
/// nothing, a mask image cannot be read, or every pixel is masked.
pub(crate) fn evaluate(entry: &mut Entry, cmp: &Comparison, config: &RunConfig) -> Result<Scene> {
    let (w, h) = (cmp.metrics.width, cmp.metrics.height);
    let mask = mask_for(
        &entry.name,
        w,
        h,
        &config.masks,
        config.config_dir.as_deref(),
    )?;
    let mask_ref = mask.as_deref();
    if let Some(m) = mask_ref {
        let excluded = m.iter().filter(|&&b| b).count();
        entry.masked_fraction = Some(excluded as f64 / m.len().max(1) as f64);
    }
    let metrics = match mask_ref {
        None => cmp.metrics,
        Some(_) => masked_metrics(&cmp.error_map, mask_ref, w, h, [0, 0, w, h])
            .ok_or_else(|| Error::Config("every pixel is masked".into()))?,
    };

    entry.regions.clear();
    for r in &config.regions {
        if !matches(r.glob.as_deref(), &entry.name)? {
            continue;
        }
        let rect = resolve_rect(r.rect, w, h)
            .ok_or_else(|| Error::Config(format!("region {:?} is empty at {w}x{h}", r.name)))?;
        let metric_used = r.metric.unwrap_or(entry.metric_used);
        let (value, status, region_metrics) =
            match masked_metrics(&cmp.error_map, mask_ref, w, h, rect) {
                Some(m) => {
                    let v = metric_value(&m, metric_used);
                    (v, r.threshold.map(|t| status_of(v, t)), m)
                }
                // Every pixel of the region is masked: informational, no value.
                None => (
                    f64::NAN,
                    None,
                    Metrics {
                        mean: 0.0,
                        max: 0.0,
                        p50: 0.0,
                        p95: 0.0,
                        p99: 0.0,
                        frac_above_0_1: 0.0,
                        frac_above_0_5: 0.0,
                        width: rect[2],
                        height: rect[3],
                    },
                ),
            };
        entry.regions.push(RegionResult {
            name: r.name.clone(),
            rect_px: rect,
            status,
            metric_used,
            threshold: r.threshold,
            value,
            metrics: region_metrics,
        });
    }
    Ok(Scene { metrics, mask })
}

/// The entry status once regions are considered: a failing region fails it.
pub(crate) fn combine(status: Status, regions: &[RegionResult]) -> Status {
    if regions.iter().any(|r| r.status == Some(Status::Fail)) {
        Status::Fail
    } else {
        status
    }
}
