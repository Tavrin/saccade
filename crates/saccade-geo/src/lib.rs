//! Native raster measurements and tile coverage through a first-party extension.
//! Georeferencing is evidence; no reprojection or resampling is performed.
pub mod input;
pub mod tiles;
use input::Raster;
use serde_json::{Value, json};
use std::path::Path;

/// Raster report discriminator.
pub const SCHEMA: &str = "saccade-raster.v1";
/// Class-raster report discriminator.
pub const MASK_SCHEMA: &str = "saccade-raster-mask.v1";

/// Typed extension errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Filesystem failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// TIFF decoding failure.
    #[error(transparent)]
    Tiff(#[from] tiff::TiffError),
    /// Raster encoding or image decoding failure.
    #[error(transparent)]
    Image(#[from] image::ImageError),
    /// Shared measurement failure.
    #[error(transparent)]
    Core(#[from] saccade_core::Error),
    /// Invalid or unsupported input/policy.
    #[error("{0}")]
    Input(String),
    /// Exact grid mismatch, preserving both descriptions.
    #[error(
        "different CRS/grid; reference: {reference:?}; candidate: {candidate:?}; reproject/resample externally"
    )]
    Grid {
        /// Reference grid.
        reference: Box<input::Grid>,
        /// Candidate grid.
        candidate: Box<input::Grid>,
    },
}
impl Error {
    /// Transport error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Grid { .. } => "different_crs_grid",
            Self::Input(_) => "invalid_raster_input",
            _ => "raster_error",
        }
    }
}
/// Extension result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Explicit visualisation of three native bands; indices are one-based.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RgbMapping {
    /// Native band indices mapped to R, G, B (repeated indices are permitted).
    pub bands: [usize; 3],
    /// Fixed native-unit lower bounds, shared by both inputs.
    pub min: [f64; 3],
    /// Fixed native-unit upper bounds, shared by both inputs.
    pub max: [f64; 3],
}
fn mapping_valid(m: &RgbMapping, bands: usize) -> Result<()> {
    for c in 0..3 {
        if m.bands[c] == 0
            || m.bands[c] > bands
            || !m.min[c].is_finite()
            || !m.max[c].is_finite()
            || m.min[c] >= m.max[c]
            || !(m.max[c] - m.min[c]).is_finite()
        {
            return Err(Error::Input(
                "RGB needs three existing one-based bands and finite increasing native-unit ranges"
                    .into(),
            ));
        }
    }
    Ok(())
}
fn preview(r: &Raster, m: &RgbMapping, excluded: &[bool]) -> image::RgbaImage {
    let mut image = image::RgbaImage::new(r.grid.width, r.grid.height);
    for (i, pixel) in image.pixels_mut().enumerate() {
        let mut rgb = [0, 0, 0, 255];
        if !excluded[i] {
            for (c, channel) in rgb[..3].iter_mut().enumerate() {
                let v = r.values[i * r.bands + m.bands[c] - 1];
                *channel =
                    (((v - m.min[c]) / (m.max[c] - m.min[c])).clamp(0., 1.) * 255.).round() as u8;
            }
        }
        *pixel = image::Rgba(rgb);
    }
    image
}
fn write_delta(path: &Path, w: u32, h: u32, delta: &[f64]) -> Result<()> {
    let mut encoder = tiff::encoder::TiffEncoder::new(std::fs::File::create(path)?)?;
    encoder.write_image::<tiff::encoder::colortype::Gray64Float>(w, h, delta)?;
    Ok(())
}
/// Compare matching-grid rasters in native units and write per-band change maps.
/// Measurements have no quality threshold and do not confer acceptance authority.
pub fn compare(
    reference: &Path,
    candidate: &Path,
    out: &Path,
    rgb: Option<&RgbMapping>,
) -> Result<Value> {
    let a = input::read(reference)?;
    let b = input::read(candidate)?;
    input::same_grid(&a, &b)?;
    if a.bands != b.bands {
        return Err(Error::Input("band counts differ".into()));
    }
    if let Some(m) = rgb {
        mapping_valid(m, a.bands)?;
    }
    prepare_out(out, &[reference, candidate])?;
    let n = a.grid.width as usize * a.grid.height as usize;
    let mut bands = Vec::with_capacity(a.bands);
    for band in 0..a.bands {
        let mut change = image::GrayImage::new(a.grid.width, a.grid.height);
        let mut delta = vec![f64::NAN; n];
        let (mut count, mut changed, mut both, mut only_a, mut only_b) =
            (0u64, 0u64, 0u64, 0u64, 0u64);
        let (mut sum, mut abs, mut squares, mut min, mut max) =
            (0f64, 0f64, 0f64, f64::INFINITY, f64::NEG_INFINITY);
        for (i, pixel) in change.pixels_mut().enumerate() {
            let (x, y) = (a.values[i * a.bands + band], b.values[i * b.bands + band]);
            let code = match (a.is_nodata(x), b.is_nodata(y)) {
                (true, true) => {
                    both += 1;
                    128
                }
                (true, false) => {
                    only_a += 1;
                    64
                }
                (false, true) => {
                    only_b += 1;
                    192
                }
                (false, false) => {
                    let d = y - x;
                    delta[i] = d;
                    count += 1;
                    sum += d;
                    abs += d.abs();
                    squares += d * d;
                    min = min.min(d);
                    max = max.max(d);
                    if d != 0. {
                        changed += 1;
                        255
                    } else {
                        0
                    }
                }
            };
            *pixel = image::Luma([code]);
        }
        let stem = format!("band-{}", band + 1);
        change.save(out.join(format!("{stem}-change.png")))?;
        write_delta(
            &out.join(format!("{stem}-delta.tiff")),
            a.grid.width,
            a.grid.height,
            &delta,
        )?;
        let stats = if count == 0 {
            Value::Null
        } else {
            json!({"mean_delta":sum/count as f64,"mean_absolute_delta":abs/count as f64,"rmse":(squares/count as f64).sqrt(),"min_delta":min,"max_delta":max})
        };
        bands.push(json!({"band":band+1,"valid_pairs":count,"changed_pixels":changed,"nodata":{"both":both,"reference_only":only_a,"candidate_only":only_b},"statistics":stats,"change_map":format!("{stem}-change.png"),"delta_map":format!("{stem}-delta.tiff")}));
    }
    let perceptual = if let Some(m) = rgb {
        let excluded: Vec<bool> = (0..n)
            .map(|i| {
                m.bands.iter().any(|band| {
                    a.is_nodata(a.values[i * a.bands + band - 1])
                        || b.is_nodata(b.values[i * b.bands + band - 1])
                })
            })
            .collect();
        let valid = excluded.iter().filter(|v| !**v).count();
        if valid == 0 {
            return Err(Error::Input("RGB mapping has no valid pixel pairs".into()));
        }
        let ap = preview(&a, m, &excluded);
        let bp = preview(&b, m, &excluded);
        ap.save(out.join("reference-rgb.png"))?;
        bp.save(out.join("candidate-rgb.png"))?;
        let comparison = saccade_core::compare::compare_rgba_masked(
            &bp,
            &ap,
            &Default::default(),
            Some(&excluded),
            saccade_core::compare::MaskMode::Neutralize,
        )?;
        let mut heatmap = comparison.heatmap_rgb();
        saccade_core::compare::hatch_masked(&mut heatmap, &excluded);
        heatmap.save(out.join("rgb-flip.png"))?;
        json!({"mapping":m,"transfer":"native ranges linearly mapped to sRGB bytes, clipped and rounded","mask_mode":"neutralize","valid_pairs":valid,"metrics":comparison.metrics,"heatmap":"rgb-flip.png"})
    } else {
        Value::Null
    };
    Ok(saccade_core::report_links::decorate(
        &json!({"schema":SCHEMA,"operation":"raster_compare","verdict":"measured","reference":a.description(),"candidate":b.description(),"policy":{"grid":"exact metadata equality; no reprojection or resampling","delta":"candidate minus reference, native units","nodata":"exclude either side per band; retain one-sided counts","change_map_codes":{"equal":0,"changed":255,"both_nodata":128,"reference_nodata":64,"candidate_nodata":192},"delta_map":"float64 TIFF, NaN for excluded pairs"},"bands":bands,"perceptual":perceptual}),
    )?)
}

