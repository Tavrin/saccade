//! Numerical comparisons for G-buffers, without colour conversion or FLIP.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[cfg(feature = "graphics")]
use crate::compare::{Comparison, metrics_of};
use crate::error::{Error, Result};
use crate::report::{Entry, Metric};

/// Meaning of the samples in a non-colour image.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BufferKind {
    /// Normalised depth, or linear floating-point depth.
    Depth,
    /// Unit surface normals.
    Normal,
    /// Two-dimensional motion vectors.
    Motion,
    /// Exact discrete mask samples.
    Mask,
    /// Exact discrete identifier samples.
    Id,
}

/// One `[[buffer]]` rule; the first matching rule wins.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BufferSpec {
    /// Named capture layers and layer-predicate scope (wave9).
    #[serde(default)]
    pub capture_layers: Option<crate::evidence_quality::layers::Policy>,
    /// Case-insensitive relative image glob.
    pub glob: String,
    /// Numerical meaning of the buffer.
    pub kind: BufferKind,
    /// Encoding; defaults to the kind's conventional encoding.
    #[serde(default)]
    pub encoding: String,
    /// Motion's decoded signed components are multiplied by this pixel scale.
    #[serde(default = "one")]
    pub scale: f64,
    /// Deciding statistic, default mean. Mask/id always use changed fraction.
    #[serde(default = "mean")]
    pub metric: Metric,
    /// Limit in depth units, degrees, pixels or changed-pixel fraction.
    pub threshold: Option<f64>,
}

fn one() -> f64 {
    1.0
}

fn mean() -> Metric {
    Metric::Mean
}

impl BufferSpec {
    /// Effective encoding when the config omitted it.
    pub fn encoding(&self) -> &str {
        if !self.encoding.is_empty() {
            return &self.encoding;
        }
        match self.kind {
            BufferKind::Depth => "linear01",
            BufferKind::Normal => "rgb_snorm",
            BufferKind::Motion => "rg_snorm",
            BufferKind::Mask | BufferKind::Id => "exact",
        }
    }

    /// Effective threshold in the buffer's numerical units.
    pub fn threshold(&self) -> f64 {
        self.threshold.unwrap_or(match self.kind {
            BufferKind::Depth => 0.01,
            BufferKind::Normal => 1.0,
            BufferKind::Motion => 0.5,
            BufferKind::Mask | BufferKind::Id => 0.0,
        })
    }

    /// Validates the glob and the kind/encoding contract before any writes.
    pub fn validate(&self) -> Result<()> {
        crate::config::compile_glob(&self.glob)?;
        if let Some(policy) = &self.capture_layers {
            policy.validate()?;
        }
        let valid = match self.kind {
            BufferKind::Depth => matches!(self.encoding(), "linear01" | "reverse_z" | "r32f"),
            BufferKind::Normal => matches!(self.encoding(), "rgb_snorm" | "oct"),
            BufferKind::Motion => self.encoding() == "rg_snorm",
            BufferKind::Mask | BufferKind::Id => self.encoding() == "exact",
        };
        if !valid {
            return Err(Error::Config(format!(
                "buffer {:?}: unsupported encoding {:?} for {:?}",
                self.glob,
                self.encoding(),
                self.kind
            )));
        }
        if !self.threshold().is_finite() || self.threshold() < 0.0 {
            return Err(Error::Config(
                "buffer threshold must be finite and >= 0".into(),
            ));
        }
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err(Error::Config("buffer scale must be finite and > 0".into()));
        }
        if matches!(self.kind, BufferKind::Mask | BufferKind::Id)
            && (self.metric != Metric::Mean || self.threshold() > 1.0)
        {
            return Err(Error::Config(
                "mask/id use metric mean (changed fraction) and a threshold in [0, 1]".into(),
            ));
        }
        Ok(())
    }
}

