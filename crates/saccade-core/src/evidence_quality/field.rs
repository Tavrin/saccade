//! Per-ID shifts and explicit comparison scope; numerical units stay declared.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// Recorded field-analysis controls shared by colour and numerical buffers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    /// Require a supplied inclusion/exclusion mask or ID layer.
    pub require_scope: bool,
    /// Number of highest-contribution IDs retained, at most 32.
    pub top: usize,
    /// Absolute normalized luminance threshold for colour samples.
    pub threshold: f64,
    /// Write numeric FLIP and tile grids alongside the report.
    pub export_maps: bool,
    /// Repeat input files or directories for inline noise estimation.
    pub noise_from: Vec<std::path::PathBuf>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            require_scope: false,
            top: 5,
            threshold: 0.02,
            export_maps: false,
            noise_from: Vec::new(),
        }
    }
}
impl Policy {
    /// Validate controls before output acquisition.
    pub fn validate(&self) -> Result<()> {
        if self.top > 32
            || !self.threshold.is_finite()
            || self.threshold < 0.0
            || (!self.noise_from.is_empty() && !(2..=32).contains(&self.noise_from.len()))
        {
            return Err(Error::Config(
                "invalid field top/threshold/repeat count".into(),
            ));
        }
        Ok(())
    }
}
/// Native ID samples retained with their source identity.
#[derive(Debug, Clone, PartialEq)]
pub struct IdLayer {
    /// Native label per pixel.
    pub values: Vec<u32>,
    /// Names when supplied; unknown IDs retain their number.
    pub names: BTreeMap<u32, String>,
    /// Source description, including derived-dump confidence.
    pub provenance: String,
}
/// Signed and unsigned statistics over one ID's actual included samples.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Statistics {
    /// Candidate minus baseline mean (angular differences use nonnegative angles).
    pub signed_mean_shift: f64,
    /// Mean absolute difference in the declared unit.
    pub mean_absolute: f64,
    /// Nearest-rank p95 absolute difference.
    pub p95: f64,
    /// Declared per-pixel difference threshold.
    pub threshold: f64,
    /// Fraction strictly over threshold, in `[0,1]`.
    #[cfg_attr(
        feature = "schema",
        schemars(description = "Fraction strictly over threshold, in [0,1].")
    )]
    pub share_over_threshold: f64,
}
/// One ID, ranked by its total absolute-difference contribution.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdShift {
    /// Native object/material/instance label.
    pub id: u32,
    /// Supplied name or numeric label.
    pub name: String,
    /// Included pixels in this ID's baseline/candidate footprint union.
    pub pixels: u64,
    /// Bounding box of those included samples.
    pub rect_px: [u32; 4],
    /// Luminance, depth, degrees, pixels or changed_fraction.
    pub unit: String,
    /// Primary shift statistics (angular statistics for normal buffers).
    pub shift: Statistics,
    /// Depth relative difference, denominator max(abs(baseline),1e-12).
    pub relative: Option<Statistics>,
    /// Sum absolute error, split equally for pixels that switch IDs.
    pub contribution: f64,
    /// Contribution divided by total scoped error, including omitted IDs.
    pub contribution_share: f64,
    /// Portable base | candidate | error crop strip, if generated.
    pub crop: Option<String>,
}
/// Additive field evidence in ordinary comparison verdicts.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Structural/numerical class; the deciding metric remains independently recorded.
    pub class: String,
    /// Explicit scope: whole_frame_unmasked, masked, id_layer or id_layer_masked.
    pub scope: String,
    /// Unit-specific ID summaries; truncation is recorded.
    pub per_id: Vec<IdShift>,
    /// Total IDs measured before top-N truncation.
    pub ids_measured: usize,
    /// Membership source; dump-derived layers explicitly have lower confidence.
    pub provenance: Vec<String>,
    /// Numeric map JSON index relative to the bundle.
    pub maps: Option<String>,
    /// Repeat-derived noise decision when requested.
    pub noise: Option<super::repeat_noise::Decision>,
}
/// Scope description is computed from actual inputs, not a policy declaration.
pub fn scope(ids: bool, masked: bool) -> &'static str {
    match (ids, masked) {
        (false, false) => "whole_frame_unmasked",
        (false, true) => "masked",
        (true, false) => "id_layer",
        (true, true) => "id_layer_masked",
    }
}
/// Validate and decode exact integer labels, including RGB24 conventional ID images.
pub fn ids(image: &image::DynamicImage) -> Result<Vec<u32>> {
    let values: Vec<f64> = match image {
        image::DynamicImage::ImageLuma8(i) => i.pixels().map(|p| f64::from(p[0])).collect(),
        image::DynamicImage::ImageLuma16(i) => i.pixels().map(|p| f64::from(p[0])).collect(),
        image::DynamicImage::ImageRgb32F(i) => i.pixels().map(|p| f64::from(p[0])).collect(),
        image::DynamicImage::ImageRgba32F(i) => i.pixels().map(|p| f64::from(p[0])).collect(),
        image::DynamicImage::ImageRgb8(i) => i
            .pixels()
            .map(|p| f64::from((u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2])))
            .collect(),
        _ => {
            return Err(Error::Config(
                "ID layer requires L8, L16, RGB24 or float R samples".into(),
            ));
        }
    };
    if values
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0 || *v > 16_777_215.0 || v.fract() != 0.0)
    {
        return Err(Error::Config(
            "ID layer contains invalid integer labels".into(),
        ));
    }
    Ok(values.into_iter().map(|v| v as u32).collect())
}
fn statistics(values: &[f64], threshold: f64) -> Statistics {
    let mut abs: Vec<_> = values.iter().map(|v| v.abs()).collect();
    abs.sort_by(f64::total_cmp);
    let n = values.len();
    Statistics {
        signed_mean_shift: values.iter().sum::<f64>() / n as f64,
        mean_absolute: abs.iter().sum::<f64>() / n as f64,
        p95: abs[(n as f64 * 0.95).ceil() as usize - 1],
        threshold,
        share_over_threshold: abs.iter().filter(|v| **v > threshold).count() as f64 / n as f64,
    }
}
/// Attribute signed differences to unions of stable label footprints. Switched-ID
/// pixels split their ranking contribution equally; their statistics remain per pixel.
/// Returns retained rows, total ID count, and whether any ID mean absolute difference
/// exceeds the threshold (computed before truncation).
#[allow(clippy::too_many_arguments)]
pub fn attribute(
    b: &IdLayer,
    c: &IdLayer,
    dimensions: (u32, u32),
    signed: &[f64],
    relative: Option<&[f64]>,
    excluded: Option<&[bool]>,
    unit: &str,
    threshold: f64,
    top: usize,
) -> Result<(Vec<IdShift>, usize, bool)> {
    let (w, h) = dimensions;
    let n = u64::from(w) * u64::from(h);
    if n == 0
        || n > 16_777_216
        || signed.len() != n as usize
        || b.values.len() != signed.len()
        || c.values.len() != signed.len()
        || relative.is_some_and(|v| v.len() != signed.len())
        || excluded.is_some_and(|v| v.len() != signed.len())
        || signed.iter().any(|v| !v.is_finite())
        || relative.is_some_and(|v| v.iter().any(|v| !v.is_finite()))
        || !threshold.is_finite()
        || threshold < 0.0
        || top > 32
    {
        return Err(Error::Config(
            "invalid ID attribution samples/policy".into(),
        ));
    }
    let mut groups: BTreeMap<u32, Vec<(usize, f64)>> = BTreeMap::new();
    for i in 0..signed.len() {
        if excluded.is_some_and(|m| m[i]) {
            continue;
        }
        let switched = b.values[i] != c.values[i];
        groups
            .entry(b.values[i])
            .or_default()
            .push((i, if switched { 0.5 } else { 1.0 }));
        if switched {
            groups.entry(c.values[i]).or_default().push((i, 0.5));
        }
        if groups.len() > 65536 {
            return Err(Error::Config("ID attribution exceeds label budget".into()));
        }
    }
    let total: f64 = groups
        .values()
        .flat_map(|v| v.iter())
        .map(|(i, weight)| signed[*i].abs() * weight)
        .sum();
    let count = groups.len();
    let mut result = Vec::new();
    for (id, indexes) in groups {
        let values: Vec<_> = indexes.iter().map(|(i, _)| signed[*i]).collect();
        let contribution = indexes
            .iter()
            .map(|(i, weight)| signed[*i].abs() * weight)
            .sum::<f64>();
        let min_x = indexes
            .iter()
            .map(|(i, _)| *i as u32 % w)
            .min()
            .unwrap_or(0);
        let max_x = indexes
            .iter()
            .map(|(i, _)| *i as u32 % w)
            .max()
            .unwrap_or(0);
        let min_y = indexes
            .iter()
            .map(|(i, _)| *i as u32 / w)
            .min()
            .unwrap_or(0);
        let max_y = indexes
            .iter()
            .map(|(i, _)| *i as u32 / w)
            .max()
            .unwrap_or(0);
        result.push(IdShift {
            id,
            name: c
                .names
                .get(&id)
                .or_else(|| b.names.get(&id))
                .cloned()
                .unwrap_or_else(|| id.to_string()),
            pixels: indexes.len() as u64,
            rect_px: [min_x, min_y, max_x - min_x + 1, max_y - min_y + 1],
            unit: unit.into(),
            shift: statistics(&values, threshold),
            relative: relative.map(|r| {
                statistics(
                    &indexes.iter().map(|(i, _)| r[*i]).collect::<Vec<_>>(),
                    threshold,
                )
            }),
            contribution,
            contribution_share: if total > 0.0 {
                contribution / total
            } else {
                0.0
            },
            crop: None,
        });
    }
    result.sort_by(|a, b| {
        b.contribution
            .total_cmp(&a.contribution)
            .then_with(|| a.id.cmp(&b.id))
    });
    let mean_over_threshold = result.iter().any(|r| r.shift.mean_absolute > threshold);
    result.truncate(top);
    Ok((result, count, mean_over_threshold))
}
/// Write bounded top-ID crop strips using the existing report display conventions.
pub fn crops(
    rows: &mut [IdShift],
    base: &image::RgbaImage,
    candidate: &image::RgbaImage,
    errors: &[f32],
    out: &std::path::Path,
    name: &str,
) -> Result<()> {
    let (w, h) = base.dimensions();
    if candidate.dimensions() != (w, h) || errors.len() != w as usize * h as usize {
        return Err(Error::Config("ID crop dimensions differ".into()));
    }
    let directory = out.join("regions");
    std::fs::create_dir_all(&directory).map_err(crate::run::io_err("creating ID crops".into()))?;
    let digest = crate::localized::digest(name.as_bytes());
    let heat = crate::compare::Comparison {
        metrics: crate::compare::metrics_of(errors, w, h),
        error_map: errors.to_vec(),
    }
    .heatmap_rgb();
    for row in rows {
        let [x, y, rw, rh] = row.rect_px;
        let step = rw.max(rh).div_ceil(512).max(1);
        let (cw, ch) = (rw.div_ceil(step), rh.div_ceil(step));
        let strip = image::RgbImage::from_fn(cw * 3, ch, |xx, yy| {
            let px = x + (xx % cw * step).min(rw - 1);
            let py = y + (yy * step).min(rh - 1);
            match xx / cw {
                0 => image::Rgb(base.get_pixel(px, py).0[..3].try_into().unwrap_or([0; 3])),
                1 => image::Rgb(
                    candidate.get_pixel(px, py).0[..3]
                        .try_into()
                        .unwrap_or([0; 3]),
                ),
                _ => *heat.get_pixel(px, py),
            }
        });
        let path = format!("regions/{}-id-{}.png", &digest[..16], row.id);
        strip
            .save(out.join(&path))
            .map_err(|source| Error::Encode {
                path: out.join(&path),
                source,
            })?;
        row.crop = Some(path);
    }
    Ok(())
}