/// Evaluate single-band class rasters with the existing overlap/boundary metrics.
/// Nodata in either raster is excluded before the shared evaluator is called.
pub fn mask_metrics(
    reference: &Path,
    candidate: &Path,
    policy: &saccade_core::mask_metrics::Policy,
) -> Result<Value> {
    use saccade_core::{
        evidence_quality::layers::Predicate,
        mask_metrics::{ClassSpec, Labels},
    };
    let a = input::read(reference)?;
    let b = input::read(candidate)?;
    input::same_grid(&a, &b)?;
    if a.bands != 1 || b.bands != 1 {
        return Err(Error::Input("class rasters must be single-band".into()));
    }
    if let Some(void) = &policy.void {
        void.predicate.validate()?;
        if matches!(void.predicate, Predicate::Labels { .. }) {
            return Err(Error::Input(
                "label-glob void predicates need a dictionary".into(),
            ));
        }
    }
    let declared_policy = json!({
        "classes": match &policy.classes {
            saccade_core::mask_metrics::ClassSelection::Foreground => "foreground",
            saccade_core::mask_metrics::ClassSelection::EachLabel => "each_label",
            saccade_core::mask_metrics::ClassSelection::Named(_) => "named",
        },
        "named": if let saccade_core::mask_metrics::ClassSelection::Named(classes) = &policy.classes {
            classes.iter().map(|c| json!({"name":c.name,"predicate":c.predicate})).collect::<Vec<_>>()
        } else { Vec::new() },
        "void": policy.void.as_ref().map(|c| json!({"name":c.name,"predicate":c.predicate})),
        "boundary_tolerance_px":policy.boundary_tolerance_px,
    });
    let mut av = Vec::new();
    let mut bv = Vec::new();
    let mut excluded = Vec::new();
    let mut used = std::collections::BTreeSet::new();
    for (&x, &y) in a.values.iter().zip(&b.values) {
        let void = a.is_nodata(x)
            || b.is_nodata(y)
            || policy.void.as_ref().is_some_and(|c| c.predicate.matches(x));
        excluded.push(void);
        let mut labels = [0u32; 2];
        if !void {
            for (v, dest) in [x, y].into_iter().zip(&mut labels) {
                if !v.is_finite() || !(0. ..=f64::from(u32::MAX)).contains(&v) || v.fract() != 0. {
                    return Err(Error::Input(
                        "class values must be exact non-negative u32 integers".into(),
                    ));
                }
                *dest = v as u32;
                used.insert(*dest);
            }
        }
        av.push(labels[0]);
        bv.push(labels[1]);
    }
    let sentinel = (0..=u32::MAX)
        .find(|v| !used.contains(v))
        .ok_or_else(|| Error::Input("no unused void label".into()))?;
    for (i, void) in excluded.iter().enumerate() {
        if *void {
            av[i] = sentinel;
            bv[i] = sentinel;
        }
    }
    let mut effective = policy.clone();
    effective.void = Some(ClassSpec {
        name: "nodata_or_declared_void".into(),
        predicate: Predicate::Ids {
            values: vec![sentinel],
        },
    });
    let reference = Labels::new(a.grid.width, a.grid.height, av)?;
    let candidate = Labels::new(b.grid.width, b.grid.height, bv)?;
    let metrics = saccade_core::mask_metrics::evaluate(&candidate, &reference, &effective)?;
    Ok(saccade_core::report_links::decorate(
        &json!({"schema":MASK_SCHEMA,"operation":"raster_mask_metrics","verdict":"measured","reference":a.description(),"candidate":b.description(),"declared_policy":declared_policy,"excluded_pixels":excluded.iter().filter(|v| **v).count(),"exclusion":"nodata on either side or declared reference void; internal unused sentinel","metrics":metrics}),
    )?)
}

pub(crate) fn prepare_out(out: &Path, inputs: &[&Path]) -> Result<()> {
    saccade_core::run::guard_output_dir(out, inputs, &[])?;
    if out.exists() && std::fs::read_dir(out)?.next().is_some() {
        return Err(Error::Input("output directory must be empty".into()));
    }
    std::fs::create_dir_all(out)?;
    Ok(())
}