/// Statistics in the buffer's units, rather than perceptual FLIP units.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BufferStats {
    /// Mean absolute depth, angular or end-point error; changed fraction for discrete buffers.
    pub mean: f64,
    /// Largest per-pixel error.
    pub max: f64,
    /// Nearest-rank 95th percentile.
    pub p95: f64,
    /// Nearest-rank 99th percentile.
    pub p99: f64,
    /// Depth only: mean `abs(capture-base) / max(abs(base), 1e-12)`.
    pub mean_relative: Option<f64>,
    /// Depth only: maximum relative error.
    pub max_relative: Option<f64>,
    /// Mask/id only: fraction of pixels with identical native samples, including alpha.
    pub exact_match_fraction: Option<f64>,
    /// Mask/id only: number of changed pixels.
    pub changed_pixels: Option<u64>,
}

/// Additive `Entry.buffer` payload.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BufferResult {
    /// Buffer meaning.
    pub kind: BufferKind,
    /// Effective encoding.
    pub encoding: String,
    /// `depth`, `degrees`, `pixels` or `changed_fraction`.
    pub unit: String,
    /// Motion-vector scale, in pixels per signed component unit.
    pub scale: f64,
    /// Deciding statistic.
    pub metric: Metric,
    /// Deciding value in `unit`.
    pub value: f64,
    /// Pass limit in `unit`.
    pub threshold: f64,
    /// Per-kind numerical statistics.
    pub stats: BufferStats,
    /// Per-image p99 error, floored by the threshold and 1e-12, at the top of the colormap.
    pub heatmap_max: f64,
}

#[cfg(feature = "graphics")]
fn open(path: &Path) -> Result<image::DynamicImage> {
    image::open(path).map_err(|source| Error::Decode {
        path: path.to_path_buf(),
        source,
    })
}

/// Decodes an EXR's R channel, including a single-channel EXR.
#[cfg(feature = "graphics")]
pub(crate) fn decode_r32f(path: &Path) -> Result<image::DynamicImage> {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exr"))
    {
        return Err(Error::Config(
            "r32f depth requires EXR files (R channel)".into(),
        ));
    }
    decode_r32f_bytes(&crate::evidence_quality::read(path, 128 << 20)?, path)
}
#[cfg(feature = "graphics")]
pub(crate) fn decode_r32f_bytes(bytes: &[u8], path: &Path) -> Result<image::DynamicImage> {
    use exr::prelude::{ReadChannels, ReadLayers, ReadSpecificChannel};
    let metadata = exr::meta::MetaData::read_from_buffered(std::io::Cursor::new(bytes), false)
        .map_err(|e| Error::Config(format!("reading EXR dimensions: {e}")))?;
    if metadata.headers.iter().any(|h| {
        h.layer_size.width() == 0
            || h.layer_size.height() == 0
            || h.layer_size.width() > 16384
            || h.layer_size.height() > 16384
            || h.layer_size.width().saturating_mul(h.layer_size.height()) > 16_777_216
    }) {
        return Err(Error::Config("EXR scalar layer exceeds pixel limit".into()));
    }
    let img = exr::prelude::read()
        .no_deep_data()
        .largest_resolution_level()
        .specific_channels()
        .required("R")
        .collect_pixels(
            |size, _| image::Rgb32FImage::new(size.width() as u32, size.height() as u32),
            |pixels, position, (r,): (f32,)| {
                pixels.put_pixel(
                    position.x() as u32,
                    position.y() as u32,
                    image::Rgb([r, r, r]),
                )
            },
        )
        .first_valid_layer()
        .all_attributes()
        .from_buffered(std::io::Cursor::new(bytes))
        .map_err(|e| Error::Config(format!("decoding EXR R channel {}: {e}", path.display())))?;
    Ok(image::DynamicImage::ImageRgb32F(
        img.layer_data.channel_data.pixels,
    ))
}

#[cfg(feature = "graphics")]
pub(crate) fn decode(path: &Path, spec: &BufferSpec) -> Result<image::DynamicImage> {
    if spec.encoding() == "r32f" {
        decode_r32f(path)
    } else {
        open(path)
    }
}