#[cfg(all(test, feature = "graphics"))]
mod tests {
    use super::*;
    use crate::evidence_quality::layers;
    fn fixture(root: &std::path::Path, normal: bool, candidate: bool) -> std::path::PathBuf {
        std::fs::create_dir_all(root).unwrap();
        let path = root.join(if normal { "normal.exr" } else { "depth.png" });
        if normal {
            image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_fn(64, 64, |x, _| {
                let angle = if candidate && x >= 48 {
                    15f32.to_radians()
                } else {
                    0.0
                };
                image::Rgb([(angle.sin() + 1.0) / 2.0, 0.5, (angle.cos() + 1.0) / 2.0])
            }))
            .save(&path)
            .unwrap();
        } else {
            image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(64, 64, |x, _| {
                image::Luma([if candidate && x >= 48 { 22000 } else { 20000 }])
            })
            .save(&path)
            .unwrap();
        }
        image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(64, 64, |x, _| {
            image::Luma([if x >= 48 { 7 } else { 1 }])
        })
        .save(root.join("ids.png"))
        .unwrap();
        let manifest = layers::Manifest {
            labels: Default::default(),
            schema: layers::SCHEMA.into(),
            image_sha256: crate::run::sha256_file(&path).unwrap(),
            dimensions: [64, 64],
            layers: vec![layers::Layer {
                name: "surface".into(),
                image: "ids.png".into(),
                kind: layers::Kind::Id,
                sha256: None,
                scale: 1.0,
                colour_space: Default::default(),
                additive: false,
            }],
        };
        std::fs::write(
            path.with_file_name(format!(
                "{}.layers.json",
                path.file_name().unwrap().to_str().unwrap()
            )),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        path
    }
    #[test]
    fn one_id_normal_rotation_is_systematic_and_ranked_first() {
        let tmp = tempfile::tempdir().unwrap();
        let b = fixture(&tmp.path().join("b"), true, false);
        let c = fixture(&tmp.path().join("c"), true, true);
        let cfg=crate::config::RunConfig::from_toml_str("[[buffer]]\nglob='normal.exr'\nkind='normal'\nthreshold=2.0\n[buffer.capture_layers]\nattribution=false\n").unwrap();
        let out = tmp.path().join("out");
        let r = crate::run::run(&b, &c, &out, &cfg).unwrap();
        let field = r.entries[0].field_evidence.as_ref().unwrap();
        assert_eq!(field.class, "systematic_shift");
        assert_eq!(field.per_id[0].id, 7);
        assert_eq!(field.per_id[0].pixels, 1024);
        assert!((field.per_id[0].shift.mean_absolute - 15.0).abs() < 0.001);
        assert!((field.per_id[0].shift.p95 - 15.0).abs() < 0.001);
        assert_eq!(field.per_id[0].shift.share_over_threshold, 1.0);
        assert_eq!(field.per_id[0].contribution_share, 1.0);
        assert!(out.join(field.per_id[0].crop.as_ref().unwrap()).exists());
        let html = std::fs::read_to_string(out.join("index.html")).unwrap();
        assert!(html.contains("Per-ID shifts"));
        assert!(html.contains("degrees"));
        assert!(r.is_regression());
    }
    #[test]
    fn depth_retains_signed_absolute_and_relative_statistics() {
        let tmp = tempfile::tempdir().unwrap();
        let b = fixture(&tmp.path().join("b"), false, false);
        let c = fixture(&tmp.path().join("c"), false, true);
        let cfg=crate::config::RunConfig::from_toml_str("[[buffer]]\nglob='depth.png'\nkind='depth'\nthreshold=0.01\n[buffer.capture_layers]\nattribution=false\n").unwrap();
        let r = crate::run::run(&b, &c, &tmp.path().join("out"), &cfg).unwrap();
        let row = &r.entries[0].field_evidence.as_ref().unwrap().per_id[0];
        assert_eq!(row.id, 7);
        assert!((row.shift.mean_absolute - 2000.0 / 65535.0).abs() < 1e-6);
        assert!((row.shift.signed_mean_shift - row.shift.mean_absolute).abs() < 1e-12);
        assert!((row.relative.as_ref().unwrap().mean_absolute - 0.1).abs() < 1e-6);
    }
    #[test]
    fn colour_scope_dump_and_required_scope_are_visible() {
        let tmp = tempfile::tempdir().unwrap();
        let b = tmp.path().join("b");
        let c = tmp.path().join("c");
        for root in [&b, &c] {
            std::fs::create_dir(root).unwrap();
            image::RgbImage::from_pixel(32, 32, image::Rgb([100; 3]))
                .save(root.join("frame.png"))
                .unwrap();
            std::fs::write(
                root.join("instances.json"),
                br#"[{"id":3,"name":"surface","boxes":[[0,0,16,32]]}]"#,
            )
            .unwrap();
        }
        let mut cfg = crate::config::RunConfig::default();
        let r = crate::run::run(
            &b.join("frame.png"),
            &c.join("frame.png"),
            &tmp.path().join("unmasked"),
            &cfg,
        )
        .unwrap();
        assert_eq!(
            r.entries[0].field_evidence.as_ref().unwrap().scope,
            "whole_frame_unmasked"
        );
        assert!(
            std::fs::read_to_string(tmp.path().join("unmasked/index.html"))
                .unwrap()
                .contains("whole_frame_unmasked")
        );
        cfg.field.require_scope = true;
        let r = crate::run::run(
            &b.join("frame.png"),
            &c.join("frame.png"),
            &tmp.path().join("required"),
            &cfg,
        )
        .unwrap();
        assert!(r.is_regression());
        assert!(
            r.entries[0]
                .error
                .as_ref()
                .unwrap()
                .contains("requires scope")
        );
        cfg.layers = Some(layers::Policy {
            dump: Some("instances.json".into()),
            scope: Some(layers::LayerScope {
                layer: "derived_instances".into(),
                predicate: layers::Predicate::Mask,
                mode: Default::default(),
            }),
            ..Default::default()
        });
        let r = crate::run::run(
            &b.join("frame.png"),
            &c.join("frame.png"),
            &tmp.path().join("dump"),
            &cfg,
        )
        .unwrap();
        let f = r.entries[0].field_evidence.as_ref().unwrap();
        assert_eq!(f.scope, "id_layer_masked");
        assert_eq!(f.per_id[0].pixels, 512);
        assert!(f.provenance[0].contains("derived from dump; lower confidence"));
        assert!(!r.is_regression());
    }
    #[test]
    fn switched_ids_split_contribution_and_exclusions_apply() {
        let b = IdLayer {
            values: vec![1, 1, 2, 2],
            names: Default::default(),
            provenance: "native".into(),
        };
        let c = IdLayer {
            values: vec![1, 2, 2, 2],
            ..b.clone()
        };
        let (rows, count, _) = attribute(
            &b,
            &c,
            (2, 2),
            &[1.0, -2.0, 3.0, 100.0],
            None,
            Some(&[false, false, false, true]),
            "units",
            1.5,
            5,
        )
        .unwrap();
        assert_eq!(count, 2);
        assert_eq!(rows[0].id, 2);
        assert_eq!(rows[0].contribution, 4.0);
        assert_eq!(rows[1].contribution, 2.0);
        assert!((rows.iter().map(|r| r.contribution_share).sum::<f64>() - 1.0).abs() < 1e-12);
    }
    #[test]
    fn inline_repeat_noise_and_maps_preserve_explicit_hotspot_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let image = |name: &str, v: u8| {
            let p = tmp.path().join(name);
            image::RgbaImage::from_pixel(32, 32, image::Rgba([v, v, v, 255]))
                .save(&p)
                .unwrap();
            p
        };
        let base = image("base.png", 100);
        let candidate = image("candidate.png", 101);
        let shifted = image("shift.png", 110);
        let mut cfg = crate::config::RunConfig::default();
        cfg.field.noise_from = vec![image("repeat-a.png", 99), image("repeat-b.png", 101)];
        cfg.field.export_maps = true;
        cfg.spatial = Some(crate::evidence_quality::spatial::Policy {
            decide: true,
            ..Default::default()
        });
        for (name, c, expected) in [
            ("noise", &candidate, "texture_noise_only"),
            ("shift", &shifted, "systematic_shift"),
        ] {
            let out = tmp.path().join(name);
            let r = crate::run::run(&base, c, &out, &cfg).unwrap();
            let f = r.entries[0].field_evidence.as_ref().unwrap();
            assert_eq!(f.class, expected);
            assert_eq!(r.is_regression(), name == "shift");
            let index: crate::evidence_quality::maps::Index =
                serde_json::from_slice(&std::fs::read(out.join(f.maps.as_ref().unwrap())).unwrap())
                    .unwrap();
            for map in &index.maps {
                assert!(out.join(&map.npy).exists());
                assert!(out.join(&map.exr).exists());
            }
        }
        // A wide repeat envelope cannot waive the caller's explicit FLIP peak gate.
        cfg.field.noise_from = vec![image("wide-a.png", 90), image("wide-b.png", 120)];
        cfg.hotspot_threshold = 0.000001;
        cfg.hotspot_fail = Some(0.000001);
        let r = crate::run::run(&base, &shifted, &tmp.path().join("hotspot"), &cfg).unwrap();
        assert_eq!(
            r.entries[0].field_evidence.as_ref().unwrap().class,
            "texture_noise_only"
        );
        assert!(r.is_regression());
    }
}
