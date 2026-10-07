//! Bounded TIFF decoding and grid metadata; no colour conversion or resampling.
use crate::{Error, Result};
use serde::Serialize;
use std::{io::Cursor, path::Path};
use tiff::{
    decoder::{Decoder, DecodingResult, Limits},
    tags::Tag,
};

/// Maximum number of decoded scalar samples, across every band.
pub const MAX_SAMPLES: u64 = 32_000_000;
/// Maximum encoded input size (512 MiB).
pub const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// Preserved grid definition and raw GeoTIFF keys.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Grid {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// CRS identifier when an EPSG key is present; user-defined keys remain below.
    pub crs: Option<String>,
    /// Affine pixel-coordinate transform: origin x, x step, x skew, origin y, y skew, y step.
    pub geotransform: Option<[f64; 6]>,
    /// GeoKeyDirectoryTag, including raster-type semantics and user-defined CRS keys.
    pub geo_keys: Vec<u16>,
    /// GeoDoubleParamsTag.
    pub geo_doubles: Vec<f64>,
    /// GeoAsciiParamsTag, without the terminal TIFF NUL.
    pub geo_ascii: Option<String>,
    /// Original ModelPixelScaleTag.
    pub pixel_scale: Vec<f64>,
    /// Original ModelTiepointTag.
    pub tiepoints: Vec<f64>,
    /// Original ModelTransformationTag.
    pub transformation: Vec<f64>,
}

/// Native samples converted losslessly to f64 from 8/16/32-bit integers or float32.
#[derive(Debug, Clone)]
pub struct Raster {
    /// Grid metadata.
    pub grid: Grid,
    /// Number of bands; samples are interleaved by pixel.
    pub bands: usize,
    /// Sample representation in the original TIFF.
    pub sample_type: &'static str,
    /// Raw nodata text; NaN is preserved as text, not JSON null.
    pub nodata: Option<String>,
    /// SHA-256 of the exact encoded bytes decoded.
    pub sha256: String,
    /// Native sample values in row-major, pixel-interleaved order.
    pub values: Vec<f64>,
}

impl Raster {
    /// Whether a sample is nodata according to this input's declaration.
    pub fn is_nodata(&self, value: f64) -> bool {
        self.nodata
            .as_ref()
            .and_then(|v| v.trim().parse::<f64>().ok())
            .is_some_and(|n| {
                let n = if self.sample_type == "float32" {
                    f64::from(n as f32)
                } else {
                    n
                };
                value == n || (value.is_nan() && n.is_nan())
            })
    }
    /// Metadata only, with no sample array.
    pub fn description(&self) -> serde_json::Value {
        serde_json::json!({"grid":self.grid,"bands":self.bands,"sample_type":self.sample_type,"nodata":self.nodata,"sha256":self.sha256})
    }
}