#[cfg(feature = "graphics")]
fn normal(p: [f32; 3], oct: bool) -> Result<[f64; 3]> {
    let mut n = p.map(|v| f64::from(v) * 2.0 - 1.0);
    if oct {
        n[2] = 1.0 - n[0].abs() - n[1].abs();
        if n[2] < 0.0 {
            let x = (1.0 - n[1].abs()) * if n[0] >= 0.0 { 1.0 } else { -1.0 };
            n[1] = (1.0 - n[0].abs()) * if n[1] >= 0.0 { 1.0 } else { -1.0 };
            n[0] = x;
        }
    }
    let len = n.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !len.is_finite() || len <= 1e-12 {
        return Err(Error::Config(
            "normal buffer contains a non-finite or zero vector".into(),
        ));
    }
    Ok(n.map(|v| v / len))
}

/// Compares a numerical pair and writes its error heatmap into a normal report.
#[cfg(feature = "graphics")]
pub(crate) fn fill_pair(
    entry: &mut Entry,
    baseline: &Path,
    capture: &Path,
    out: &Path,
    spec: &BufferSpec,
    field_policy: &crate::evidence_quality::field::Policy,
) -> Result<()> {
    spec.validate()?;
    let (b, c) = (decode(baseline, spec)?, decode(capture, spec)?);
    let identical = b.color() == c.color()
        && b.width() == c.width()
        && b.height() == c.height()
        && b.as_bytes() == c.as_bytes();
    let (w, h) = (b.width(), b.height());
    if (w, h) != (c.width(), c.height()) {
        return Err(Error::DimensionMismatch {
            test_w: c.width(),
            test_h: c.height(),
            ref_w: w,
            ref_h: h,
        });
    }
    if w == 0 || h == 0 {
        return Err(Error::EmptyImage);
    }
    let mut layer_pair = spec
        .capture_layers
        .as_ref()
        .map(|policy| {
            Ok::<_, Error>((
                crate::evidence_quality::layers::load_for_buffer(
                    baseline,
                    policy,
                    &b,
                    spec.encoding() == "r32f",
                )?,
                crate::evidence_quality::layers::load_for_buffer(
                    capture,
                    policy,
                    &c,
                    spec.encoding() == "r32f",
                )?,
            ))
        })
        .transpose()?;
    if let Some((bl, cl)) = &mut layer_pair {
        crate::evidence_quality::layers::bundle(bl, out, &entry.name, "baseline")?;
        crate::evidence_quality::layers::bundle(cl, out, &entry.name, "candidate")?;
    }
    let selected = if let Some((bl, cl)) = &layer_pair {
        let policy = spec
            .capture_layers
            .as_ref()
            .ok_or_else(|| Error::Config("layer policy missing".into()))?;
        let (selected, nb, nc) = crate::evidence_quality::layers::scope_pair(bl, cl, policy)?;
        entry.layers = Some(crate::evidence_quality::layers::evidence(
            bl, cl, policy, &selected, nb, nc, &b, &c,
        )?);
        selected
    } else {
        vec![true; w as usize * h as usize]
    };
    let mut relative = Vec::new();
    let mut signed = Vec::new();
    let mut changed = None;
    let errors: Vec<f32> = if matches!(spec.kind, BufferKind::Mask | BufferKind::Id) {
        if b.color() != c.color() {
            return Err(Error::Config(
                "mask/id buffers must have the same native sample format".into(),
            ));
        }
        let stride = usize::from(b.color().bytes_per_pixel());
        let errors: Vec<f32> = b
            .as_bytes()
            .chunks_exact(stride)
            .zip(c.as_bytes().chunks_exact(stride))
            .map(|(b, c)| if b == c { 0.0 } else { 1.0 })
            .collect();
        changed = Some(errors.iter().filter(|&&e| e != 0.0).count() as u64);
        errors
    } else {
        if spec.encoding() == "r32f"
            && [baseline, capture]
                .iter()
                .any(|p| !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exr")))
        {
            return Err(Error::Config(
                "r32f depth requires EXR files (R channel)".into(),
            ));
        }
        let (b, c) = (b.to_rgb32f(), c.to_rgb32f());
        b.pixels()
            .zip(c.pixels())
            .map(|(b, c)| {
                let error = match spec.kind {
                    BufferKind::Depth => {
                        let decode = |v: f32| {
                            if spec.encoding() == "reverse_z" {
                                1.0 - f64::from(v)
                            } else {
                                f64::from(v)
                            }
                        };
                        let (b, c) = (decode(b[0]), decode(c[0]));
                        if !b.is_finite() || !c.is_finite() {
                            return Err(Error::Config(
                                "depth buffer contains non-finite samples".into(),
                            ));
                        }
                        signed.push(c - b);
                        let abs = (c - b).abs();
                        relative.push(abs / b.abs().max(1e-12));
                        abs
                    }
                    BufferKind::Normal => {
                        let (b, c) = (
                            normal(b.0, spec.encoding() == "oct")?,
                            normal(c.0, spec.encoding() == "oct")?,
                        );
                        let dot = b.iter().zip(c).map(|(b, c)| b * c).sum::<f64>();
                        if b == c {
                            0.0
                        } else {
                            dot.clamp(-1.0, 1.0).acos().to_degrees()
                        }
                    }
                    BufferKind::Motion => {
                        let dx = (f64::from(c[0]) - f64::from(b[0])) * 2.0 * spec.scale;
                        let dy = (f64::from(c[1]) - f64::from(b[1])) * 2.0 * spec.scale;
                        if ![b[0], b[1], c[0], c[1]].iter().all(|v| v.is_finite()) {
                            return Err(Error::Config(
                                "motion buffer contains non-finite samples".into(),
                            ));
                        }
                        dx.hypot(dy)
                    }
                    BufferKind::Mask | BufferKind::Id => 0.0,
                };
                if !error.is_finite() || error > f64::from(f32::MAX) {
                    return Err(Error::Config(
                        "buffer error exceeds the finite numerical range".into(),
                    ));
                }
                Ok(error as f32)
            })
            .collect::<Result<_>>()?
    };
    if signed.is_empty() {
        signed = errors.iter().map(|v| f64::from(*v)).collect();
    }
    use crate::evidence_quality::{field, maps};
    let ids = layer_pair
        .as_ref()
        .map(|(b, c)| {
            Ok::<_, Error>((
                crate::evidence_quality::layers::id_layer(b)?,
                crate::evidence_quality::layers::id_layer(c)?,
            ))
        })
        .transpose()?;
    let masked = selected.iter().any(|v| !*v);
    let excluded: Vec<_> = selected.iter().map(|v| !*v).collect();
    let id_pair = match ids {
        Some((Some(b), Some(c))) => Some((b, c)),
        None | Some((None, None)) => None,
        _ => return Err(Error::Config("ID layer absent on one buffer arm".into())),
    };
    let scope = field::scope(id_pair.is_some(), masked);
    if field_policy.require_scope && scope == "whole_frame_unmasked" {
        return Err(Error::Config(
            "render evidence requires buffer ID/mask scope".into(),
        ));
    }
    let unit = match spec.kind {
        BufferKind::Depth => "depth",
        BufferKind::Normal => "degrees",
        BufferKind::Motion => "pixels",
        BufferKind::Id | BufferKind::Mask => "changed_fraction",
    };
    let mut field_result = field::Report {
        class: if identical {
            "identical"
        } else {
            "local_structure"
        }
        .into(),
        scope: scope.into(),
        per_id: Vec::new(),
        ids_measured: 0,
        provenance: Vec::new(),
        maps: None,
        noise: None,
    };
    if let Some((bl, cl)) = &id_pair {
        let (rows, count, mean_over_threshold) = field::attribute(
            bl,
            cl,
            (w, h),
            &signed,
            if relative.is_empty() {
                None
            } else {
                Some(&relative)
            },
            Some(&excluded),
            unit,
            spec.threshold(),
            field_policy.top,
        )?;
        field_result.per_id = rows;
        field_result.ids_measured = count;
        field_result.provenance = vec![bl.provenance.clone(), cl.provenance.clone()];
        if mean_over_threshold {
            field_result.class = "systematic_shift".into();
        }
    }
    if !field_policy.noise_from.is_empty() {
        return Err(Error::Config("repeat noise requires colour captures; native buffer noise is not inferred from display pixels".into()));
    }
    let kept: Vec<_> = errors
        .iter()
        .zip(&selected)
        .filter_map(|(e, include)| include.then_some(*e))
        .collect();
    if !relative.is_empty() {
        relative = relative
            .into_iter()
            .zip(&selected)
            .filter_map(|(v, include)| include.then_some(v))
            .collect();
    }
    if changed.is_some() {
        changed = Some(kept.iter().filter(|v| **v != 0.0).count() as u64);
    }
    let m = metrics_of(&kept, w, h);
    let value = crate::run::metric_value(&m, spec.metric);
    // Every error was checked for finiteness above. A percentile keeps isolated
    // outliers from hiding ordinary errors; the threshold preserves the gate's scale.
    let heatmap_max = m.p99.max(spec.threshold()).max(1e-12);
    let unit = match spec.kind {
        BufferKind::Depth => "depth",
        BufferKind::Normal => "degrees",
        BufferKind::Motion => "pixels",
        BufferKind::Mask | BufferKind::Id => "changed_fraction",
    };
    let cmp = Comparison {
        metrics: m,
        error_map: errors
            .iter()
            .map(|&e| (f64::from(e) / heatmap_max).clamp(0.0, 1.0) as f32)
            .collect(),
    };
    let rel = format!("images/{}.d/heatmap.png", entry.name);
    let path = out.join(&rel);
    cmp.heatmap_rgb()
        .save(&path)
        .map_err(|source| Error::Encode { path, source })?;
    field::crops(
        &mut field_result.per_id,
        &b.to_rgba8(),
        &c.to_rgba8(),
        &cmp.error_map,
        out,
        &entry.name,
    )?;
    if field_policy.export_maps {
        let values = errors
            .iter()
            .zip(&selected)
            .map(|(v, selected)| if *selected { *v } else { f32::NAN })
            .collect();
        field_result.maps = Some(maps::write(
            &[maps::Map {
                name: "native_error".into(),
                dimensions: [w, h],
                unit: unit.into(),
                values,
            }],
            out,
            &entry.name,
        )?);
    }
    entry.field_evidence = Some(field_result);
    entry.metric_used = spec.metric;
    entry.threshold = spec.threshold();
    entry.value = Some(value);
    entry.status = crate::run::status_of(value, entry.threshold);
    entry.bit_identical = Some(identical);
    entry.paths.heatmap = Some(rel);
    entry.buffer = Some(BufferResult {
        kind: spec.kind,
        encoding: spec.encoding().into(),
        unit: unit.into(),
        scale: spec.scale,
        metric: spec.metric,
        value,
        threshold: spec.threshold(),
        heatmap_max,
        stats: BufferStats {
            mean: m.mean,
            max: m.max,
            p95: m.p95,
            p99: m.p99,
            mean_relative: (!relative.is_empty())
                .then(|| relative.iter().sum::<f64>() / relative.len() as f64),
            max_relative: relative.iter().copied().reduce(f64::max),
            exact_match_fraction: changed.map(|n| 1.0 - n as f64 / kept.len() as f64),
            changed_pixels: changed,
        },
    });
    Ok(())
}

#[cfg(not(feature = "graphics"))]
pub(crate) fn decode(_path: &Path, _spec: &BufferSpec) -> Result<image::DynamicImage> {
    Err(Error::FeatureUnavailable {
        feature: "graphics",
    })
}
#[cfg(not(feature = "graphics"))]
pub(crate) fn fill_pair(
    _entry: &mut Entry,
    _baseline: &Path,
    _capture: &Path,
    _out: &Path,
    _spec: &BufferSpec,
    _field_policy: &crate::evidence_quality::field::Policy,
) -> Result<()> {
    Err(Error::FeatureUnavailable {
        feature: "graphics",
    })
}
