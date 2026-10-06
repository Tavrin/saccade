//! Portable per-pixel float maps: NumPy v1 and 32-bit EXR, without Python tooling.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
/// Numeric map bundle index discriminator.
pub const SCHEMA: &str = "saccade-error-maps.v1";
/// One row-major, 32-bit floating-point map.
#[derive(Debug, Clone, PartialEq)]
pub struct Map {
    /// Stable map name.
    pub name: String,
    /// Width/height (tile maps are actual grids, not resized pixel fields).
    pub dimensions: [u32; 2],
    /// Unit description.
    pub unit: String,
    /// Row-major f32 samples, NaN means outside scope.
    pub values: Vec<f32>,
}
/// One map's encoded artifacts.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    /// Name.
    pub name: String,
    /// Array height/width.
    pub shape: [u32; 2],
    /// Unit.
    pub unit: String,
    /// NumPy v1 little-endian f32, row-major.
    pub npy: String,
    /// EXR f32 R/G/B channels each carry the same scalar.
    pub exr: String,
}
/// Numeric map index.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Index {
    /// Versioned discriminator.
    pub schema: String,
    /// Describes shape, NaN exclusions and EXR channel layout.
    pub layout: String,
    /// Artifacts.
    pub maps: Vec<Artifact>,
}
/// Extract FLIP samples and every numerical tile-statistics grid.
pub fn collect(
    errors: &[f32],
    dimensions: (u32, u32),
    excluded: Option<&[bool]>,
    spatial: Option<&super::spatial::SpatialReport>,
) -> Result<Vec<Map>> {
    let (w, h) = dimensions;
    if errors.len() != w as usize * h as usize || excluded.is_some_and(|m| m.len() != errors.len())
    {
        return Err(Error::Config("map geometry differs".into()));
    }
    let mut maps = vec![Map {
        name: "flip".into(),
        dimensions: [w, h],
        unit: "FLIP [0,1]".into(),
        values: errors
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if excluded.is_some_and(|m| m[i]) {
                    f32::NAN
                } else {
                    *v
                }
            })
            .collect(),
    }];
    if let Some(s) = spatial {
        let size = s.policy.tile_size;
        let dims = [w.div_ceil(size), h.div_ceil(size)];
        type Extractor = fn(&super::spatial::Tile) -> Option<f64>;
        let fields: &[(&str, Extractor)] = &[
            ("baseline_mean", |t| Some(t.baseline_mean)),
            ("candidate_mean", |t| Some(t.candidate_mean)),
            ("signed_shift", |t| Some(t.signed_shift)),
            ("relative_shift", |t| Some(t.relative_shift)),
            ("shift_ci_low", |t| Some(t.shift_ci[0])),
            ("shift_ci_high", |t| Some(t.shift_ci[1])),
            ("baseline_contrast", |t| Some(t.baseline_contrast)),
            ("candidate_contrast", |t| Some(t.candidate_contrast)),
            ("variance_ratio", |t| t.variance_ratio),
            ("detail_energy_ratio", |t| t.detail_energy_ratio),
            ("baseline_gap", |t| t.baseline_gap),
            ("candidate_gap", |t| t.candidate_gap),
            ("gap_change", |t| t.gap_change),
            ("mean_flip", |t| Some(t.mean_flip)),
            ("coarse_mean_flip", |t| Some(t.coarse_mean_flip)),
        ];
        for (name, get) in fields {
            maps.push(Map {
                name: format!("tile_{name}"),
                dimensions: dims,
                unit: "spatial-1 recorded statistic".into(),
                values: s
                    .tiles
                    .iter()
                    .map(|t| {
                        if t.pixels == 0 {
                            f32::NAN
                        } else {
                            get(t).map_or(f32::NAN, |v| v as f32)
                        }
                    })
                    .collect(),
            });
        }
    }
    Ok(maps)
}
/// NumPy v1 encoding without an external numpy installation.
pub fn npy(map: &Map) -> Result<Vec<u8>> {
    let [w, h] = map.dimensions;
    if w == 0 || h == 0 || map.values.len() != w as usize * h as usize {
        return Err(Error::Config("invalid map dimensions".into()));
    }
    let mut header = format!("{{'descr': '<f4', 'fortran_order': False, 'shape': ({h}, {w}), }}");
    let padding = (64 - (10 + header.len() + 1) % 64) % 64;
    header.push_str(&" ".repeat(padding));
    header.push('\n');
    let length =
        u16::try_from(header.len()).map_err(|_| Error::Config("NPY header too large".into()))?;
    let mut bytes = Vec::with_capacity(10 + header.len() + map.values.len() * 4);
    bytes.extend_from_slice(b"\x93NUMPY\x01\x00");
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(header.as_bytes());
    for value in &map.values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}
