//! HDR (`.exr`, `.hdr`) support: decoding to linear `f32` RGB and HDR-FLIP.
//!
//! HDR-FLIP evaluates LDR-FLIP at a range of exposures and keeps, per pixel,
//! the largest error. The exposure range is derived from the reference
//! image's luminance. `flip-rs` implements NVIDIA's reference algorithm with
//! floating-point exposures (see `THIRD_PARTY.md`). The 8-bit tone mapping
//! below is used only for display PNGs, never for HDR-FLIP evaluation.

use std::path::Path;

use serde::Deserialize;

use crate::compare::{CompareOptions, Comparison, check_ppd, metrics_of};
use crate::error::{Error, Result};
use crate::report::{HdrInfo, Properties};

/// Largest linear value kept after decoding. Larger values (and infinities)
/// are clamped so display tone mapping stays finite.
const MAX_LINEAR: f32 = 1.0e15;

/// Tone mapper applied to each exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tonemapper {
    /// Narkowicz ACES approximation (default).
    #[default]
    Aces,
    /// Hable (Uncharted 2) filmic curve.
    Hable,
    /// Luminance-based Reinhard.
    Reinhard,
}

impl Tonemapper {
    /// The name used in reports, config files and flags.
    pub fn name(self) -> &'static str {
        match self {
            Self::Aces => "aces",
            Self::Hable => "hable",
            Self::Reinhard => "reinhard",
        }
    }

    /// Parses `aces`, `hable` or `reinhard` (case-insensitive).
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "aces" => Ok(Self::Aces),
            "hable" => Ok(Self::Hable),
            "reinhard" => Ok(Self::Reinhard),
            other => Err(Error::Config(format!(
                "unknown tonemapper {other:?} (expected aces, hable or reinhard)"
            ))),
        }
    }

    /// Display tone-curve coefficients `[a0, a1, a2, b0, b1, b2]` for
    /// `(a0 x^2 + a1 x + a2) / (b0 x^2 + b1 x + b2)`. Reinhard uses its
    /// luminance-based display path instead.
    fn coefficients(self) -> [f32; 6] {
        match self {
            Self::Reinhard => [0.0, 1.0, 0.0, 0.0, 1.0, 1.0],
            // 0.6 cancels the usual ACES pre-exposure.
            Self::Aces => [
                0.6 * 0.6 * 2.51,
                0.6 * 0.03,
                0.0,
                0.6 * 0.6 * 2.43,
                0.6 * 0.59,
                0.14,
            ],
            Self::Hable => [0.231_683, 0.013_791, 0.0, 0.18, 0.3, 0.018],
        }
    }
}

/// HDR-FLIP settings (`[hdr]` in `saccade.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HdrConfig {
    /// Tone mapper.
    pub tonemapper: Tonemapper,
    /// First exposure in stops; `None` computes it from the reference.
    pub start_exposure: Option<f32>,
    /// Last exposure in stops; `None` computes it from the reference.
    pub stop_exposure: Option<f32>,
    /// Number of exposures; `None` derives it from the range.
    pub num_exposures: Option<u32>,
}

impl HdrConfig {
    /// Checks finite, ordered endpoints and a count supported by `flip-rs`.
    pub fn validate(&self) -> Result<()> {
        for (what, v) in [("start", self.start_exposure), ("stop", self.stop_exposure)] {
            if v.is_some_and(|v| !v.is_finite()) {
                return Err(Error::Config(format!("hdr {what}_exposure must be finite")));
            }
        }
        if let (Some(a), Some(b)) = (self.start_exposure, self.stop_exposure)
            && a > b
        {
            return Err(Error::Config(format!(
                "hdr start_exposure ({a}) must not exceed stop_exposure ({b})"
            )));
        }
        if self
            .num_exposures
            .is_some_and(|n| !(2..=flip_rs::MAX_EXPOSURES as u32).contains(&n))
        {
            return Err(Error::Config(format!(
                "hdr num_exposures must be between 2 and {}",
                flip_rs::MAX_EXPOSURES
            )));
        }
        Ok(())
    }

    /// Parses the CLI form `START:STOP:N` (stops, stops, count).
    pub fn parse_exposures(&mut self, s: &str) -> Result<()> {
        let bad = || Error::Config(format!("--hdr-exposures expects START:STOP:N, got {s:?}"));
        let mut parts = s.split(':');
        let (Some(a), Some(b), Some(n), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(bad());
        };
        self.start_exposure = Some(a.trim().parse().map_err(|_| bad())?);
        self.stop_exposure = Some(b.trim().parse().map_err(|_| bad())?);
        self.num_exposures = Some(n.trim().parse().map_err(|_| bad())?);
        self.validate()
    }
}

