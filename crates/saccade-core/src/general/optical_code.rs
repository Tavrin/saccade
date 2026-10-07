//! Optical-code evidence on final SDR pixels, independent of similarity metrics.
use serde::{Deserialize, Serialize};

/// Standalone contract; existing comparison contracts are unchanged.
pub const SCHEMA: &str = "saccade-optical-code.v1";
/// Supported explicitly selected symbologies (not exhaustive scene detection).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Symbology {
    /// QR Code.
    #[default]
    Qr,
    /// Code 128.
    Code128,
    /// Code 39.
    Code39,
    /// EAN-13.
    Ean13,
    /// EAN-8.
    Ean8,
    /// UPC-A.
    Upca,
    /// UPC-E.
    Upce,
    /// Interleaved 2 of 5.
    Itf,
    /// Data Matrix.
    DataMatrix,
    /// Aztec.
    Aztec,
    /// PDF417.
    Pdf417,
}
/// Expected decoded Unicode text; no URL normalization or execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Expected {
    /// Exact equality.
    Exact(String),
    /// Rust regular expression matched against the entire payload.
    Pattern(String),
}
/// Declared assertion policy; pixel thresholds are optional and independent of decode.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Options {
    /// Symbology to search for.
    pub symbology: Symbology,
    /// Optional capture-pixel region [x, y, width, height].
    pub region: Option<[u32; 4]>,
    /// Expected value or pattern; absent means decode only.
    pub expected: Option<Expected>,
    /// Optional minimum QR module size in original pixels.
    pub minimum_module_px: Option<f64>,
    /// Optional minimum normalized luma contrast (0..1).
    pub minimum_contrast: Option<f64>,
    /// Require four sampled clear QR modules on every side.
    pub require_quiet_zone: bool,
}
/// Decoder and verification states are never inferred from pixel similarity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// No decodable candidate was located; not proof that no code exists.
    NotFound,
    /// A QR grid was located but its payload could not be decoded.
    NotDecodable,
    /// A payload was decoded; inspect payload_match and independent quality gates.
    Decoded,
}
/// QR pixel indicators, or explicit missingness for unsupported geometry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Quality {
    /// Minimum local module spacing in original capture pixels.
    pub module_size_px: Option<f64>,
    /// Difference of mean light/dark module luma divided by 255.
    pub contrast: Option<f64>,
    /// Clear sampled modules, capped at four, [top, right, bottom, left].
    pub quiet_zone_modules: Option<[u32; 4]>,
    /// True when all four strips are sampled and clear; null when unavailable.
    pub quiet_zone_clear: Option<bool>,
    /// Why an indicator is unavailable or bounded.
    pub reasons: Vec<String>,
}
/// Offline optical-code evidence for one final rendered image/page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    /// Contract discriminator.
    pub schema: String,
    /// Decoded, not_decodable or not_found, distinct from verification failure.
    pub state: State,
    /// Located candidate, including an undecodable QR grid.
    pub found: bool,
    /// Actual requested/located symbology.
    pub symbology: Symbology,
    /// Decoded Unicode text; null on failure.
    pub payload: Option<String>,
    /// Expected-payload match; null without an assertion or successful decode.
    pub payload_match: Option<bool>,
    /// Bounding envelope [x, y, width, height] in original capture pixels.
    pub bounding_box: Option<[u32; 4]>,
    /// QR projected-grid envelope or decoder-point envelope (not a full 1D symbol box).
    pub bounding_box_kind: Option<String>,
    /// Heuristic pixel margin indicators, not an ISO grade or decode probability.
    pub quality: Quality,
    /// Independent policy pass/fail; a mismatch remains state=decoded.
    pub verdict: String,
    /// Failure reasons including explicit unavailable required indicators.
    pub failures: Vec<String>,
    /// Declared policy.
    pub policy: Options,
    /// Original raster dimensions.
    pub dimensions: [u32; 2],
    /// Digest of final straight-RGBA pixels (before white compositing).
    pub raster_sha256: String,
    /// Original encoded input digest, set by file transports.
    pub input_sha256: Option<String>,
    /// One-based document page; null for raster inputs.
    pub page: Option<usize>,
    /// Declared document render density; null for raster inputs.
    pub dpi: Option<f64>,
    /// Interpretation and qualification boundaries.
    pub limitations: Vec<String>,
}