pub(crate) fn bytes(path: &Path, limit: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > limit {
        return Err(Error::Input(
            "input is not a regular file or exceeds size limit".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(Error::Input("input exceeds size limit".into()));
    }
    Ok(bytes)
}
fn doubles(d: &mut Decoder<Cursor<&[u8]>>, tag: Tag) -> Result<Vec<f64>> {
    let v = d
        .find_tag(tag)?
        .map(|v| v.into_f64_vec())
        .transpose()?
        .unwrap_or_default();
    if v.iter().any(|v| !v.is_finite()) {
        return Err(Error::Input("nonfinite grid metadata".into()));
    }
    Ok(v)
}
fn ascii(d: &mut Decoder<Cursor<&[u8]>>, tag: Tag) -> Result<Option<String>> {
    Ok(d.find_tag(tag)?.map(|v| v.into_string()).transpose()?)
}

/// Read one TIFF image, retaining native bands and georeferencing.
/// Multiple IFDs are refused rather than silently selecting a page or overview.
pub fn read(path: &Path) -> Result<Raster> {
    let bytes = bytes(path, MAX_BYTES)?;
    let mut d = Decoder::new(Cursor::new(bytes.as_slice()))?.with_limits(Limits::default());
    let (width, height) = d.dimensions()?;
    let bands = usize::from(
        d.find_tag_unsigned::<u16>(Tag::SamplesPerPixel)?
            .unwrap_or(1),
    );
    if width == 0
        || height == 0
        || bands == 0
        || bands > 1024
        || (u64::from(width) * u64::from(height)).saturating_mul(bands as u64) > MAX_SAMPLES
    {
        return Err(Error::Input(
            "raster exceeds 32 million samples or 1024 bands".into(),
        ));
    }
    // The TIFF decoder can invert white-is-zero or convert YCbCr. Those would
    // change numerical native units, so accept only direct sample interpretations.
    let photometric = d.get_tag_unsigned::<u16>(Tag::PhotometricInterpretation)?;
    if !matches!(photometric, 1 | 2 | 5)
        || d.find_tag_unsigned::<u16>(Tag::Orientation)?.unwrap_or(1) != 1
    {
        return Err(Error::Input(
            "native comparison requires black-is-zero, RGB or CMYK, and top-left orientation"
                .into(),
        ));
    }
    let depth = d.get_tag_u16_vec(Tag::BitsPerSample)?;
    if depth.is_empty() || depth.iter().any(|v| !matches!(v, 8 | 16 | 32)) {
        return Err(Error::Input(
            "supported native depths are 8/16/32-bit integers and float32".into(),
        ));
    }
    let keys = d
        .find_tag(Tag::GeoKeyDirectoryTag)?
        .map(|v| v.into_u16_vec())
        .transpose()?
        .unwrap_or_default();
    let geo_doubles = doubles(&mut d, Tag::GeoDoubleParamsTag)?;
    let geo_ascii = ascii(&mut d, Tag::GeoAsciiParamsTag)?;
    let mut crs = None;
    if !keys.is_empty() {
        if keys.len() < 4 || keys[0] != 1 || keys.len() != 4 + usize::from(keys[3]) * 4 {
            return Err(Error::Input("malformed GeoKey directory".into()));
        }
        let mut seen = std::collections::BTreeSet::new();
        for key in keys[4..].as_chunks::<4>().0 {
            if key[2] == 0 || !seen.insert(key[0]) {
                return Err(Error::Input("empty or duplicate GeoKey".into()));
            }
            let end = usize::from(key[3]) + usize::from(key[2]);
            let valid = match key[1] {
                0 => key[2] == 1,
                34735 => end <= keys.len(),
                34736 => end <= geo_doubles.len(),
                34737 => end <= geo_ascii.as_ref().map_or(0, |s| s.len()),
                _ => false,
            };
            if !valid {
                return Err(Error::Input("invalid GeoKey parameter reference".into()));
            }
            if matches!(key[0], 2048 | 3072 | 4096) && key[1] == 0 && key[3] != 32767 && key[3] != 0
            {
                // Prefer projected or vertical identifier over the underlying geographic CRS.
                if key[0] == 3072 || crs.is_none() {
                    crs = Some(format!("EPSG:{}", key[3]));
                }
            }
        }
    }
    let pixel_scale = doubles(&mut d, Tag::ModelPixelScaleTag)?;
    let tiepoints = doubles(&mut d, Tag::ModelTiepointTag)?;
    let transformation = doubles(&mut d, Tag::ModelTransformationTag)?;
    let geotransform = if !transformation.is_empty() {
        if !pixel_scale.is_empty()
            || !tiepoints.is_empty()
            || transformation.len() != 16
            || transformation[12..] != [0., 0., 0., 1.]
            || transformation[2] != 0.
            || transformation[6] != 0.
        {
            return Err(Error::Input(
                "ambiguous or non-affine 2D grid transform".into(),
            ));
        }
        Some([
            transformation[3],
            transformation[0],
            transformation[1],
            transformation[7],
            transformation[4],
            transformation[5],
        ])
    } else if !pixel_scale.is_empty() || !tiepoints.is_empty() {
        if pixel_scale.len() != 3
            || tiepoints.is_empty()
            || !tiepoints.len().is_multiple_of(6)
            || pixel_scale[0] <= 0.
            || pixel_scale[1] <= 0.
        {
            return Err(Error::Input("incomplete scale/tiepoint grid".into()));
        }
        let t = &tiepoints[..6];
        let g = [
            t[3] - t[0] * pixel_scale[0],
            pixel_scale[0],
            0.,
            t[4] + t[1] * pixel_scale[1],
            0.,
            -pixel_scale[1],
        ];
        for t in tiepoints.as_chunks::<6>().0 {
            if t[3] != g[0] + t[0] * g[1] || t[4] != g[3] + t[1] * g[5] {
                return Err(Error::Input(
                    "tiepoints do not define one affine grid".into(),
                ));
            }
        }
        Some(g)
    } else {
        None
    };
    if geotransform
        .is_some_and(|g| g.iter().any(|v| !v.is_finite()) || g[1] * g[5] - g[2] * g[4] == 0.)
    {
        return Err(Error::Input("invalid or singular grid transform".into()));
    }
    let nodata = ascii(&mut d, Tag::GdalNodata)?;
    if nodata
        .as_ref()
        .is_some_and(|s| s.trim().parse::<f64>().is_err())
    {
        return Err(Error::Input("nodata is not a numeric value or NaN".into()));
    }
    let planar = d
        .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)?
        .unwrap_or(1)
        == 2;
    let mut decoded = DecodingResult::U8(Vec::new());
    d.read_image_to_buffer(&mut decoded)?;
    if d.more_images() {
        return Err(Error::Input(
            "multiple TIFF images require explicit external page/overview extraction".into(),
        ));
    }
    macro_rules! convert {
        ($v:expr, $name:expr) => {
            ($name, $v.into_iter().map(f64::from).collect::<Vec<_>>())
        };
    }
    let (sample_type, mut values) = match decoded {
        DecodingResult::U8(v) => convert!(v, "uint8"),
        DecodingResult::U16(v) => convert!(v, "uint16"),
        DecodingResult::U32(v) => convert!(v, "uint32"),
        DecodingResult::I8(v) => convert!(v, "int8"),
        DecodingResult::I16(v) => convert!(v, "int16"),
        DecodingResult::I32(v) => convert!(v, "int32"),
        DecodingResult::F32(v) => convert!(v, "float32"),
        _ => {
            return Err(Error::Input(
                "unsupported native sample representation".into(),
            ));
        }
    };
    let pixels = width as usize * height as usize;
    if values.len() != pixels * bands {
        return Err(Error::Input("decoder did not preserve every band".into()));
    }
    if planar {
        let mut interleaved = vec![0.; values.len()];
        for p in 0..pixels {
            for b in 0..bands {
                interleaved[p * bands + b] = values[b * pixels + p];
            }
        }
        values = interleaved;
    }
    if values.iter().any(|v| {
        !v.is_finite()
            && !nodata
                .as_ref()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .is_some_and(|n| *v == n || (v.is_nan() && n.is_nan()))
    }) {
        return Err(Error::Input(
            "nonfinite samples must be explicitly declared nodata".into(),
        ));
    }
    Ok(Raster {
        grid: Grid {
            width,
            height,
            crs,
            geotransform,
            geo_keys: keys,
            geo_doubles,
            geo_ascii,
            pixel_scale,
            tiepoints,
            transformation,
        },
        bands,
        sample_type,
        nodata,
        sha256: saccade_core::localized::digest(&bytes),
        values,
    })
}

/// Refuse differing grids with both complete descriptions. No tolerance or reprojection.
pub fn same_grid(a: &Raster, b: &Raster) -> Result<()> {
    if a.grid != b.grid {
        return Err(Error::Grid {
            reference: Box::new(a.grid.clone()),
            candidate: Box::new(b.grid.clone()),
        });
    }
    Ok(())
}
