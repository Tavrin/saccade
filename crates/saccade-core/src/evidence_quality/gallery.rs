//! Automatic ROI strips selected from existing hotspots and measured tile findings.
use super::spatial::{self, SpatialReport};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Automatic gallery region with its artifact links and local statistics.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GalleryRegion {
    /// Full-resolution `[x,y,width,height]`.
    pub rect_px: [u32; 4],
    /// Selection reason(s): hotspot, bias, coverage, or detail.
    pub reasons: Vec<String>,
    /// Pixels measured after scope restrictions.
    pub pixels: u64,
    /// Mean full-resolution FLIP.
    pub mean_flip: f64,
    /// Maximum full-resolution FLIP.
    pub max_flip: f64,
    /// Signed mean normalized sRGB luminance shift.
    pub signed_shift: f64,
    /// Candidate/base fine-detail energy.
    pub detail_energy_ratio: Option<f64>,
    /// Mean tile gap change in this footprint.
    pub gap_change: Option<f64>,
    /// Baseline | candidate | heatmap crop strip.
    pub strip: String,
    /// Exact 2x nearest-neighbour enlargement of the strip.
    pub zoom_2x: String,
}
fn save(img: &image::RgbImage, path: &Path) -> Result<()> {
    img.save(path).map_err(|source| Error::Encode {
        path: path.into(),
        source,
    })
}
fn overlap(a: [u32; 4], b: [u32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}
/// Generate top-N portable crops. No input file is modified.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    name: &str,
    base: &image::RgbaImage,
    candidate: &image::RgbaImage,
    errors: &[f32],
    excluded: Option<&[bool]>,
    hotspots: &[crate::report::Hotspot],
    spatial: &SpatialReport,
    out: &Path,
) -> Result<Vec<GalleryRegion>> {
    let (w, h) = base.dimensions();
    if candidate.dimensions() != base.dimensions()
        || errors.len() != (w as usize * h as usize)
        || excluded.is_some_and(|m| m.len() != errors.len())
    {
        return Err(Error::Config("gallery dimensions differ".into()));
    }
    let top = spatial.policy.gallery_top;
    if top == 0 {
        return Ok(Vec::new());
    }
    let mut candidates: Vec<([u32; 4], Vec<String>, f64)> = hotspots
        .iter()
        .map(|h| {
            (
                h.rect_px,
                vec!["hotspot".into()],
                h.mean_flip + h.max_flip * 0.1,
            )
        })
        .collect();
    for region in &spatial.regions {
        candidates.push((
            region.rect_px,
            region.reasons.clone(),
            1.0 + region.signed_shift.abs(),
        ));
    }
    for tile in &spatial.tiles {
        if tile.pixels == 0 {
            continue;
        }
        let mut reasons = Vec::new();
        if tile.absolute_shifted || tile.relative_shifted {
            reasons.push("signed_bias".into());
        }
        if tile
            .gap_change
            .is_some_and(|g| g.abs() > spatial.policy.gap_shift)
        {
            reasons.push("coverage".into());
        }
        if !reasons.is_empty() {
            candidates.push((
                tile.rect_px,
                reasons,
                tile.signed_shift.abs() / spatial.policy.absolute_shift
                    + tile.gap_change.unwrap_or(0.0).abs() / spatial.policy.gap_shift,
            ));
        }
    }
    candidates.sort_by(|a, b| b.2.total_cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let mut chosen: Vec<([u32; 4], Vec<String>)> = Vec::new();
    for (r, reasons, _) in candidates {
        if chosen.iter().any(|(b, _)| r == *b) {
            continue;
        }
        chosen.push((r, reasons));
        if chosen.len() == top {
            break;
        }
    }
    let directory = out.join("regions");
    std::fs::create_dir_all(&directory)
        .map_err(crate::run::io_err("creating ROI gallery".into()))?;
    let id = crate::localized::digest(name.as_bytes());
    let b = crate::compare::flatten_over(base, 0);
    let c = crate::compare::flatten_over(candidate, 0);
    let cmp = crate::compare::Comparison {
        metrics: crate::compare::metrics_of(errors, w, h),
        error_map: errors.to_vec(),
    };
    let heat = cmp.heatmap_rgb();
    let lb: Vec<_> = spatial::rgb(base)
        .into_iter()
        .map(spatial::luminance)
        .collect();
    let lc: Vec<_> = spatial::rgb(candidate)
        .into_iter()
        .map(spatial::luminance)
        .collect();
    let mut result = Vec::new();
    for (i, (r, reasons)) in chosen.into_iter().enumerate() {
        let [x, y, rw, rh] = r;
        if rw == 0
            || rh == 0
            || x.checked_add(rw).is_none_or(|v| v > w)
            || y.checked_add(rh).is_none_or(|v| v > h)
        {
            return Err(Error::Config("gallery rectangle exceeds image".into()));
        }
        let mut strip = image::RgbImage::new(rw * 3, rh);
        for (panel, img) in [&b, &c, &heat].into_iter().enumerate() {
            for yy in 0..rh {
                for xx in 0..rw {
                    strip.put_pixel(panel as u32 * rw + xx, yy, *img.get_pixel(x + xx, y + yy));
                }
            }
        }
        let strip_path = format!("regions/{}-{i}.png", &id[..16]);
        let zoom_path = format!("regions/{}-{i}-2x.png", &id[..16]);
        save(&strip, &out.join(&strip_path))?;
        let zoom =
            image::imageops::resize(&strip, rw * 6, rh * 2, image::imageops::FilterType::Nearest);
        save(&zoom, &out.join(&zoom_path))?;
        let indexes: Vec<_> = (y..y + rh)
            .flat_map(|yy| (x..x + rw).map(move |xx| (yy * w + xx) as usize))
            .filter(|i| !excluded.is_some_and(|m| m[*i]))
            .collect();
        let count = indexes.len().max(1) as f64;
        let selected: Vec<_> = spatial
            .tiles
            .iter()
            .filter(|t| t.pixels > 0 && overlap(r, t.rect_px))
            .collect();
        let changes: Vec<_> = selected.iter().filter_map(|t| t.gap_change).collect();
        let bd = spatial::detail_scoped(&lb, w, h, r, excluded);
        let cd = spatial::detail_scoped(&lc, w, h, r, excluded);
        let energy = if bd > 1e-15 {
            Some(cd / bd)
        } else if cd <= 1e-15 {
            Some(1.0)
        } else {
            None
        };
        result.push(GalleryRegion {
            rect_px: r,
            reasons,
            pixels: indexes.len() as u64,
            mean_flip: indexes.iter().map(|i| f64::from(errors[*i])).sum::<f64>() / count,
            max_flip: indexes
                .iter()
                .map(|i| f64::from(errors[*i]))
                .fold(0.0, f64::max),
            signed_shift: indexes.iter().map(|i| lc[*i] - lb[*i]).sum::<f64>() / count,
            detail_energy_ratio: energy,
            gap_change: (!changes.is_empty())
                .then(|| changes.iter().sum::<f64>() / changes.len() as f64),
            strip: strip_path,
            zoom_2x: zoom_path,
        });
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gallery_has_three_panels_and_exact_nearest_zoom() {
        let tmp = tempfile::tempdir().unwrap();
        let b = image::RgbaImage::from_pixel(64, 64, image::Rgba([80, 80, 80, 255]));
        let mut c = b.clone();
        for y in 0..32 {
            for x in 0..32 {
                c.put_pixel(x, y, image::Rgba([120, 120, 120, 255]));
            }
        }
        let opts = crate::compare::CompareOptions::default();
        let cmp = crate::compare::compare_rgba(&c, &b, &opts).unwrap();
        let s = spatial::analyze(
            &b,
            &c,
            &cmp.error_map,
            None,
            None,
            &spatial::Policy::default(),
            &opts,
            crate::diagnostics::ChangeClass::LocalStructure,
        )
        .unwrap();
        let hot = crate::hotspots::find_hotspots(&cmp.error_map, None, 64, 64, &Default::default());
        let r = generate(
            "frame.png",
            &b,
            &c,
            &cmp.error_map,
            None,
            &hot,
            &s,
            tmp.path(),
        )
        .unwrap();
        assert!(!r.is_empty());
        let strip = image::open(tmp.path().join(&r[0].strip)).unwrap().to_rgb8();
        let zoom = image::open(tmp.path().join(&r[0].zoom_2x))
            .unwrap()
            .to_rgb8();
        assert_eq!(strip.dimensions(), (r[0].rect_px[2] * 3, r[0].rect_px[3]));
        assert_eq!(zoom.dimensions(), (strip.width() * 2, strip.height() * 2));
        for y in 0..zoom.height() {
            for x in 0..zoom.width() {
                assert_eq!(zoom.get_pixel(x, y), strip.get_pixel(x / 2, y / 2));
            }
        }
    }
}