/// Persist maps and JSON index to the report bundle.
pub fn write(maps: &[Map], out: &Path, name: &str) -> Result<String> {
    let digest = crate::localized::digest(name.as_bytes());
    let dir = format!("maps/{}", &digest[..16]);
    std::fs::create_dir_all(out.join(&dir)).map_err(crate::run::io_err("creating maps".into()))?;
    let mut artifacts = Vec::new();
    for map in maps {
        if !map
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(Error::Config("invalid map name".into()));
        }
        let npy_path = format!("{dir}/{}.npy", map.name);
        let exr_path = format!("{dir}/{}.exr", map.name);
        std::fs::write(out.join(&npy_path), npy(map)?)
            .map_err(crate::run::io_err("writing NPY".into()))?;
        let [w, h] = map.dimensions;
        exr::prelude::write_rgb_file(out.join(&exr_path), w as usize, h as usize, |x, y| {
            let v = map.values[y * w as usize + x];
            (v, v, v)
        })
        .map_err(|e| Error::Config(format!("writing float EXR: {e}")))?;
        artifacts.push(Artifact {
            name: map.name.clone(),
            shape: [h, w],
            unit: map.unit.clone(),
            npy: npy_path,
            exr: exr_path,
        });
    }
    let index=Index{schema:SCHEMA.into(),layout:"row-major float32; shape [height,width]; NaN=excluded/unavailable; EXR R/G/B replicate scalar, no display scaling; tile grids are not per-pixel interpolation".into(),maps:artifacts};
    let path = format!("{dir}/index.json");
    std::fs::write(out.join(&path), serde_json::to_vec_pretty(&index)?)
        .map_err(crate::run::io_err("writing map index".into()))?;
    Ok(path)
}
#[cfg(test)]
mod tests {
    #[test]
    fn float_artifacts_preserve_samples_and_native_exr_precision() {
        let map = super::Map {
            name: "flip".into(),
            dimensions: [2, 2],
            unit: "error".into(),
            values: vec![0.0, 0.12345679, -0.4, f32::NAN],
        };
        let bytes = super::npy(&map).unwrap();
        assert_eq!(&bytes[..8], b"\x93NUMPY\x01\x00");
        let header = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
        assert_eq!((10 + header) % 64, 0);
        assert_eq!(
            &bytes[10 + header + 4..10 + header + 8],
            &map.values[1].to_le_bytes()
        );
        let tmp = tempfile::tempdir().unwrap();
        let path = super::write(std::slice::from_ref(&map), tmp.path(), "frame").unwrap();
        let index: super::Index =
            serde_json::from_slice(&std::fs::read(tmp.path().join(path)).unwrap()).unwrap();
        let img = image::open(tmp.path().join(&index.maps[0].exr))
            .unwrap()
            .to_rgb32f();
        assert_eq!(img.get_pixel(1, 0)[0], map.values[1]);
        assert_eq!(img.get_pixel(0, 1)[0], -0.4);
        assert!(img.get_pixel(1, 1)[0].is_nan());
    }
}