/// Verify a bounded final raster. Feature absence is a typed error, never a decode failure.
pub fn verify(image: &image::RgbaImage, options: Options) -> crate::Result<Report> {
    #[cfg(feature = "optical-code")]
    {
        native::verify(image, options)
    }
    #[cfg(not(feature = "optical-code"))]
    {
        let _ = (image, options);
        Err(crate::Error::FeatureUnavailable {
            feature: "optical-code",
        })
    }
}

#[cfg(feature = "optical-code")]
mod native {
    use super::*;
    use crate::Error;
    use rxing::{
        Binarizer, DecodeHints, Luma8Source,
        common::{DetectorRXingResult, HybridBinarizer},
    };
    mod geometry;

    impl Symbology {
        fn format(self) -> rxing::BarcodeFormat {
            use rxing::BarcodeFormat as F;
            match self {
                Self::Qr => F::QR_CODE,
                Self::Code128 => F::CODE_128,
                Self::Code39 => F::CODE_39,
                Self::Ean13 => F::EAN_13,
                Self::Ean8 => F::EAN_8,
                Self::Upca => F::UPC_A,
                Self::Upce => F::UPC_E,
                Self::Itf => F::ITF,
                Self::DataMatrix => F::DATA_MATRIX,
                Self::Aztec => F::AZTEC,
                Self::Pdf417 => F::PDF_417,
            }
        }
    }
    pub(super) fn verify(image: &image::RgbaImage, options: Options) -> crate::Result<Report> {
        let [w, h] = [image.width(), image.height()];
        if w == 0
            || h == 0
            || w > 16384
            || h > 16384
            || u64::from(w) * u64::from(h) > super::super::input::MAX_PIXELS
        {
            return Err(Error::Config("optical-code raster pixel limit".into()));
        }
        let r = options.region.unwrap_or([0, 0, w, h]);
        if r[2] == 0
            || r[3] == 0
            || u64::from(r[0]) + u64::from(r[2]) > u64::from(w)
            || u64::from(r[1]) + u64::from(r[3]) > u64::from(h)
        {
            return Err(Error::Config(
                "optical-code region is outside the raster".into(),
            ));
        }
        if options
            .minimum_module_px
            .is_some_and(|v| !v.is_finite() || v <= 0. || v > 16384.)
            || options
                .minimum_contrast
                .is_some_and(|v| !v.is_finite() || !(0. ..=1.).contains(&v))
        {
            return Err(Error::Config("invalid optical-code pixel threshold".into()));
        }
        let regex = match &options.expected {
            Some(Expected::Pattern(s)) => {
                if s.len() > 4096 {
                    return Err(Error::Config("payload pattern exceeds 4096 bytes".into()));
                }
                Some(
                    regex::RegexBuilder::new(&format!("\\A(?:{s})\\z"))
                        .size_limit(1024 * 1024)
                        .build()
                        .map_err(|e| Error::Config(format!("payload pattern: {e}")))?,
                )
            }
            Some(Expected::Exact(s)) if s.len() > 65536 => {
                return Err(Error::Config("expected payload exceeds 64 KiB".into()));
            }
            _ => None,
        };
        // Composite alpha on a declared white display background, never decode invisible RGB.
        let mut luma = Vec::with_capacity((r[2] * r[3]) as usize);
        for y in r[1]..r[1] + r[3] {
            for x in r[0]..r[0] + r[2] {
                let p = image.get_pixel(x, y).0;
                let gray =
                    (299 * u32::from(p[0]) + 587 * u32::from(p[1]) + 114 * u32::from(p[2]) + 500)
                        / 1000;
                luma.push(
                    ((gray * u32::from(p[3]) + 255 * (255 - u32::from(p[3])) + 127) / 255) as u8,
                );
            }
        }
        let mut report = Report {
            schema: SCHEMA.into(), state: State::NotFound, found: false, symbology: options.symbology,
            payload: None, payload_match: None, bounding_box: None, bounding_box_kind: None,
            quality: Quality::default(), verdict: "fail".into(), failures: Vec::new(), policy: options.clone(),
            dimensions: [w,h], raster_sha256: crate::localized::digest(image.as_raw()),
            input_sha256: None, page: None, dpi: None,
            limitations: vec![
                "One selected symbol per region; not_found means no decodable candidate located, not proven absence. Use separate regions for multiple codes.".into(),
                "Payload is decoded Unicode text, not raw codewords; never executed or dereferenced.".into(),
                "SDR luma on white-composited pixels; QR sampled margins are heuristic, not print certification, ECC headroom or decode probability.".into(),
                "Four-module quiet-zone sampling is capped and cannot prove cleanliness between samples. Crop edges limit evidence.".into(),
                "Non-QR boxes enclose decoder points, not full symbol extents; module and quiet-zone indicators are unavailable.".into(),
                "Decode and payload assertions are independent of visual similarity. No model, provider or network is used.".into(),
            ],
        };
        let hints = DecodeHints {
            TryHarder: Some(true),
            ..Default::default()
        };
        let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> crate::Result<()> {
            if options.symbology == Symbology::Qr {
                let source = Luma8Source::new_with_slice(&luma,r[2],r[3]).map_err(decoder_error)?;
                let binarizer = HybridBinarizer::new(source);
                let matrix = match binarizer.get_black_matrix() {
                    Ok(matrix) => matrix,
                    Err(e) if normal_failure(&e) => return Ok(()),
                    Err(e) => return Err(decoder_error(e)),
                };
                let grid = rxing::qrcode::detector::Detector::new(matrix).detect_with_hints(&hints);
                match grid {
                    Ok(grid) => {
                        report.found = true; report.state = State::NotDecodable;
                        let (bbox, quality) = geometry::qr(&grid, &luma, r)?;
                        report.bounding_box = Some(bbox); report.bounding_box_kind = Some("qr_projected_grid".into());
                        report.quality = quality;
                        match rxing::qrcode::decoder::qrcode_decoder::decode_bitmatrix_with_hints(grid.getBits(), &hints) {
                            Ok(value) => { report.state = State::Decoded; report.payload = Some(value.getText().into()); }
                            Err(e) if normal_failure(&e) => {},
                            Err(e) => return Err(decoder_error(e)),
                        }
                    }
                    Err(e) if normal_failure(&e) => {},
                    Err(e) => return Err(decoder_error(e)),
                }
            } else {
                match rxing::helpers::detect_in_luma_slice_with_hints(&luma, r[2], r[3], Some(options.symbology.format()), &mut hints.clone()) {
                    Ok(value) => {
                        report.found = true; report.state = State::Decoded; report.payload = Some(value.getText().into());
                        report.bounding_box = geometry::point_box(value.getPoints(), r);
                        report.bounding_box_kind = report.bounding_box.map(|_| "decoder_points".into());
                        report.quality.reasons.push("module size, quiet zone and module contrast unavailable for non-QR geometry".into());
                    }
                    Err(e) if normal_failure(&e) => {},
                    Err(e) => return Err(decoder_error(e)),
                }
            }
            Ok(())
        })).map_err(|_| Error::Config("optical-code decoder failed".into()))?;
        decoded?;
        if !report.found {
            report
                .quality
                .reasons
                .push("pixel indicators unavailable: no located code geometry".into());
        }
        if let Some(payload) = &report.payload {
            report.payload_match = match &options.expected {
                Some(Expected::Exact(s)) => Some(payload == s),
                Some(Expected::Pattern(_)) => regex.as_ref().map(|re| re.is_match(payload)),
                None => None,
            };
        }
        if report.state != State::Decoded {
            report.failures.push(
                if report.found {
                    "not_decodable"
                } else {
                    "not_found"
                }
                .into(),
            );
        }
        if report.payload_match == Some(false) {
            report.failures.push("payload_mismatch".into());
        }
        for (name, actual, minimum) in [
            (
                "module_size",
                report.quality.module_size_px,
                options.minimum_module_px,
            ),
            (
                "contrast",
                report.quality.contrast,
                options.minimum_contrast,
            ),
        ] {
            if let Some(minimum) = minimum {
                match actual {
                    Some(v) if v >= minimum => {}
                    Some(_) => report.failures.push(format!("{name}_below_minimum")),
                    None => report.failures.push(format!("{name}_unavailable")),
                }
            }
        }
        if options.require_quiet_zone && report.quality.quiet_zone_clear != Some(true) {
            report.failures.push("quiet_zone_not_established".into());
        }
        report.verdict = if report.failures.is_empty() {
            "pass"
        } else {
            "fail"
        }
        .into();
        Ok(report)
    }
    fn normal_failure(e: &rxing::Exceptions) -> bool {
        matches!(
            e,
            rxing::Exceptions::NotFoundException(_)
                | rxing::Exceptions::ChecksumException(_)
                | rxing::Exceptions::FormatException(_)
                | rxing::Exceptions::ReaderDecodeException()
        )
    }
    fn decoder_error(e: rxing::Exceptions) -> Error {
        Error::Config(format!("optical-code decoder: {e}"))
    }
}
