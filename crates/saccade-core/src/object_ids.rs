//! Attribution of measured hotspot error to capture-side object or material IDs.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::report::Hotspot;

/// A single ID's share of FLIP error above the hotspot cutoff inside a box.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdContribution {
    /// Numeric ID encoded in the sidecar image.
    pub id: u32,
    /// Name from the sidecar legend, or `<unlabeled>` for ID zero.
    pub name: String,
    /// FLIP error sum for this ID divided by all hot error inside the box.
    pub error_share: f64,
    /// Number of above-cutoff pixels assigned to this ID.
    pub pixels: u64,
}

/// Attribution for one numbered hotspot's bounding box.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HotspotAttribution {
    /// One-based index into the entry's `hotspots` list.
    pub hotspot: usize,
    /// Hot pixels measured inside the box; may differ from hotspot component area.
    pub measured_hot_pixels: u64,
    /// IDs in descending error-share order.
    pub contributions: Vec<IdContribution>,
}

/// Attribution from one capture-side object/material ID pair.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribution {
    /// `object` or `material`.
    pub kind: String,
    /// SHA-256 of the native PNG or EXR ID image.
    pub image_sha256: String,
    /// SHA-256 of the JSON legend.
    pub legend_sha256: String,
    /// Copied ID image path, relative to the report directory.
    pub image_path: String,
    /// Copied legend path, relative to the report directory.
    pub legend_path: String,
    /// Per-hotspot error shares.
    pub hotspots: Vec<HotspotAttribution>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Legend {
    schema: String,
    kind: String,
    ids: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
pub(crate) struct Sidecars<'a> {
    pub capture: &'a Path,
    pub report_dir: &'a Path,
    pub entry_name: &'a str,
}

#[derive(Clone, Copy)]
pub(crate) struct HotspotPixels<'a> {
    pub errors: &'a [f32],
    pub mask: Option<&'a [bool]>,
    pub width: u32,
    pub height: u32,
    pub hotspots: &'a [Hotspot],
    pub cutoff: f32,
}

/// Names reserved for capture-side attribution images, excluded from normal
/// colour-image pairing. The JSON legend is not an image and needs no filter.
pub fn is_sidecar_image_name(name: &str) -> bool {
    [
        ".object-id.png",
        ".object-id.exr",
        ".material-id.png",
        ".material-id.exr",
    ]
    .iter()
    .any(|suffix| name.to_ascii_lowercase().ends_with(suffix))
}

fn sidecar(capture: &Path, kind: &str, ext: &str) -> Result<PathBuf> {
    let stem = capture
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Config(format!("non-UTF8 capture stem: {}", capture.display())))?;
    Ok(capture.with_file_name(format!("{stem}.{kind}-id.{ext}")))
}

