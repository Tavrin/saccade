//! Repeat-derived empirical noise envelopes, without an independence assumption.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
/// Repeat noise artifact discriminator.
pub const SCHEMA: &str = "saccade-repeat-noise.v1";
/// Numerical differences: normalized sRGB signed luminance and RGB RMS over black.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// Pixel footprint.
    pub rect_px: [u32; 4],
    /// Included pixels.
    pub pixels: u64,
    /// Maximum pairwise absolute signed mean.
    pub mean_limit: f64,
    /// Maximum pairwise root mean squared difference.
    pub rms_limit: f64,
}
/// Noise envelope for one capture name.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Floor {
    /// Repeat content identities in declared order.
    pub repeat_sha256: Vec<String>,
    /// Width/height.
    pub dimensions: [u32; 2],
    /// Tile width/height.
    pub tile_size: u32,
    /// Maximum pairwise global signed-mean magnitude.
    pub global_mean_limit: f64,
    /// Maximum pairwise global RMS.
    pub global_rms_limit: f64,
    /// Tile envelopes.
    pub tiles: Vec<Tile>,
}
/// Persisted per-entry repeat noise floors.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Versioned discriminator.
    pub schema: String,
    /// max_pairwise_rgb_luminance_envelope_v1, fully deterministic.
    pub method: String,
    /// Repeat capture names and floors.
    pub entries: std::collections::BTreeMap<String, Floor>,
    /// Declaration/interpretation limits.
    pub limits: Vec<String>,
}
/// Pair result relative to an empirical noise floor.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    /// texture_noise_only or systematic_shift.
    pub class: String,
    /// Exact repeat-derived floor, including source identities and method limits.
    pub floor: Floor,
    /// Pairwise global signed luminance shift.
    pub signed_mean: f64,
    /// Global RMS difference.
    pub rms: f64,
    /// Tile indexes above either empirical envelope.
    pub beyond_tiles: Vec<usize>,
    /// Method and fixed quantization tolerance.
    pub method: String,
}
fn luma(img: &image::RgbaImage) -> Vec<[f64; 3]> {
    super::spatial::rgb(img)
}
fn moments(
    a: &[[f64; 3]],
    b: &[[f64; 3]],
    w: u32,
    rect: [u32; 4],
    excluded: Option<&[bool]>,
) -> (f64, f64, u64) {
    let [x, y, rw, rh] = rect;
    let (mut sum, mut sq, mut n) = (0.0, 0.0, 0u64);
    for yy in y..y + rh {
        for xx in x..x + rw {
            let i = (yy * w + xx) as usize;
            if excluded.is_some_and(|m| m[i]) {
                continue;
            }
            let d: [f64; 3] = std::array::from_fn(|c| b[i][c] - a[i][c]);
            sum += super::spatial::luminance(d);
            sq += d.iter().map(|v| v * v).sum::<f64>() / 3.0;
            n += 1;
        }
    }
    if n == 0 {
        (0.0, 0.0, 0)
    } else {
        (sum / n as f64, (sq / n as f64).sqrt(), n)
    }
}
/// Resolve one same-name capture from each file or run directory.
pub fn paths(repeats: &[PathBuf], name: &str) -> Result<Vec<PathBuf>> {
    if !(2..=32).contains(&repeats.len()) {
        return Err(Error::Config(
            "noise requires 2..32 repeats of the same arm".into(),
        ));
    }
    repeats
        .iter()
        .map(|p| {
            if p.is_dir() {
                super::relative(p, name)
            } else {
                Ok(p.clone())
            }
        })
        .collect()
}
/// Build an empirical max-over-all-repeat-pairs envelope on the actual inclusion scope.
pub fn build(paths: &[PathBuf], tile_size: u32, excluded: Option<&[bool]>) -> Result<Floor> {
    if !(2..=32).contains(&paths.len()) || !(4..=4096).contains(&tile_size) {
        return Err(Error::Config("invalid repeat noise count/grid".into()));
    }
    let mut images = Vec::new();
    let mut hashes = Vec::new();
    let mut dims = None;
    for path in paths {
        let bytes = super::read(path, 128 << 20)?;
        let decoded = super::decode(&bytes, path)?;
        if matches!(
            decoded,
            image::DynamicImage::ImageRgb32F(_) | image::DynamicImage::ImageRgba32F(_)
        ) {
            return Err(Error::Config("repeat noise requires SDR colour captures; native float/HDR samples need their own noise model".into()));
        }
        let img = decoded.to_rgba8();
        if dims.is_some_and(|d| d != img.dimensions()) {
            return Err(Error::Config("repeat noise geometry differs".into()));
        }
        dims = Some(img.dimensions());
        let pixels = u64::from(img.width()) * u64::from(img.height());
        if pixels * paths.len() as u64 * (paths.len() - 1) as u64 / 2 > 64 * 1024 * 1024 {
            return Err(Error::Config(
                "repeat pairwise work exceeds pixel budget".into(),
            ));
        }
        images.push(luma(&img));
        hashes.push(crate::localized::digest(&bytes));
    }
    let (w, h) = dims.ok_or_else(|| Error::Config("no repeats".into()))?;
    if excluded.is_some_and(|m| m.len() != w as usize * h as usize || m.iter().all(|v| *v)) {
        return Err(Error::Config("invalid repeat noise scope".into()));
    }
    let mut tiles = Vec::new();
    for y in (0..h).step_by(tile_size as usize) {
        for x in (0..w).step_by(tile_size as usize) {
            tiles.push(Tile {
                rect_px: [x, y, tile_size.min(w - x), tile_size.min(h - y)],
                pixels: 0,
                mean_limit: 0.0,
                rms_limit: 0.0,
            });
        }
    }
    let (mut gm, mut gr) = (0.0f64, 0.0f64);
    for i in 0..images.len() {
        for j in i + 1..images.len() {
            let (mean, rms, _) = moments(&images[i], &images[j], w, [0, 0, w, h], excluded);
            gm = gm.max(mean.abs());
            gr = gr.max(rms);
            for tile in &mut tiles {
                let (mean, rms, n) = moments(&images[i], &images[j], w, tile.rect_px, excluded);
                tile.pixels = n;
                tile.mean_limit = tile.mean_limit.max(mean.abs());
                tile.rms_limit = tile.rms_limit.max(rms);
            }
        }
    }
    Ok(Floor {
        repeat_sha256: hashes,
        dimensions: [w, h],
        tile_size,
        global_mean_limit: gm,
        global_rms_limit: gr,
        tiles,
    })
}
/// Decide whether every scoped tile and the whole pair stay inside the repeat envelope.
pub fn decide(
    base: &image::RgbaImage,
    candidate: &image::RgbaImage,
    floor: Floor,
    excluded: Option<&[bool]>,
) -> Result<Decision> {
    let (w, h) = base.dimensions();
    if candidate.dimensions() != (w, h)
        || floor.dimensions != [w, h]
        || excluded.is_some_and(|m| m.len() != w as usize * h as usize)
    {
        return Err(Error::Config("noise comparison geometry differs".into()));
    }
    let (b, c) = (luma(base), luma(candidate));
    let (mean, rms, _) = moments(&b, &c, w, [0, 0, w, h], excluded);
    let epsilon = 1e-12;
    let mut beyond = Vec::new();
    for (i, t) in floor.tiles.iter().enumerate() {
        let (m, r, n) = moments(&b, &c, w, t.rect_px, excluded);
        if n != t.pixels {
            return Err(Error::Config("repeat/pair scope differs".into()));
        }
        if m.abs() > t.mean_limit + epsilon || r > t.rms_limit + epsilon {
            beyond.push(i);
        }
    }
    let shifted = !beyond.is_empty()
        || mean.abs() > floor.global_mean_limit + epsilon
        || rms > floor.global_rms_limit + epsilon;
    Ok(Decision{class:if shifted{"systematic_shift"}else{"texture_noise_only"}.into(),floor,signed_mean:mean,rms,beyond_tiles:beyond,method:"max_pairwise_rgb_luminance_envelope_v1; numeric epsilon=1e-12; finite repeats are an empirical envelope, not population coverage".into()})
}
/// Build every shared image in repeat run directories, or one repeated file pair.
pub fn build_runs(repeats: &[PathBuf], tile_size: u32) -> Result<Report> {
    if repeats.len() < 2 {
        return Err(Error::Config("noise requires repeats".into()));
    }
    let names = if repeats[0].is_dir() {
        crate::run::collect_images(&repeats[0])?
            .files
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    } else {
        vec![
            repeats[0]
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        ]
    };
    if repeats.iter().any(|p| p.is_dir() != repeats[0].is_dir()) {
        return Err(Error::Config(
            "noise repeats must all be files or all run directories".into(),
        ));
    }
    if repeats[0].is_dir() {
        for repeat in repeats.iter().skip(1) {
            let found = crate::run::collect_images(repeat)?
                .files
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            if found != names {
                return Err(Error::Config("repeat capture names differ".into()));
            }
        }
    }
    if names.is_empty() || names.len() > 1024 {
        return Err(Error::Config(
            "noise capture set empty or over budget".into(),
        ));
    }
    let mut entries = std::collections::BTreeMap::new();
    for name in names {
        entries.insert(
            name.clone(),
            build(&paths(repeats, &name)?, tile_size, None)?,
        );
    }
    Ok(Report{schema:SCHEMA.into(),method:"max_pairwise_rgb_luminance_envelope_v1".into(),entries,limits:vec!["Caller declares all repeats are the same arm. Use --require-valid-arms to validate producer identity; hashes bind pixels but do not prove capture conditions.".into(),"Normalized sRGB luminance over black; all repeat pairs contribute maxima. Finite-repeat envelope is not population noise qualification; RGB RMS detects chromatic differences; native buffers require their own noise model.".into()]})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeat_envelope_accepts_noise_and_rejects_bias() {
        let tmp = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for (k, v) in [99, 101, 100].into_iter().enumerate() {
            let p = tmp.path().join(format!("{k}.png"));
            image::RgbaImage::from_pixel(32, 32, image::Rgba([v, v, v, 255]))
                .save(&p)
                .unwrap();
            paths.push(p);
        }
        let floor = build(&paths, 16, None).unwrap();
        assert!(floor.global_rms_limit > 0.0);
        assert_eq!(floor.tiles.len(), 4);
        let b = image::RgbaImage::from_pixel(32, 32, image::Rgba([100, 100, 100, 255]));
        let c = image::RgbaImage::from_pixel(32, 32, image::Rgba([101, 101, 101, 255]));
        assert_eq!(
            decide(&b, &c, floor.clone(), None).unwrap().class,
            "texture_noise_only"
        );
        let chromatic = image::RgbaImage::from_pixel(32, 32, image::Rgba([150, 85, 100, 255]));
        let chromatic_decision = decide(&b, &chromatic, floor.clone(), None).unwrap();
        assert!(chromatic_decision.signed_mean.abs() < floor.global_mean_limit);
        assert!(chromatic_decision.rms > floor.global_rms_limit);
        assert_eq!(chromatic_decision.class, "systematic_shift");
        let shifted = image::RgbaImage::from_pixel(32, 32, image::Rgba([110, 110, 110, 255]));
        let d = decide(&b, &shifted, floor, None).unwrap();
        assert_eq!(d.class, "systematic_shift");
        assert_eq!(d.beyond_tiles.len(), 4);
    }
    #[test]
    fn float_repeat_noise_refuses_display_quantization() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("radiance.exr");
        image::DynamicImage::ImageRgb32F(image::Rgb32FImage::from_pixel(
            16,
            16,
            image::Rgb([2.0, 1.0, 0.5]),
        ))
        .save(&path)
        .unwrap();
        assert!(
            build(&[path.clone(), path], 8, None)
                .unwrap_err()
                .to_string()
                .contains("SDR")
        );
    }
}