/// A decoded linear-light RGB image, tightly packed `f32`.
#[derive(Debug, Clone, PartialEq)]
pub struct HdrImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 3` linear values, row-major.
    pub data: Vec<f32>,
    /// What decoding replaced: the NaN, infinite and negative samples of the
    /// file (`data` itself is already cleaned).
    pub replaced: ReplacedSamples,
}

/// Counts of samples [`decode_hdr`] had to replace to keep `data` finite.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplacedSamples {
    /// NaN samples (became 0).
    pub nan: u64,
    /// Infinite samples (became the clamp maximum or 0).
    pub inf: u64,
    /// Finite samples below zero (became 0).
    pub negative: u64,
}

/// Whether `path` has an HDR extension (`exr` or `hdr`, case-insensitive).
pub fn is_hdr_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exr") || e.eq_ignore_ascii_case("hdr"))
}

/// Decodes an `.exr` or `.hdr` file to linear `f32` RGB. Alpha is dropped;
/// NaN and negative values become 0 and values are clamped to a large finite
/// maximum, so later arithmetic stays finite.
pub fn decode_hdr(path: &Path) -> Result<HdrImage> {
    let img = image::open(path).map_err(|source| Error::Decode {
        path: path.to_path_buf(),
        source,
    })?;
    let rgb = img.to_rgb32f();
    let (width, height) = rgb.dimensions();
    let mut replaced = ReplacedSamples::default();
    let data = rgb
        .into_raw()
        .into_iter()
        .map(|v| {
            if v.is_nan() {
                replaced.nan += 1;
                0.0
            } else {
                if v.is_infinite() {
                    replaced.inf += 1;
                } else if v < 0.0 {
                    replaced.negative += 1;
                }
                v.clamp(0.0, MAX_LINEAR)
            }
        })
        .collect();
    Ok(HdrImage {
        width,
        height,
        data,
        replaced,
    })
}

/// Errors with [`Error::HdrMismatch`] when exactly one of the two paths is HDR.
pub fn check_same_kind(baseline: &Path, capture: &Path) -> Result<()> {
    if is_hdr_path(baseline) == is_hdr_path(capture) {
        return Ok(());
    }
    let kind = |p: &Path| if is_hdr_path(p) { "HDR" } else { "LDR" };
    Err(Error::HdrMismatch(format!(
        "baseline is {}, capture is {}",
        kind(baseline),
        kind(capture)
    )))
}

fn luminance(px: &[f32]) -> f32 {
    match px {
        [r, g, b] => 0.2126 * r + 0.7152 * g + 0.0722 * b,
        _ => 0.0,
    }
}

fn tonemap_px(c: [f32; 3], tm: Tonemapper) -> [f32; 3] {
    if tm == Tonemapper::Reinhard {
        let f = 1.0 / (1.0 + luminance(&c));
        return [c[0] * f, c[1] * f, c[2] * f];
    }
    let t = tm.coefficients();
    c.map(|x| {
        let x2 = x * x;
        (x2 * t[0] + x * t[1] + t[2]) / (x2 * t[3] + x * t[4] + t[5])
    })
}

fn srgb_byte(linear: f32) -> u8 {
    let v = if linear.is_nan() {
        0.0
    } else {
        linear.clamp(0.0, 1.0)
    };
    let e = if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0 + 0.5) as u8
}

/// The display PNG content: tone-mapped at exposure 0 with `tm`.
/// This quantised image is not used for HDR-FLIP evaluation.
pub fn display_image(img: &HdrImage, tm: Tonemapper) -> image::RgbImage {
    let mut out = Vec::with_capacity(img.data.len());
    for &[r, g, b] in img.data.as_chunks::<3>().0 {
        let linear = [r, g, b].map(|v| if v.is_nan() { 0.0 } else { v.min(MAX_LINEAR) });
        out.extend(tonemap_px(linear, tm).map(srgb_byte));
    }
    image::RgbImage::from_raw(img.width, img.height, out)
        .unwrap_or_else(|| image::RgbImage::new(img.width, img.height))
}

/// Structural checks on linear luminance (Rec. 709 weights, no transfer
/// function). `is_all_white` means every channel is at least 1.0.
pub fn validate_hdr(img: &HdrImage) -> Properties {
    let count = img.data.len() / 3;
    if count == 0 {
        return Properties {
            is_all_black: false,
            is_all_white: false,
            mean_luminance: 0.0,
            min_luminance: 0.0,
            max_luminance: 0.0,
            nan_count: 0,
            inf_count: 0,
            negative_count: 0,
        };
    }
    let black = img.data.iter().all(|&v| v == 0.0);
    let white = img.data.iter().all(|&v| v >= 1.0);
    // Decoding already replaced these samples, so the file's counts are used.
    let ReplacedSamples {
        nan: nan_count,
        inf: inf_count,
        negative: negative_count,
    } = img.replaced;
    // Luminance statistics cover finite pixels only, so a NaN or infinite
    // sample cannot turn them into values JSON cannot carry.
    let (mut sum, mut min, mut max, mut finite) = (0.0f64, f32::INFINITY, f32::NEG_INFINITY, 0u64);
    for px in img.data.as_chunks::<3>().0 {
        let l = luminance(px);
        if !l.is_finite() {
            continue;
        }
        sum += f64::from(l);
        min = min.min(l);
        max = max.max(l);
        finite += 1;
    }
    if finite == 0 {
        (min, max) = (0.0, 0.0);
    }
    Properties {
        is_all_black: black,
        is_all_white: white,
        mean_luminance: if finite == 0 {
            0.0
        } else {
            (sum / finite as f64) as f32
        },
        min_luminance: min,
        max_luminance: max,
        nan_count,
        inf_count,
        negative_count,
    }
}

/// Reference float HDR-FLIP of `capture` against `baseline`. Missing
/// exposure endpoints and count are resolved from the baseline by `flip-rs`.
/// The resolved parameters are returned for the entry's `hdr` record.
///
/// Errors on mismatched or empty images, malformed buffers, nonfinite input,
/// invalid viewing/exposure parameters, or automatic endpoints on an all-black
/// reference. Give explicit endpoints to compare all-black HDR references.
pub fn compare_hdr(
    capture: &HdrImage,
    baseline: &HdrImage,
    opts: &CompareOptions,
) -> Result<(Comparison, HdrInfo)> {
    check_ppd(opts.pixels_per_degree)?;
    let cfg = &opts.hdr;
    cfg.validate()?;
    let (w, h) = (capture.width, capture.height);
    let (bw, bh) = (baseline.width, baseline.height);
    if w == 0 || h == 0 || bw == 0 || bh == 0 {
        return Err(Error::EmptyImage);
    }
    if (w, h) != (bw, bh) {
        return Err(Error::DimensionMismatch {
            test_w: w,
            test_h: h,
            ref_w: bw,
            ref_h: bh,
        });
    }
    let reference = flip_rs::RgbImage::new(w as usize, h as usize, baseline.data.clone())?;
    let test = flip_rs::RgbImage::new(w as usize, h as usize, capture.data.clone())?;
    let mut options = flip_rs::HdrOptions::default();
    options.ppd = opts.pixels_per_degree;
    options.start_exposure = cfg.start_exposure;
    options.stop_exposure = cfg.stop_exposure;
    options.num_exposures = cfg.num_exposures.map(|n| n as usize);
    options.tonemapper = match cfg.tonemapper {
        Tonemapper::Aces => flip_rs::Tonemapper::Aces,
        Tonemapper::Hable => flip_rs::Tonemapper::Hable,
        Tonemapper::Reinhard => flip_rs::Tonemapper::Reinhard,
    };
    options.return_exposure_map = false;
    let result = flip_rs::hdr_flip(&reference, &test, options)?;
    let parameters = result.parameters;
    let tonemapper = match parameters.tonemapper {
        flip_rs::Tonemapper::Aces => "aces",
        flip_rs::Tonemapper::Hable => "hable",
        flip_rs::Tonemapper::Reinhard => "reinhard",
        _ => return Err(Error::Config("unsupported HDR-FLIP tonemapper".into())),
    };
    let info = HdrInfo {
        tonemapper: tonemapper.to_string(),
        start_exposure: parameters.start_exposure,
        stop_exposure: parameters.stop_exposure,
        num_exposures: u32::try_from(parameters.num_exposures)
            .map_err(|_| Error::Config("HDR-FLIP exposure count exceeds u32".into()))?,
        auto_range: cfg.start_exposure.is_none() && cfg.stop_exposure.is_none(),
    };
    let error_map = result.error_map.into_pixels();
    let metrics = metrics_of(&error_map, w, h);
    Ok((Comparison { metrics, error_map }, info))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::compare::compare;

    const N: u32 = 128;

    /// A scene with a smooth mid-tone gradient and a bright patch of `peak`.
    fn scene(peak: f32) -> HdrImage {
        let mut data = Vec::new();
        for y in 0..N {
            for x in 0..N {
                let in_patch = (32..96).contains(&x) && (32..96).contains(&y);
                let base = 0.05 + 0.3 * (x as f32 / N as f32);
                let v = if in_patch { peak } else { base };
                data.extend([v, v * 0.9, v * 0.8]);
            }
        }
        HdrImage {
            width: N,
            height: N,
            data,
            replaced: ReplacedSamples::default(),
        }
    }

    fn write_exr(dir: &Path, name: &str, img: &HdrImage) -> std::path::PathBuf {
        let buf = image::Rgb32FImage::from_raw(img.width, img.height, img.data.clone())
            .expect("buffer size");
        let path = dir.join(name);
        buf.save(&path).expect("write exr");
        path
    }

    #[test]
    fn identical_pair_scores_zero_and_exr_round_trips() {
        let dir = tempfile::tempdir().expect("tempdir");
        let img = scene(8.0);
        let path = write_exr(dir.path(), "a.exr", &img);
        let back = decode_hdr(&path).expect("decode");
        assert_eq!((back.width, back.height), (N, N));
        let (cmp, info) = compare_hdr(&back, &back, &CompareOptions::default()).expect("cmp");
        assert_eq!(cmp.metrics.max, 0.0);
        assert_eq!(cmp.metrics.mean, 0.0);
        assert!(info.auto_range && info.num_exposures >= 2);
    }

    #[test]
    fn radiance_decodes_and_reference_reinhard_exposure_is_reported() {
        // NVIDIA FLIP v1.7, FLIP.h image::computeExposures: Reinhard reaches
        // 0.85 at xmax = 0.85 / (1 - 0.85) = 17/3. For a uniform unit-linear
        // reference, both endpoints are log2(17/3) = 2.5025003 stops.
        // This checks the resolved reference value at saccade's API boundary,
        // independent of an explicit exposure override. Tolerance 1e-5 stop.
        let dir = tempfile::tempdir().expect("tempdir");
        let img = HdrImage {
            width: 4,
            height: 4,
            data: vec![1.0; 4 * 4 * 3],
            replaced: ReplacedSamples::default(),
        };
        let path = dir.path().join("unit.hdr");
        image::Rgb32FImage::from_raw(img.width, img.height, img.data)
            .expect("buffer size")
            .save(&path)
            .expect("write Radiance HDR");
        let decoded = decode_hdr(&path).expect("decode Radiance HDR");
        assert_eq!((decoded.width, decoded.height), (4, 4));
        let opts = CompareOptions {
            hdr: HdrConfig {
                tonemapper: Tonemapper::Reinhard,
                ..Default::default()
            },
            ..Default::default()
        };
        let (cmp, info) = compare_hdr(&decoded, &decoded, &opts).expect("compare");
        let expected = (17.0_f32 / 3.0).log2();
        assert!((info.start_exposure - expected).abs() < 1e-5);
        assert!((info.stop_exposure - expected).abs() < 1e-5);
        assert_eq!(info.num_exposures, 2);
        assert_eq!(info.tonemapper, "reinhard");
        assert!(info.auto_range);
        assert_eq!(cmp.metrics.max, 0.0);
    }

    #[test]
    fn one_stop_highlight_change_is_seen_by_hdr_flip_but_not_by_a_single_ldr_exposure() {
        let (base, cap) = (scene(32.0), scene(64.0));
        let opts = CompareOptions::default();
        let (hdr, _) = compare_hdr(&cap, &base, &opts).expect("hdr");
        let ldr = compare(
            &display_image(&cap, Tonemapper::Aces),
            &display_image(&base, Tonemapper::Aces),
            &opts,
        )
        .expect("ldr");
        eprintln!(
            "HDR-FLIP mean {:.5} max {:.5}; tone-mapped-only mean {:.5} max {:.5}",
            hdr.metrics.mean, hdr.metrics.max, ldr.metrics.mean, ldr.metrics.max
        );
        assert!(hdr.metrics.mean > 0.01, "hdr mean {}", hdr.metrics.mean);
        assert!(
            hdr.metrics.mean > 5.0 * ldr.metrics.mean,
            "hdr {} vs ldr {}",
            hdr.metrics.mean,
            ldr.metrics.mean
        );
    }

    #[test]
    fn mixed_ldr_hdr_pair_is_an_error() {
        let err = check_same_kind(Path::new("a/x.exr"), Path::new("b/x.png")).expect_err("mixed");
        assert!(err.to_string().contains("baseline is HDR, capture is LDR"));
        assert!(check_same_kind(Path::new("x.EXR"), Path::new("y.hdr")).is_ok());
    }

    #[test]
    fn auto_range_comes_from_the_reference() {
        let opts = CompareOptions::default();
        let base = scene(8.0);
        let (_, info) = compare_hdr(&scene(16.0), &base, &opts).expect("auto");
        assert!(info.start_exposure < 0.0 && info.stop_exposure > info.start_exposure);
        assert!(info.auto_range);
        let (_, same_info) = compare_hdr(&base, &base, &opts).expect("same reference");
        assert_eq!(info, same_info);
        let (_, brighter) = compare_hdr(&base, &scene(16.0), &opts).expect("brighter reference");
        assert!((brighter.start_exposure - (info.start_exposure - 1.0)).abs() < 1e-4);

        for tonemapper in [Tonemapper::Aces, Tonemapper::Hable, Tonemapper::Reinhard] {
            let given = CompareOptions {
                hdr: HdrConfig {
                    tonemapper,
                    start_exposure: Some(-2.0),
                    stop_exposure: Some(1.0),
                    num_exposures: Some(4),
                },
                ..opts
            };
            let (_, info) = compare_hdr(&scene(16.0), &base, &given).expect("explicit");
            assert_eq!(info.tonemapper, tonemapper.name());
            assert_eq!(
                (
                    info.start_exposure,
                    info.stop_exposure,
                    info.num_exposures,
                    info.auto_range
                ),
                (-2.0, 1.0, 4, false)
            );
            let partial = CompareOptions {
                hdr: HdrConfig {
                    stop_exposure: None,
                    ..given.hdr
                },
                ..given
            };
            let (_, info) = compare_hdr(&base, &base, &partial).expect("partial auto");
            assert_eq!(info.start_exposure, -2.0);
            assert!(!info.auto_range);
        }
    }

    #[test]
    fn float_hdr_difference_survives_identical_display_bytes() {
        let mut base = scene(1.0);
        base.data.fill(0.5);
        let mut cap = base.clone();
        cap.data.fill(0.5001);
        assert_eq!(
            display_image(&base, Tonemapper::Aces),
            display_image(&cap, Tonemapper::Aces)
        );
        let opts = CompareOptions {
            hdr: HdrConfig {
                start_exposure: Some(0.0),
                stop_exposure: Some(0.0),
                num_exposures: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let (cmp, info) = compare_hdr(&cap, &base, &opts).expect("float HDR");
        assert!(cmp.metrics.mean > 0.0);
        assert_eq!(info.num_exposures, 2);
    }

    #[test]
    fn undefined_hdr_inputs_are_errors() {
        let mut black = scene(1.0);
        black.data.fill(0.0);
        let opts = CompareOptions::default();
        assert!(compare_hdr(&black, &black, &opts).is_err());
        let explicit = CompareOptions {
            hdr: HdrConfig {
                start_exposure: Some(0.0),
                stop_exposure: Some(0.0),
                num_exposures: Some(2),
                ..Default::default()
            },
            ..opts
        };
        let (cmp, info) = compare_hdr(&black, &black, &explicit).expect("explicit black");
        assert_eq!(cmp.metrics.max, 0.0);
        assert_eq!(info.num_exposures, 2);
        for n in [0, 1, flip_rs::MAX_EXPOSURES as u32 + 1, u32::MAX] {
            let bad = CompareOptions {
                hdr: HdrConfig {
                    num_exposures: Some(n),
                    ..explicit.hdr
                },
                ..opts
            };
            assert!(matches!(
                compare_hdr(&black, &black, &bad),
                Err(Error::Config(_))
            ));
        }
        let mut malformed = black.clone();
        malformed.data.pop();
        assert!(compare_hdr(&malformed, &black, &explicit).is_err());
        let mut nonfinite = black.clone();
        nonfinite.data[0] = f32::NAN;
        assert!(compare_hdr(&nonfinite, &black, &explicit).is_err());
        let mismatch = HdrImage {
            width: 1,
            height: 1,
            data: vec![0.0; 3],
            ..black.clone()
        };
        assert!(matches!(
            compare_hdr(&mismatch, &black, &explicit),
            Err(Error::DimensionMismatch { .. })
        ));
        let empty = HdrImage {
            width: 0,
            height: 0,
            data: vec![],
            ..black
        };
        assert!(matches!(
            compare_hdr(&empty, &empty, &explicit),
            Err(Error::EmptyImage)
        ));
    }
}