fn present(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => Err(Error::Config(format!(
            "ID sidecar is a symlink: {}",
            path.display()
        ))),
        Ok(m) if m.is_file() => Ok(true),
        Ok(_) => Err(Error::Config(format!(
            "ID sidecar is not a regular file: {}",
            path.display()
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::Io {
            context: format!("checking {}", path.display()),
            source,
        }),
    }
}

fn decode_ids(path: &Path, width: u32, height: u32) -> Result<Vec<u32>> {
    let image = image::open(path).map_err(|source| Error::Decode {
        path: path.to_path_buf(),
        source,
    })?;
    if (image.width(), image.height()) != (width, height) {
        return Err(Error::Config(format!(
            "ID image {} has dimensions {}x{}, expected {}x{}",
            path.display(),
            image.width(),
            image.height(),
            width,
            height
        )));
    }
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exr"))
    {
        let rgb = image.to_rgb32f();
        rgb.pixels()
            .map(|p| {
                let v = p.0[0];
                if !v.is_finite() || v < 0.0 || v > 16_777_215.0 || v.fract() != 0.0 {
                    Err(Error::Config(format!(
                        "EXR ID image {} contains a non-integer or out-of-range red value",
                        path.display()
                    )))
                } else {
                    Ok(v as u32)
                }
            })
            .collect()
    } else {
        Ok(image
            .to_rgb8()
            .pixels()
            .map(|p| (u32::from(p.0[0]) << 16) | (u32::from(p.0[1]) << 8) | u32::from(p.0[2]))
            .collect())
    }
}

fn one_kind(
    sidecars: Sidecars<'_>,
    pixels: HotspotPixels<'_>,
    kind: &str,
) -> Result<Option<Attribution>> {
    let Sidecars {
        capture,
        report_dir,
        entry_name,
    } = sidecars;
    let HotspotPixels {
        errors,
        mask,
        width,
        height,
        hotspots,
        cutoff,
    } = pixels;
    let (png, exr, legend_path) = (
        sidecar(capture, kind, "png")?,
        sidecar(capture, kind, "exr")?,
        sidecar(capture, kind, "json")?,
    );
    let (p, x, l) = (present(&png)?, present(&exr)?, present(&legend_path)?);
    if !p && !x && !l {
        return Ok(None);
    }
    if p == x || !l {
        return Err(Error::Config(format!(
            "{kind} ID sidecar needs exactly one PNG or EXR image and one JSON legend next to {}",
            capture.display()
        )));
    }
    let image_path = if p { png } else { exr };
    let image_sha256 = crate::run::sha256_file(&image_path)?;
    let legend_sha256 = crate::run::sha256_file(&legend_path)?;
    let bytes = std::fs::read(&legend_path).map_err(crate::run::io_err(format!(
        "reading {}",
        legend_path.display()
    )))?;
    let legend: Legend = serde_json::from_slice(&bytes)?;
    if legend.schema != "saccade-object-ids.v1" || legend.kind != kind {
        return Err(Error::Config(format!(
            "ID legend {} needs schema saccade-object-ids.v1 and kind {kind}",
            legend_path.display()
        )));
    }
    let mut names = BTreeMap::new();
    for (id, name) in legend.ids {
        let id: u32 = id.parse().map_err(|_| {
            Error::Config(format!("invalid ID {id:?} in {}", legend_path.display()))
        })?;
        if id > 16_777_215 {
            return Err(Error::Config(format!(
                "ID {id} in {} exceeds 24 bits",
                legend_path.display()
            )));
        }
        if name.trim().is_empty() {
            return Err(Error::Config(format!(
                "empty name for ID {id} in {}",
                legend_path.display()
            )));
        }
        names.insert(id, name);
    }
    let ids = decode_ids(&image_path, width, height)?;
    let mut results = Vec::new();
    for (index, hotspot) in hotspots.iter().enumerate() {
        let [x0, y0, w, h] = hotspot.rect_px;
        let mut sums: BTreeMap<u32, (f64, u64)> = BTreeMap::new();
        let mut total = 0.0f64;
        for y in y0..y0.saturating_add(h).min(height) {
            for x in x0..x0.saturating_add(w).min(width) {
                let i = y as usize * width as usize + x as usize;
                if !hotspot.pixel_runs.is_empty()
                    && !hotspot
                        .pixel_runs
                        .iter()
                        .any(|r| i >= r[0] as usize && i < (r[0] + r[1]) as usize)
                {
                    continue;
                }
                let v = errors[i];
                if !v.is_finite() || v <= cutoff || mask.is_some_and(|m| m[i]) {
                    continue;
                }
                let id = ids[i];
                if id != 0 && !names.contains_key(&id) {
                    return Err(Error::Config(format!(
                        "ID {id} in {} has no legend name",
                        image_path.display()
                    )));
                }
                let part = sums.entry(id).or_default();
                part.0 += f64::from(v);
                part.1 += 1;
                total += f64::from(v);
            }
        }
        let mut contributions: Vec<_> = sums
            .into_iter()
            .map(|(id, (sum, pixels))| IdContribution {
                id,
                name: names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| "<unlabeled>".into()),
                error_share: if total > 0.0 { sum / total } else { 0.0 },
                pixels,
            })
            .collect();
        contributions.sort_by(|a, b| {
            b.error_share
                .total_cmp(&a.error_share)
                .then(a.id.cmp(&b.id))
        });
        results.push(HotspotAttribution {
            hotspot: index + 1,
            measured_hot_pixels: contributions.iter().map(|c| c.pixels).sum(),
            contributions,
        });
    }
    let extension = image_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");
    let image_rel = format!("images/{entry_name}.d/{kind}-id.{extension}");
    let legend_rel = format!("images/{entry_name}.d/{kind}-id.json");
    for (source, rel) in [(&image_path, &image_rel), (&legend_path, &legend_rel)] {
        let dest = report_dir.join(rel);
        if present(&dest)? {
            std::fs::remove_file(&dest)
                .map_err(crate::run::io_err(format!("removing {}", dest.display())))?;
        }
        std::fs::copy(source, &dest)
            .map_err(crate::run::io_err(format!("copying {}", source.display())))?;
    }
    if crate::run::sha256_file(&image_path)? != image_sha256
        || crate::run::sha256_file(&legend_path)? != legend_sha256
        || crate::run::sha256_file(&report_dir.join(&image_rel))? != image_sha256
        || crate::run::sha256_file(&report_dir.join(&legend_rel))? != legend_sha256
    {
        return Err(Error::Config(format!(
            "{kind} ID sidecars changed while reading or copying"
        )));
    }
    Ok(Some(Attribution {
        kind: kind.into(),
        image_sha256,
        legend_sha256,
        image_path: image_rel,
        legend_path: legend_rel,
        hotspots: results,
    }))
}

/// Read optional capture-side ID images and attribute each measured hotspot.
/// Errors on malformed or incomplete sidecars; absence is ordinary.
pub(crate) fn attribute(
    sidecars: Sidecars<'_>,
    pixels: HotspotPixels<'_>,
) -> Result<Vec<Attribution>> {
    let mut out = Vec::new();
    for kind in ["object", "material"] {
        if let Some(item) = one_kind(sidecars, pixels, kind)? {
            out.push(item);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn png_and_exr_ids_attribute_weighted_hotspot_error_and_copy_sources() {
        let dir = tempfile::tempdir().expect("tempdir");
        let capture = dir.path().join("frame.png");
        image::RgbImage::new(4, 4).save(&capture).expect("capture");
        let mut ids = image::RgbImage::from_pixel(4, 4, image::Rgb([0, 0, 1]));
        for x in 0..4 {
            ids.put_pixel(x, 3, image::Rgb([0, 0, 2]));
        }
        ids.save(dir.path().join("frame.object-id.png"))
            .expect("IDs");
        std::fs::write(
            dir.path().join("frame.object-id.json"),
            r#"{"schema":"saccade-object-ids.v1","kind":"object","ids":{"1":"tree","2":"rock"}}"#,
        )
        .expect("legend");
        image::Rgb32FImage::from_pixel(4, 4, image::Rgb([3.0, 0.0, 0.0]))
            .save(dir.path().join("frame.material-id.exr"))
            .expect("EXR IDs");
        std::fs::write(
            dir.path().join("frame.material-id.json"),
            r#"{"schema":"saccade-object-ids.v1","kind":"material","ids":{"3":"rock_moss"}}"#,
        )
        .expect("legend");
        let report = dir.path().join("report");
        std::fs::create_dir_all(report.join("images/frame.png.d")).expect("report dir");
        let hotspot = Hotspot {
            pixel_runs: Vec::new(),
            rect_px: [0, 0, 4, 4],
            rect_frac: [0.0, 0.0, 1.0, 1.0],
            area_px: 16,
            area_frac: 1.0,
            mean_flip: 0.5,
            max_flip: 0.8,
            share_of_total_error: 1.0,
            position: "center".into(),
        };
        let mut errors = vec![0.4; 16];
        errors[12..].fill(0.8);
        let result = attribute(
            Sidecars {
                capture: &capture,
                report_dir: &report,
                entry_name: "frame.png",
            },
            HotspotPixels {
                errors: &errors,
                mask: None,
                width: 4,
                height: 4,
                hotspots: std::slice::from_ref(&hotspot),
                cutoff: 0.1,
            },
        )
        .expect("attribution");
        assert_eq!(result.len(), 2);
        let object = &result[0].hotspots[0].contributions;
        assert_eq!((object[0].name.as_str(), object[0].pixels), ("tree", 12));
        assert!((object[0].error_share - 0.6).abs() < 1e-6);
        assert!((object[1].error_share - 0.4).abs() < 1e-6);
        assert_eq!(result[1].hotspots[0].contributions[0].name, "rock_moss");
        let exact = Hotspot {
            pixel_runs: vec![[0, 4]],
            ..hotspot.clone()
        };
        let exact_result = attribute(
            Sidecars {
                capture: &capture,
                report_dir: &report,
                entry_name: "frame.png",
            },
            HotspotPixels {
                errors: &errors,
                mask: None,
                width: 4,
                height: 4,
                hotspots: &[exact],
                cutoff: 0.1,
            },
        )
        .expect("exact component attribution");
        assert_eq!(exact_result[0].hotspots[0].measured_hot_pixels, 4);
        assert_eq!(exact_result[0].hotspots[0].contributions.len(), 1);
        assert_eq!(exact_result[0].hotspots[0].contributions[0].name, "tree");
        for source in &result {
            assert!(report.join(&source.image_path).is_file());
            assert!(report.join(&source.legend_path).is_file());
            assert_eq!(
                crate::run::sha256_file(&report.join(&source.image_path)).expect("hash"),
                source.image_sha256
            );
        }
        let collected = crate::run::collect_images(dir.path()).expect("collect");
        assert!(collected.files.contains_key("frame.png"));
        assert!(!collected.files.contains_key("frame.object-id.png"));
        assert!(!collected.files.contains_key("frame.material-id.exr"));
    }

    #[test]
    fn incomplete_sidecars_are_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let capture = dir.path().join("x.png");
        image::RgbImage::new(4, 4)
            .save(dir.path().join("x.object-id.png"))
            .expect("IDs");
        let error = attribute(
            Sidecars {
                capture: &capture,
                report_dir: dir.path(),
                entry_name: "x.png",
            },
            HotspotPixels {
                errors: &[0.2; 16],
                mask: None,
                width: 4,
                height: 4,
                hotspots: &[],
                cutoff: 0.1,
            },
        )
        .expect_err("missing legend");
        assert!(error.to_string().contains("exactly one PNG or EXR"));
    }
}
